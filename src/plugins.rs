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
    /// Tasks a plugin asked for with `ptc.set_tasks`, not yet applied by the app.
    pending_tasks: Rc<RefCell<Option<Vec<Task>>>>,
    /// Config patches from `ptc.config`, merged by the app in order.
    pending_config: Rc<RefCell<Vec<serde_json::Value>>>,
    errors: Rc<RefCell<Vec<String>>>,
}

impl PluginHost {
    /// Creates the Lua state with the `ptc` API and loads the core plugins.
    pub fn new(data_dir: &Path, plugins_dir: &Path, status_file: &Path) -> mlua::Result<Self> {
        let lua = Lua::new();
        let pending_tasks = Rc::new(RefCell::new(None));
        let pending_config = Rc::new(RefCell::new(Vec::new()));
        let errors = Rc::new(RefCell::new(Vec::new()));

        let ptc = lua.create_table()?;
        ptc.set("data_dir", data_dir.to_string_lossy().into_owned())?;
        ptc.set("plugins_dir", plugins_dir.to_string_lossy().into_owned())?;
        ptc.set("status_file", status_file.to_string_lossy().into_owned())?;

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
            pending_tasks,
            pending_config,
            errors,
        };
        for (name, source) in CORE_PLUGINS {
            host.load_source(name, source);
        }
        Ok(host)
    }

    /// Loads every `*.lua` file of the plugins directory, in name order.
    pub fn load_user_plugins(&self) {
        let Ok(entries) = fs::read_dir(&self.plugins_dir) else {
            return;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "lua"))
            .collect();
        files.sort();

        for file in files {
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
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json")).unwrap();
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
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json")).unwrap();
        host.load_source("test", r#"ptc.on("nope", function() end)"#);
        assert_eq!(host.take_errors().len(), 1);
    }

    #[test]
    fn core_plugin_persists_tasks_across_hosts() {
        let dir = scratch_dir("persist");

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json")).unwrap();
        host.emit_empty("startup");
        assert!(host.take_pending_tasks().is_none());
        host.emit("tasks_changed", &vec![task("write"), task("test")]);
        assert!(host.take_errors().is_empty(), "{:?}", host.take_errors());

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json")).unwrap();
        host.emit_empty("startup");
        let tasks = host.take_pending_tasks().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[1].text, "test");
        assert_eq!(tasks[0].completed_pomodoros, 1);
    }

    #[test]
    fn empty_task_list_round_trips() {
        let dir = scratch_dir("empty");
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json")).unwrap();
        host.emit("tasks_changed", &Vec::<Task>::new());
        assert!(host.take_errors().is_empty(), "{:?}", host.take_errors());

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json")).unwrap();
        host.emit_empty("startup");
        assert_eq!(host.take_pending_tasks().unwrap().len(), 0);
    }

    #[test]
    fn status_file_core_plugin_writes_the_status_file() {
        let dir = scratch_dir("status_file");
        let file = dir.join("status.json");
        let host = PluginHost::new(&dir, &dir, &file).unwrap();
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
}
