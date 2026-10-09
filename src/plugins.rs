//! Lua plugin host.
//!
//! Plugins are Lua scripts that react to events through the `ptc` global
//! (see `plugins/bootstrap.lua`). They can neither add hotkeys nor UI, and they
//! are not sandboxed (`io` and `os` are available).

use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use mlua::{Function, Lua, LuaSerdeExt, Value};
use serde::Serialize;

use crate::Task;

const BOOTSTRAP: &str = include_str!("../plugins/bootstrap.lua");
/// Always loaded, in this order, before the user's plugins.
const CORE_PLUGINS: [(&str, &str); 2] = [
    ("core.persist", include_str!("../plugins/core_persist.lua")),
    ("core.status_file", include_str!("../plugins/core_status_file.lua")),
];

pub struct PluginHost {
    lua: Lua,
    emit: Function,
    plugins_dir: PathBuf,
    preset: Option<String>,
    /// Tasks a plugin asked for with `ptc.set_tasks`, not yet applied by the app.
    pending_tasks: Rc<RefCell<Option<Vec<Task>>>>,
    /// Config patches from `ptc.config`, merged by the app in order.
    pending_config: Rc<RefCell<Vec<serde_json::Value>>>,
    errors: Rc<RefCell<Vec<String>>>,
}

impl PluginHost {
    /// Creates the Lua state with the `ptc` API and loads the core plugins. `preset` is the
    /// `--preset` name, available to plugins as `ptc.preset` (nil without one).
    pub fn new(
        data_dir: &Path,
        plugins_dir: &Path,
        status_file: &Path,
        preset: Option<&str>,
    ) -> mlua::Result<Self> {
        let lua = Lua::new();
        let pending_tasks = Rc::new(RefCell::new(None));
        let pending_config = Rc::new(RefCell::new(Vec::new()));
        let errors = Rc::new(RefCell::new(Vec::new()));

        let ptc = lua.create_table()?;
        ptc.set("data_dir", data_dir.to_string_lossy().into_owned())?;
        ptc.set("plugins_dir", plugins_dir.to_string_lossy().into_owned())?;
        ptc.set("status_file", status_file.to_string_lossy().into_owned())?;
        if let Some(preset) = preset {
            ptc.set("preset", preset)?;
        }

        let pending = Rc::clone(&pending_tasks);
        ptc.set(
            "set_tasks",
            lua.create_function(move |lua, value: Value| {
                let tasks: Vec<Task> = lua.from_value(value)?;
                *pending.borrow_mut() = Some(tasks);
                Ok(())
            })?,
        )?;

        let patches = Rc::clone(&pending_config);
        ptc.set(
            "config",
            lua.create_function(move |lua, value: Value| {
                let patch: serde_json::Value = lua.from_value(value)?;
                if !patch.is_object() {
                    return Err(mlua::Error::runtime("ptc.config expects a table"));
                }
                patches.borrow_mut().push(patch);
                Ok(())
            })?,
        )?;

        let reported = Rc::clone(&errors);
        ptc.set(
            "_report",
            lua.create_function(move |_, message: String| {
                reported.borrow_mut().push(message);
                Ok(())
            })?,
        )?;

        let json = lua.create_table()?;
        json.set(
            "encode",
            lua.create_function(|_, value: Value| {
                serde_json::to_string_pretty(&value).map_err(mlua::Error::external)
            })?,
        )?;
        json.set(
            "decode",
            lua.create_function(|lua, text: String| {
                let value: serde_json::Value =
                    serde_json::from_str(&text).map_err(mlua::Error::external)?;
                lua.to_value(&value)
            })?,
        )?;
        ptc.set("json", json)?;
        lua.globals().set("ptc", ptc)?;

        let emit: Function = lua.load(BOOTSTRAP).set_name("=bootstrap").eval()?;

        let host = PluginHost {
            lua,
            emit,
            plugins_dir: plugins_dir.to_path_buf(),
            preset: preset.map(str::to_string),
            pending_tasks,
            pending_config,
            errors,
        };
        for (name, source) in CORE_PLUGINS {
            host.load_source(name, source);
        }
        Ok(host)
    }

    /// Loads the `*.lua` files of the plugins directory.
    ///
    /// A file named `<name>.<preset>.lua` is a preset variant: it is loaded only when ptc runs with
    /// `--preset <preset>`, right after the shared `<name>.lua`. Everything else loads always.
    pub fn load_user_plugins(&self) {
        let Ok(entries) = fs::read_dir(&self.plugins_dir) else {
            return;
        };
        let mut plugins: Vec<(String, Option<String>, PathBuf)> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "lua"))
            .filter_map(|path| {
                let stem = path.file_stem()?.to_string_lossy().into_owned();
                let (name, preset) = match stem.rsplit_once('.') {
                    Some((name, preset)) => (name.to_string(), Some(preset.to_string())),
                    None => (stem, None),
                };
                Some((name, preset, path))
            })
            // Presets other than the current one are skipped.
            .filter(|(_, preset, _)| preset.is_none() || *preset == self.preset)
            .collect();
        // By name, the shared file before its preset variant.
        plugins.sort_by(|a, b| (&a.0, a.1.is_some()).cmp(&(&b.0, b.1.is_some())));

        for (_, _, file) in plugins {
            let name = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            match fs::read_to_string(&file) {
                Ok(source) => self.load_source(&name, &source),
                Err(err) => self.report(format!("plugin {name}: {err}")),
            }
        }
    }

    pub fn load_source(&self, name: &str, source: &str) {
        let result = self.lua.load(source).set_name(format!("={name}")).exec();
        if let Err(err) = result {
            self.report(format!("plugin {name}: {err}"));
        }
    }

    pub fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        let result = self
            .lua
            .to_value(payload)
            .and_then(|payload| self.emit.call::<()>((event, payload)));
        if let Err(err) = result {
            self.report(format!("event {event}: {err}"));
        }
    }

    pub fn emit_empty(&self, event: &str) {
        if let Err(err) = self.emit.call::<()>((event, Value::Nil)) {
            self.report(format!("event {event}: {err}"));
        }
    }

    #[cfg(test)]
    pub fn eval_for_test(&self, chunk: &str) -> String {
        self.lua.load(chunk).eval().unwrap()
    }

    pub fn take_pending_tasks(&self) -> Option<Vec<Task>> {
        self.pending_tasks.borrow_mut().take()
    }

    pub fn take_pending_config(&self) -> Vec<serde_json::Value> {
        std::mem::take(&mut *self.pending_config.borrow_mut())
    }

    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.borrow_mut())
    }

    pub fn report(&self, message: String) {
        self.errors.borrow_mut().push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(text: &str) -> Task {
        Task {
            text: text.to_string(),
            estimated_pomodoros: 2,
            completed_pomodoros: 1,
            done: false,
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn handlers_receive_events_and_errors_are_collected() {
        let dir = scratch_dir("events");
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.load_source(
            "test",
            r#"
            seen = {}
            ptc.on("pomodoro_state_changed", function(e) table.insert(seen, e.state) end)
            ptc.on("quit", function() error("boom") end)
            ptc.set_tasks({ { text = "a", estimated_pomodoros = 3, completed_pomodoros = 0, done = false } })
            "#,
        );
        assert!(host.take_errors().is_empty());

        #[derive(Serialize)]
        struct Event {
            state: &'static str,
        }
        host.emit("pomodoro_state_changed", &Event { state: "work" });
        let seen: String = host
            .lua
            .load(r#"return table.concat(seen, ",")"#)
            .eval()
            .unwrap();
        assert_eq!(seen, "work");

        host.emit_empty("quit");
        assert_eq!(host.take_errors().len(), 1);

        let tasks = host.take_pending_tasks().unwrap();
        assert_eq!(tasks[0].text, "a");
        assert_eq!(tasks[0].estimated_pomodoros, 3);
    }

    #[test]
    fn unknown_events_are_rejected() {
        let dir = scratch_dir("unknown");
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.load_source("test", r#"ptc.on("nope", function() end)"#);
        assert_eq!(host.take_errors().len(), 1);
    }

    #[test]
    fn core_plugin_persists_tasks_across_hosts() {
        let dir = scratch_dir("persist");

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.emit_empty("startup");
        assert!(host.take_pending_tasks().is_none());
        host.emit("tasks_changed", &vec![task("write"), task("test")]);
        assert!(host.take_errors().is_empty(), "{:?}", host.take_errors());

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.emit_empty("startup");
        let tasks = host.take_pending_tasks().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[1].text, "test");
        assert_eq!(tasks[0].completed_pomodoros, 1);
    }

    #[test]
    fn empty_task_list_round_trips() {
        let dir = scratch_dir("empty");
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.emit("tasks_changed", &Vec::<Task>::new());
        assert!(host.take_errors().is_empty(), "{:?}", host.take_errors());

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.emit_empty("startup");
        assert_eq!(host.take_pending_tasks().unwrap().len(), 0);
    }

    #[test]
    fn status_file_core_plugin_writes_the_status_file() {
        let dir = scratch_dir("status_file");
        let file = dir.join("status.json");
        let host = PluginHost::new(&dir, &dir, &file, None).unwrap();
        let read = || -> serde_json::Value {
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap()
        };

        host.emit_empty("startup");
        assert_eq!(read()["state"], "idle");

        #[derive(Serialize)]
        struct Event {
            state: &'static str,
            paused: bool,
            waiting: bool,
            completed_pomodoros: u32,
            ends_at: u64,
            remaining_seconds: u64,
            current_task: Task,
        }
        host.emit(
            "pomodoro_state_changed",
            &Event {
                state: "work",
                paused: false,
                waiting: false,
                completed_pomodoros: 2,
                ends_at: 1_800_000_000,
                remaining_seconds: 1500,
                current_task: task("write the docs"),
            },
        );
        let status = read();
        assert_eq!(status["state"], "work");
        assert_eq!(status["ends_at"], 1_800_000_000u64);
        assert_eq!(status["remaining_seconds"], 1500);
        assert_eq!(status["completed_pomodoros"], 2);
        assert_eq!(status["current_task"]["text"], "write the docs");
        assert_eq!(status["current_task"]["estimated_pomodoros"], 2);
        assert!(status["updated_at"].is_number());

        // Idle after a finished break carries the moment it started; a plain idle does not.
        #[derive(Serialize)]
        struct IdleEvent {
            state: &'static str,
            // Like the real event: absent, not null, when there is none.
            #[serde(skip_serializing_if = "Option::is_none")]
            idle_since: Option<u64>,
        }
        host.emit("pomodoro_state_changed", &IdleEvent { state: "idle", idle_since: Some(1_800_000_100) });
        assert_eq!(read()["state"], "idle");
        assert_eq!(read()["idle_since"], 1_800_000_100u64);
        host.emit("pomodoro_state_changed", &IdleEvent { state: "idle", idle_since: None });
        assert!(read().get("idle_since").is_none());

        host.emit_empty("quit");
        assert_eq!(read()["state"], "off");
        assert!(host.take_errors().is_empty(), "{:?}", host.take_errors());
    }

    #[test]
    fn preset_is_visible_to_plugins_and_selects_plugin_variants_and_the_tasks_file() {
        let dir = scratch_dir("preset");
        let status = dir.join("ptc-status.json");
        // `a.job.lua` is a variant of `a.lua`, `b.home.lua` belongs to another preset.
        for (file, body) in [
            ("a.lua", r#"log = (log or "") .. "a,""#),
            ("a.job.lua", r#"log = (log or "") .. "a.job,""#),
            ("b.home.lua", r#"log = (log or "") .. "b.home,""#),
            ("c.lua", r#"log = (log or "") .. "c,""#),
        ] {
            fs::write(dir.join(file), body).unwrap();
        }

        let host = PluginHost::new(&dir, &dir, &status, Some("job")).unwrap();
        host.load_user_plugins();
        assert!(host.take_errors().is_empty());
        assert_eq!(host.eval_for_test("return ptc.preset"), "job");
        // The variant loads right after its shared file; other presets' files are skipped.
        assert_eq!(host.eval_for_test("return log"), "a,a.job,c,");

        // Tasks are kept per preset.
        host.emit("tasks_changed", &vec![task("only in job")]);
        assert!(dir.join("tasks.job.json").exists());
        assert!(!dir.join("tasks.json").exists());

        let host = PluginHost::new(&dir, &dir, &status, None).unwrap();
        host.load_user_plugins();
        assert_eq!(host.eval_for_test("return tostring(ptc.preset)"), "nil");
        assert_eq!(host.eval_for_test("return log"), "a,c,");
        host.emit_empty("startup");
        assert!(host.take_pending_tasks().is_none(), "the default tasks file is separate");

        let host = PluginHost::new(&dir, &dir, &status, Some("job")).unwrap();
        host.emit_empty("startup");
        assert_eq!(host.take_pending_tasks().unwrap()[0].text, "only in job");
    }
}
