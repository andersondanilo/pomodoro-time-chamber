use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::PathBuf,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use plugins::PluginHost;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use stats::Stats;

mod number_ascii_art;
mod plugins;
mod stats;
mod stats_view;

const SIDEBAR_WIDTH: u16 = 40;
const MIN_ESTIMATED_POMODOROS: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Theme {
    /// Background of the whole screen (sidebar).
    background: Color,
    /// Background of the tasks panel.
    panel_background: Color,
    foreground: Color,
    /// App title shown above the clock.
    title: Color,
    /// Hotkey hint text (arrow and label).
    hotkey: Color,
    /// The key itself in a hotkey hint.
    hotkey_key: Color,
    /// Status line background for each pomodoro state.
    status_idle: Color,
    status_work: Color,
    status_short_break: Color,
    status_long_break: Color,
    /// Background of the selected task.
    selection: Color,
    /// Scrollbar of the task list.
    scrollbar: Color,
    /// Background of the status bar at the bottom of the tasks panel.
    tasks_status_bar: Color,
    /// Text color of that status bar.
    tasks_status_bar_text: Color,
    /// Background of the 3-column marker when the task is not done.
    task_unmarked: Color,
    /// Background of the 3-column marker when the task is done.
    task_marked: Color,
    /// Color of the `x` inside the done marker.
    task_marked_text: Color,
    /// The animated `✻` before the current task.
    task_spinner: Color,
    /// "Quit ptc?" question that replaces the hotkey list.
    quit_prompt: Color,
    /// `TASKS` title of the task list.
    tasks_title: Color,
    /// The preset next to that title.
    tasks_title_dim: Color,
    /// Statistics page.
    stats_title: Color,
    stats_heading: Color,
    stats_label: Color,
    /// Secondary text, empty states and the days without pomodoros.
    stats_dim: Color,
    /// The estimated part of a bar in the worst estimates.
    stats_estimated: Color,
    /// The part over the estimate: the worst task has the first colour, the 10th the second.
    stats_overrun_worst: Color,
    stats_overrun_mild: Color,
    /// Bars of the days chart go from the low to the high colour with the number of pomodoros.
    stats_chart_low: Color,
    stats_chart_high: Color,
    /// The bar and the label of today.
    stats_today: Color,
    /// Text drawn on a bar (its value).
    stats_bar_text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        let background = Color::Rgb(0x1a, 0x1b, 0x26);
        Theme {
            background,
            panel_background: Color::Rgb(0x24, 0x28, 0x3b),
            foreground: Color::White,
            title: Color::White,
            hotkey: Color::Blue,
            hotkey_key: Color::Yellow,
            status_idle: Color::DarkGray,
            status_work: Color::Red,
            status_short_break: Color::Green,
            status_long_break: Color::Blue,
            selection: Color::Rgb(0x3b, 0x42, 0x61),
            scrollbar: Color::Gray,
            tasks_status_bar: Color::Rgb(0x2e, 0x33, 0x50),
            tasks_status_bar_text: Color::Gray,
            task_unmarked: background,
            task_marked: Color::Green,
            task_marked_text: Color::Black,
            task_spinner: Color::Rgb(0xd9, 0x77, 0x57),
            quit_prompt: Color::Rgb(0xf7, 0x76, 0x8e),
            tasks_title: Color::Rgb(0xe0, 0xaf, 0x68),
            tasks_title_dim: Color::Rgb(0x56, 0x5f, 0x89),
            stats_title: Color::Rgb(0xe0, 0xaf, 0x68),
            stats_heading: Color::Rgb(0x7a, 0xa2, 0xf7),
            stats_label: Color::Rgb(0xc0, 0xca, 0xf5),
            stats_dim: Color::Rgb(0x56, 0x5f, 0x89),
            stats_estimated: Color::Rgb(0x2a, 0xc3, 0xde),
            stats_overrun_worst: Color::Rgb(0xf7, 0x76, 0x8e),
            stats_overrun_mild: Color::Rgb(0xe0, 0xaf, 0x68),
            stats_chart_low: Color::Rgb(0x3d, 0x59, 0xa1),
            stats_chart_high: Color::Rgb(0xbb, 0x9a, 0xf7),
            stats_today: Color::Rgb(0xff, 0x9e, 0x64),
            stats_bar_text: Color::Rgb(0x1a, 0x1b, 0x26),
        }
    }
}

/// A key plus modifiers (e.g. `Ctrl+j`).
#[derive(Clone, Copy)]
struct KeyBinding {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyBinding {
    fn plain(code: KeyCode) -> Self {
        KeyBinding {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> Self {
        KeyBinding {
            code,
            modifiers: KeyModifiers::CONTROL,
        }
    }

    /// SHIFT is ignored: it is already reflected in the character (`+`, `J`).
    fn matches(&self, key: &KeyEvent) -> bool {
        key.code == self.code
            && key.modifiers.difference(KeyModifiers::SHIFT)
                == self.modifiers.difference(KeyModifiers::SHIFT)
    }
}

impl std::fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            write!(f, "Ctrl+")?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            write!(f, "Alt+")?;
        }
        match self.code {
            KeyCode::Char(' ') => write!(f, "Space"),
            code => write!(f, "{code}"),
        }
    }
}

/// Parses `q`, `Enter`, `Esc`, `Ctrl+j`, `Alt+x`, `F5`, ... (modifiers and names are case-insensitive).
impl std::str::FromStr for KeyBinding {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let mut modifiers = KeyModifiers::NONE;
        let mut rest = text;
        loop {
            let prefixes = [
                ("ctrl+", KeyModifiers::CONTROL),
                ("alt+", KeyModifiers::ALT),
                ("shift+", KeyModifiers::SHIFT),
            ];
            let found = prefixes.iter().find(|(prefix, _)| {
                rest.get(..prefix.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            });
            let Some((prefix, modifier)) = found else {
                break;
            };
            modifiers |= *modifier;
            rest = &rest[prefix.len()..];
        }

        let lower = rest.to_lowercase();
        let code = match lower.as_str() {
            "enter" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backspace" => KeyCode::Backspace,
            "space" => KeyCode::Char(' '),
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "delete" => KeyCode::Delete,
            "insert" => KeyCode::Insert,
            other => {
                let function_key = other
                    .strip_prefix('f')
                    .and_then(|n| n.parse::<u8>().ok())
                    .filter(|n| (1..=24).contains(n));
                let mut chars = rest.chars();
                match (function_key, chars.next(), chars.next()) {
                    (Some(n), _, _) => KeyCode::F(n),
                    (None, Some(c), None) => KeyCode::Char(c),
                    _ => return Err(format!("unknown key {text:?}")),
                }
            }
        };
        Ok(KeyBinding { code, modifiers })
    }
}

impl Serialize for KeyBinding {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Recursively merges `patch` into `base`: objects merge key by key, anything else replaces.
fn merge_json(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(base), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                match base.get_mut(key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

/// Returns `config` with `patch` merged over it; unknown keys and bad values are errors.
fn merge_config(config: &Config, patch: &serde_json::Value) -> Result<Config, String> {
    let mut merged = serde_json::to_value(config).map_err(|e| e.to_string())?;
    merge_json(&mut merged, patch);
    serde_json::from_value(merged).map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Keyboard {
    quit: KeyBinding,
    toggle_pomodoro: KeyBinding,
    pause_pomodoro: KeyBinding,
    skip_break: KeyBinding,
    add_task: KeyBinding,
    edit_task: KeyBinding,
    toggle_done: KeyBinding,
    delete_task: KeyBinding,
    clear_completed: KeyBinding,
    clear_all: KeyBinding,
    increase_estimate: KeyBinding,
    decrease_estimate: KeyBinding,
    confirm_quit: KeyBinding,
    cancel_quit: KeyBinding,
    show_stats: KeyBinding,
    close_stats: KeyBinding,
    select_next: KeyBinding,
    select_previous: KeyBinding,
    move_task_down: KeyBinding,
    move_task_up: KeyBinding,
    confirm_task: KeyBinding,
    cancel_task: KeyBinding,
}

impl Default for Keyboard {
    fn default() -> Self {
        Keyboard {
            quit: KeyBinding::plain(KeyCode::Char('q')),
            toggle_pomodoro: KeyBinding::plain(KeyCode::Char('s')),
            pause_pomodoro: KeyBinding::plain(KeyCode::Char('p')),
            skip_break: KeyBinding::plain(KeyCode::Char('b')),
            add_task: KeyBinding::plain(KeyCode::Char('a')),
            edit_task: KeyBinding::plain(KeyCode::Char('e')),
            toggle_done: KeyBinding::plain(KeyCode::Char('x')),
            delete_task: KeyBinding::plain(KeyCode::Char('d')),
            clear_completed: KeyBinding::plain(KeyCode::Char('c')),
            clear_all: KeyBinding::plain(KeyCode::Char('C')),
            increase_estimate: KeyBinding::plain(KeyCode::Char('+')),
            decrease_estimate: KeyBinding::plain(KeyCode::Char('-')),
            confirm_quit: KeyBinding::plain(KeyCode::Char('y')),
            cancel_quit: KeyBinding::plain(KeyCode::Char('n')),
            show_stats: KeyBinding::plain(KeyCode::Char('i')),
            close_stats: KeyBinding::plain(KeyCode::Esc),
            select_next: KeyBinding::plain(KeyCode::Char('j')),
            select_previous: KeyBinding::plain(KeyCode::Char('k')),
            move_task_down: KeyBinding::ctrl(KeyCode::Char('j')),
            move_task_up: KeyBinding::ctrl(KeyCode::Char('k')),
            confirm_task: KeyBinding::plain(KeyCode::Enter),
            cancel_task: KeyBinding::plain(KeyCode::Esc),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PomodoroConfig {
    work_minutes: u64,
    short_break_minutes: u64,
    long_break_minutes: u64,
    /// A long break replaces the short one after this many pomodoros.
    long_break_interval: u32,
    /// Start the break automatically when a work phase ends; otherwise wait for the start key.
    auto_start_break: bool,
    /// Send a desktop notification when a phase ends.
    notifications: bool,
    /// Mark a task as done as soon as its completed pomodoros reach the estimate. When off, the
    /// completed count keeps growing (even past the estimate) until the user marks it done.
    auto_complete_tasks: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        PomodoroConfig {
            work_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_interval: 4,
            auto_start_break: false,
            notifications: true,
            auto_complete_tasks: false,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    theme: Theme,
    keyboard: Keyboard,
    pomodoro: PomodoroConfig,
}

#[derive(Clone, Copy, PartialEq)]
enum PomodoroState {
    Idle,
    Work,
    ShortBreak,
    LongBreak,
}

impl PomodoroState {
    /// Name used in plugin events.
    fn name(self) -> &'static str {
        match self {
            PomodoroState::Idle => "idle",
            PomodoroState::Work => "work",
            PomodoroState::ShortBreak => "short_break",
            PomodoroState::LongBreak => "long_break",
        }
    }

    fn label(self) -> &'static str {
        match self {
            PomodoroState::Idle => "Idle",
            PomodoroState::Work => "Focus",
            PomodoroState::ShortBreak => "Short break",
            PomodoroState::LongBreak => "Long break",
        }
    }
}

enum AppMode {
    Normal,
    TaskTextInput,
    /// The statistics page replaces the task list.
    Stats,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Task {
    /// Stable identity (0 = not assigned yet); the statistics remember tasks by it.
    #[serde(default)]
    id: u64,
    text: String,
    /// Always at least `MIN_ESTIMATED_POMODOROS`.
    #[serde(default = "default_estimate")]
    estimated_pomodoros: u32,
    #[serde(default)]
    completed_pomodoros: u32,
    #[serde(default)]
    done: bool,
}

fn default_estimate() -> u32 {
    MIN_ESTIMATED_POMODOROS
}

/// Payload of the `pomodoro_state_changed` plugin event.
#[derive(Serialize)]
struct PomodoroEvent {
    state: &'static str,
    previous: &'static str,
    paused: bool,
    /// A break is set up and waiting for the start key.
    waiting: bool,
    completed_pomodoros: u32,
    /// Unix timestamp (seconds) when the running phase ends; absent unless it is counting down.
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_at: Option<u64>,
    /// Time left in the current phase (the full phase length when idle or waiting).
    remaining_seconds: u64,
    /// The current task (the first one that is not done); absent when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    current_task: Option<Task>,
    /// Unix timestamp (seconds) of the moment the last break finished by itself; present only
    /// while idle since then (the app shows "Idle for MM:SS"). Absent for any other idle.
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_since: Option<u64>,
    /// What differs from the previous event: any of `state`, `paused`, `waiting`, `current_task`.
    changed: Vec<&'static str>,
}

/// What `pomodoro_state_changed` is derived from.
#[derive(Clone, PartialEq)]
struct PomodoroSnapshot {
    state: PomodoroState,
    paused: bool,
    waiting: bool,
    /// The first task that is not done, including its current contents.
    current_task: Option<Task>,
}

impl PomodoroSnapshot {
    /// Names of the parts that differ from `previous`.
    fn changes_from(&self, previous: &PomodoroSnapshot) -> Vec<&'static str> {
        let mut changed = Vec::new();
        if self.state != previous.state {
            changed.push("state");
        }
        if self.paused != previous.paused {
            changed.push("paused");
        }
        if self.waiting != previous.waiting {
            changed.push("waiting");
        }
        if self.current_task != previous.current_task {
            changed.push("current_task");
        }
        changed
    }
}

struct App {
    config: Config,
    plugins: Option<PluginHost>,
    /// The `--preset` name; the statistics page shows it.
    preset: Option<String>,
    stats: Stats,
    /// Where the statistics are saved (`None` in tests).
    stats_path: Option<PathBuf>,
    stats_dirty: bool,
    /// The last problem saving the statistics, printed when the app exits.
    stats_error: Option<String>,
    /// Last task id handed out; ids only grow, so they are never reused.
    last_task_id: u64,
    /// Tasks as of the last `tasks_changed` event.
    last_tasks: Vec<Task>,
    last_pomodoro: PomodoroSnapshot,
    mode: AppMode,
    input_buffer: String,
    tasks: Vec<Task>,
    selected_task: usize,
    /// Index of the first task shown in the list; keeps the cursor row visible.
    scroll_offset: usize,
    started_at: Instant,
    /// Index of the task being edited; `None` when adding a new one.
    editing_task: Option<usize>,
    should_quit: bool,
    /// "Quit ptc?" is on screen, waiting for yes or no.
    quit_pending: bool,
    pomodoro_state: PomodoroState,
    /// When the current phase ends; `None` while idle.
    phase_ends_at: Option<Instant>,
    /// Time left in the current phase while paused (`phase_ends_at` is `None` then).
    paused_remaining: Option<Duration>,
    /// A break is set up (time in `paused_remaining`) but has not been started yet.
    awaiting_break_start: bool,
    /// The running work phase was already credited to a task (when it was marked done past
    /// half of the phase), so finishing it must not credit another task.
    work_credited: bool,
    /// When the last break finished by itself; the idle time since then is shown in the status
    /// bar. Cleared as soon as anything else starts or stops.
    idle_since: Option<Instant>,
    completed_pomodoros: u32,
}

impl App {
    fn new(config: Config) -> Self {
        App {
            config,
            plugins: None,
            preset: None,
            stats: Stats::default(),
            stats_path: None,
            stats_dirty: false,
            stats_error: None,
            last_task_id: 0,
            last_tasks: Vec::new(),
            last_pomodoro: PomodoroSnapshot {
                state: PomodoroState::Idle,
                paused: false,
                waiting: false,
                current_task: None,
            },
            mode: AppMode::Normal,
            input_buffer: String::new(),
            tasks: Vec::new(),
            selected_task: 0,
            scroll_offset: 0,
            started_at: Instant::now(),
            editing_task: None,
            should_quit: false,
            quit_pending: false,
            pomodoro_state: PomodoroState::Idle,
            phase_ends_at: None,
            paused_remaining: None,
            awaiting_break_start: false,
            work_credited: false,
            idle_since: None,
            completed_pomodoros: 0,
        }
    }

    fn phase_duration(&self, state: PomodoroState) -> Duration {
        let config = &self.config.pomodoro;
        let minutes = match state {
            PomodoroState::Idle | PomodoroState::Work => config.work_minutes,
            PomodoroState::ShortBreak => config.short_break_minutes,
            PomodoroState::LongBreak => config.long_break_minutes,
        };
        Duration::from_secs(minutes * 60)
    }

    fn remaining(&self) -> Duration {
        match (self.phase_ends_at, self.paused_remaining) {
            (Some(end), _) => end.saturating_duration_since(Instant::now()),
            (None, Some(remaining)) => remaining,
            (None, None) => self.phase_duration(PomodoroState::Idle),
        }
    }

    fn start_phase(&mut self, state: PomodoroState) {
        self.pomodoro_state = state;
        self.phase_ends_at = Some(Instant::now() + self.phase_duration(state));
        self.paused_remaining = None;
        self.awaiting_break_start = false;
        self.work_credited = false;
        self.idle_since = None;
    }

    /// Sets up a break without starting its countdown.
    fn prepare_phase(&mut self, state: PomodoroState) {
        self.pomodoro_state = state;
        self.phase_ends_at = None;
        self.paused_remaining = Some(self.phase_duration(state));
        self.awaiting_break_start = true;
        self.work_credited = false;
        self.idle_since = None;
    }

    fn stop_pomodoro(&mut self) {
        self.pomodoro_state = PomodoroState::Idle;
        self.phase_ends_at = None;
        self.paused_remaining = None;
        self.awaiting_break_start = false;
        self.work_credited = false;
        self.idle_since = None;
    }

    /// A work phase is running (or paused) and more than half of it has passed.
    fn work_past_half(&self) -> bool {
        self.pomodoro_state == PomodoroState::Work
            && self.remaining() < self.phase_duration(PomodoroState::Work) / 2
    }

    fn in_break(&self) -> bool {
        matches!(
            self.pomodoro_state,
            PomodoroState::ShortBreak | PomodoroState::LongBreak
        )
    }

    fn is_paused(&self) -> bool {
        self.paused_remaining.is_some() && !self.awaiting_break_start
    }

    /// Advances to the next phase once the current one has run out.
    fn update_pomodoro(&mut self) {
        let Some(end) = self.phase_ends_at else {
            return;
        };
        if Instant::now() < end {
            return;
        }

        match self.pomodoro_state {
            PomodoroState::Work => {
                self.completed_pomodoros += 1;
                self.stats
                    .record_pomodoro(chrono::Local::now().date_naive());
                self.stats_dirty = true;
                let auto_complete = self.config.pomodoro.auto_complete_tasks;
                let already_credited = self.work_credited;
                let first_open = self.tasks.iter_mut().find(|task| !task.done);
                if let Some(task) = first_open.filter(|_| !already_credited) {
                    task.completed_pomodoros += 1;
                    if auto_complete && task.completed_pomodoros >= task.estimated_pomodoros {
                        task.done = true;
                    }
                }
                let interval = self.config.pomodoro.long_break_interval.max(1);
                let next = if self.completed_pomodoros.is_multiple_of(interval) {
                    PomodoroState::LongBreak
                } else {
                    PomodoroState::ShortBreak
                };
                let break_name = next.label().to_lowercase();
                if self.config.pomodoro.auto_start_break {
                    self.start_phase(next);
                    self.notify("Pomodoro finished", &format!("Starting a {break_name}"));
                } else {
                    self.prepare_phase(next);
                    self.notify(
                        "Pomodoro finished",
                        &format!("Time for a {break_name}. Start it when you are ready"),
                    );
                }
            }
            _ => {
                self.stop_pomodoro();
                self.idle_since = Some(Instant::now());
                self.notify("Break finished", "Ready for the next pomodoro");
            }
        }
    }

    /// Fire-and-forget desktop notification; failures (no daemon, etc.) are ignored.
    fn notify(&self, summary: &str, body: &str) {
        if !self.config.pomodoro.notifications {
            return;
        }
        let (summary, body) = (summary.to_string(), body.to_string());
        // Off the UI thread: talking to the notification daemon can block.
        thread::spawn(move || {
            let _ = notify_rust::Notification::new()
                .appname("ptc")
                .summary(&summary)
                .body(&body)
                .show();
        });
    }

    /// Text of the status bar below the clock.
    fn status_text(&self) -> String {
        let label = self.pomodoro_state.label();
        if self.awaiting_break_start {
            format!("{label} (ready)")
        } else if self.is_paused() {
            format!("{label} (paused)")
        } else if let Some(since) = self
            .idle_since
            .filter(|_| self.pomodoro_state == PomodoroState::Idle)
        {
            format!("{label} for {}", format_elapsed(since.elapsed()))
        } else {
            label.to_string()
        }
    }

    fn status_color(&self) -> Color {
        let theme = &self.config.theme;
        match self.pomodoro_state {
            PomodoroState::Idle => theme.status_idle,
            PomodoroState::Work => theme.status_work,
            PomodoroState::ShortBreak => theme.status_short_break,
            PomodoroState::LongBreak => theme.status_long_break,
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            self.update_pomodoro();
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
            // Before the plugins, so the ids are part of the first `tasks_changed`.
            self.sync_stats();
            self.sync_plugins();
        }
        Ok(())
    }

    fn next_task_id(&mut self) -> u64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        self.last_task_id = nanos.max(self.last_task_id + 1);
        self.last_task_id
    }

    /// Gives ids to tasks that have none, updates the statistics from the task list and saves them
    /// when they changed.
    fn sync_stats(&mut self) {
        for i in 0..self.tasks.len() {
            if self.tasks[i].id == 0 {
                self.tasks[i].id = self.next_task_id();
            }
        }
        if self.stats.observe_tasks(&self.tasks) {
            self.stats_dirty = true;
        }
        if self.stats_dirty {
            self.stats_dirty = false;
            if let Some(path) = &self.stats_path
                && let Err(err) = self.stats.save(path)
            {
                self.stats_error = Some(format!("stats: could not save {}: {err}", path.display()));
            }
        }
    }

    fn pomodoro_snapshot(&self) -> PomodoroSnapshot {
        PomodoroSnapshot {
            state: self.pomodoro_state,
            paused: self.is_paused(),
            waiting: self.awaiting_break_start,
            current_task: self.tasks.iter().find(|task| !task.done).cloned(),
        }
    }

    /// Emits `startup` and loads the tasks plugins provide. No `tasks_changed` for that load.
    fn start_plugins(&mut self) {
        if let Some(plugins) = &self.plugins {
            plugins.emit_empty("startup");
        }
        self.apply_plugin_config();
        self.apply_plugin_tasks();
        self.last_tasks = self.tasks.clone();
        self.last_pomodoro = self.pomodoro_snapshot();
    }

    /// Saves what is pending.
    fn shutdown_stats(&mut self) {
        self.sync_stats();
    }

    fn shutdown_plugins(&mut self) {
        self.sync_plugins();
        if let Some(plugins) = &self.plugins {
            plugins.emit_empty("quit");
        }
    }

    /// Emits events for whatever changed since the last call.
    fn sync_plugins(&mut self) {
        let snapshot = self.pomodoro_snapshot();
        if let Some(plugins) = &self.plugins {
            if snapshot != self.last_pomodoro {
                plugins.emit(
                    "pomodoro_state_changed",
                    &PomodoroEvent {
                        state: snapshot.state.name(),
                        previous: self.last_pomodoro.state.name(),
                        paused: snapshot.paused,
                        waiting: snapshot.waiting,
                        completed_pomodoros: self.completed_pomodoros,
                        ends_at: self.phase_ends_at.map(|_| {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default();
                            (now + self.remaining()).as_secs_f64().ceil() as u64
                        }),
                        remaining_seconds: self.remaining().as_secs_f64().ceil() as u64,
                        current_task: snapshot.current_task.clone(),
                        idle_since: self.idle_since.map(|since| {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default();
                            now.saturating_sub(since.elapsed()).as_secs()
                        }),
                        changed: snapshot.changes_from(&self.last_pomodoro),
                    },
                );
            }
            if self.tasks != self.last_tasks {
                plugins.emit("tasks_changed", &self.tasks);
            }
        }
        self.last_pomodoro = snapshot;
        self.last_tasks = self.tasks.clone();
        // Replacing the tasks here is not reported back as a change.
        self.apply_plugin_config();
        self.apply_plugin_tasks();
        self.last_tasks = self.tasks.clone();
    }

    /// Merges the config patches plugins sent with `ptc.config`. A bad patch is
    /// reported as a plugin error and ignored as a whole.
    fn apply_plugin_config(&mut self) {
        let Some(plugins) = &self.plugins else {
            return;
        };
        for patch in plugins.take_pending_config() {
            match merge_config(&self.config, &patch) {
                Ok(config) => self.config = config,
                Err(err) => plugins.report(format!("ptc.config: {err}")),
            }
        }
    }

    fn apply_plugin_tasks(&mut self) {
        let Some(mut tasks) = self.plugins.as_ref().and_then(|p| p.take_pending_tasks()) else {
            return;
        };
        for task in &mut tasks {
            task.estimated_pomodoros = task.estimated_pomodoros.max(MIN_ESTIMATED_POMODOROS);
        }
        self.tasks = tasks;
        self.clamp_selection();
    }

    fn draw(&mut self, frame: &mut Frame) {
        let theme = &self.config.theme;
        frame.render_widget(
            Block::new().style(Style::new().fg(theme.foreground).bg(theme.background)),
            frame.area(),
        );

        let [left, right] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Fill(1)])
                .areas(frame.area());

        self.draw_sidebar(frame, left);
        if matches!(self.mode, AppMode::Stats) {
            stats_view::draw(
                frame,
                right,
                &self.config.theme,
                &self.stats,
                self.preset.as_deref(),
                chrono::Local::now().date_naive(),
            );
        } else {
            self.draw_tasks(frame, right);
        }
    }

    /// Animated `✻` (like Claude Code's spinner), still while the pomodoro is idle.
    fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 10] = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
        if self.pomodoro_state == PomodoroState::Idle {
            return "✻";
        }
        let step = self.started_at.elapsed().as_millis() / 120;
        FRAMES[step as usize % FRAMES.len()]
    }

    /// A task row: the marker in its theme color, the spinner slot (the glyph on the
    /// current task, blank on the others so the texts stay aligned), then the text.
    fn task_line(&self, done: bool, text: String, spinner: Option<&'static str>) -> Line<'static> {
        let theme = &self.config.theme;
        // A 3-column colored block; completed ones carry a bold `x` in the middle.
        let (marker, style) = if done {
            let style = Style::new()
                .fg(theme.task_marked_text)
                .bg(theme.task_marked)
                .bold();
            (" x ", style)
        } else {
            ("   ", Style::new().bg(theme.task_unmarked))
        };
        Line::from(vec![
            Span::styled(marker, style),
            Span::raw(" "),
            Span::styled(spinner.unwrap_or(" "), Style::new().fg(theme.task_spinner)),
            Span::raw(format!(" {text}")),
        ])
    }

    /// Time to finish the pomodoros still estimated on open tasks, including the
    /// breaks between them (the cadence continues from the pomodoros already done).
    fn estimated_time_left(&self) -> Duration {
        let pending: u32 = self
            .tasks
            .iter()
            .filter(|task| !task.done)
            .map(|task| {
                task.estimated_pomodoros
                    .saturating_sub(task.completed_pomodoros)
            })
            .sum();

        let interval = self.config.pomodoro.long_break_interval.max(1);
        let mut total = self.phase_duration(PomodoroState::Work) * pending;
        // A break follows every pomodoro except the last one.
        for i in 1..pending {
            let state = if (self.completed_pomodoros + i).is_multiple_of(interval) {
                PomodoroState::LongBreak
            } else {
                PomodoroState::ShortBreak
            };
            total += self.phase_duration(state);
        }
        total
    }

    fn draw_tasks(&mut self, frame: &mut Frame, area: Rect) {
        let theme = &self.config.theme;
        let [area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        let minutes = self.estimated_time_left().as_secs() / 60;
        let estimate = if minutes >= 60 {
            format!("{}h {:02}m", minutes / 60, minutes % 60)
        } else {
            format!("{minutes}m")
        };
        let tasks_count = match self.tasks.len() {
            1 => "1 Task".to_string(),
            n => format!("{n} Tasks"),
        };
        let pomodoros_count = match self.completed_pomodoros {
            1 => "1 Completed Pomodoro".to_string(),
            n => format!("{n} Completed Pomodoros"),
        };
        let count = format!("{tasks_count} · {pomodoros_count}");
        let finish_at = (chrono::Local::now() + self.estimated_time_left()).format("%H:%M");
        let bar_style = Style::new()
            .fg(theme.tasks_status_bar_text)
            .bg(theme.tasks_status_bar);
        let bar_area = Block::new()
            .padding(Padding::new(2, 2, 0, 0))
            .inner(status_area);
        frame.render_widget(Block::new().style(bar_style), status_area);
        frame.render_widget(Paragraph::new(count).style(bar_style), bar_area);
        frame.render_widget(
            Paragraph::new(if minutes == 0 {
                format!("Estimated: {estimate}")
            } else {
                format!("Estimated: {estimate} · Finish at {finish_at}")
            })
            .right_aligned()
            .style(bar_style),
            bar_area,
        );
        let block = Block::new()
            .style(Style::new().bg(self.config.theme.panel_background))
            .padding(Padding::new(2, 2, 1, 1));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let [title_area, tasks_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);
        let theme = &self.config.theme;
        let mut title = vec![Span::styled(
            "TASKS",
            Style::new().fg(theme.tasks_title).bold(),
        )];
        if let Some(preset) = &self.preset {
            title.push(Span::styled(
                format!("  ·  preset {preset}"),
                Style::new().fg(theme.tasks_title_dim),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(title)), title_area);

        let editing = matches!(self.mode, AppMode::TaskTextInput);
        // The current task is the first one that is not done.
        let current = self.tasks.iter().position(|task| !task.done);
        let mut lines: Vec<Line> = self
            .tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let spinner = (current == Some(i)).then(|| self.spinner_frame());
                if editing && self.editing_task == Some(i) {
                    self.task_line(task.done, format!("{}█", self.input_buffer), spinner)
                } else {
                    self.task_line(task.done, task.text.clone(), spinner)
                }
            })
            .collect();

        let adding = editing && self.editing_task.is_none();
        if adding {
            lines.push(self.task_line(false, format!("{}█", self.input_buffer), None));
        }

        // Scroll just enough to keep the cursor row (selected task or new task) visible.
        let height = tasks_area.height as usize;
        let cursor = if adding {
            self.tasks.len()
        } else {
            self.selected_task
        };
        if cursor < self.scroll_offset {
            self.scroll_offset = cursor;
        } else if height > 0 && cursor >= self.scroll_offset + height {
            self.scroll_offset = cursor + 1 - height;
        }
        self.scroll_offset = self.scroll_offset.min(lines.len().saturating_sub(height));

        let row_at = |index: usize| {
            Rect::new(
                tasks_area.x,
                tasks_area.y + (index - self.scroll_offset) as u16,
                tasks_area.width,
                1,
            )
        };

        // Highlight the whole row (up to the panel's right padding), not just the text.
        if !self.tasks.is_empty() && !adding {
            frame.render_widget(
                Block::new().style(Style::new().bg(self.config.theme.selection)),
                row_at(self.selected_task),
            );
        }

        // completed/estimated on every visible task, except the one being edited.
        let visible = self.scroll_offset..(self.scroll_offset + height).min(self.tasks.len());
        for index in visible {
            if editing && self.editing_task == Some(index) {
                continue;
            }
            let task = &self.tasks[index];
            let counter = format!("{}/{}", task.completed_pomodoros, task.estimated_pomodoros);
            frame.render_widget(Paragraph::new(counter).right_aligned(), row_at(index));
        }

        frame.render_widget(
            Paragraph::new(lines).scroll((self.scroll_offset as u16, 0)),
            tasks_area,
        );

        // Only when the list overflows; drawn in the panel's right padding column.
        let total = self.tasks.len() + usize::from(adding);
        if total > height {
            let scrollbar_area = Rect::new(tasks_area.right(), tasks_area.y, 1, tasks_area.height);
            let mut scrollbar_state = ScrollbarState::new(total - height + 1)
                .viewport_content_length(height)
                .position(self.scroll_offset);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::new().fg(self.config.theme.scrollbar)),
                scrollbar_area,
                &mut scrollbar_state,
            );
        }
    }

    /// The hotkey hints: the key in its colour, then the arrow and the label.
    fn render_hotkeys(
        &self,
        frame: &mut Frame,
        hotkeys: Vec<(KeyBinding, &'static str)>,
        area: Rect,
    ) {
        // Pad the keys to the widest one so every label starts at the same column.
        let key_width = hotkeys
            .iter()
            .map(|(key, _)| key.to_string().chars().count())
            .max()
            .unwrap_or(0);
        let hotkeys: Vec<Line> = hotkeys
            .into_iter()
            .map(|(key, label)| {
                let theme = &self.config.theme;
                Line::from(vec![
                    Span::styled(
                        format!("{:<key_width$}", key.to_string()),
                        Style::new().fg(theme.hotkey_key),
                    ),
                    Span::styled(format!(" → {label}"), Style::new().fg(theme.hotkey)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(hotkeys), area);
    }

    fn draw_sidebar(&self, frame: &mut Frame, area: Rect) {
        let inner = Block::new().padding(Padding::new(3, 3, 1, 1)).inner(area);
        let [title_area, clock_area, _, status_area, _, hotkeys_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(6),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(inner);

        frame.render_widget(
            Paragraph::new("POMODORO TIME CHAMBER")
                .style(Style::new().fg(self.config.theme.title).bold()),
            title_area,
        );

        // Round up so the clock shows 00:01 until the last second has passed.
        let clock = clock_lines(Duration::from_secs(
            self.remaining().as_secs_f64().ceil() as u64
        ));
        frame.render_widget(
            Paragraph::new(clock).style(Style::new().fg(self.status_color())),
            clock_area,
        );

        let status_style = Style::new()
            .fg(self.config.theme.background)
            .bg(self.status_color());
        frame.render_widget(
            Paragraph::new(self.status_text())
                .centered()
                .style(status_style),
            status_area,
        );

        let keyboard = &self.config.keyboard;
        if self.quit_pending {
            let [question_area, answers_area] =
                Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(hotkeys_area);
            frame.render_widget(
                Paragraph::new("Quit ptc?")
                    .style(Style::new().fg(self.config.theme.quit_prompt).bold()),
                question_area,
            );
            let answers = vec![
                (keyboard.confirm_quit, "Yes, quit"),
                (keyboard.cancel_quit, "No, stay"),
            ];
            return self.render_hotkeys(frame, answers, answers_area);
        }
        let hotkeys = match self.mode {
            AppMode::Normal | AppMode::Stats => {
                let stats_open = matches!(self.mode, AppMode::Stats);
                let idle = self.pomodoro_state == PomodoroState::Idle;
                let mut hotkeys = vec![(
                    keyboard.toggle_pomodoro,
                    if idle {
                        "Start pomodoro"
                    } else if self.awaiting_break_start {
                        "Start break"
                    } else {
                        "Stop pomodoro"
                    },
                )];
                if !idle && !self.awaiting_break_start {
                    hotkeys.push((
                        keyboard.pause_pomodoro,
                        if self.is_paused() {
                            "Resume pomodoro"
                        } else {
                            "Pause pomodoro"
                        },
                    ));
                }
                if self.in_break() {
                    hotkeys.push((keyboard.skip_break, "Skip break"));
                }
                if stats_open {
                    // Only the pomodoro controls work on this page.
                    hotkeys.insert(0, (keyboard.close_stats, "Back to the tasks"));
                    hotkeys.push((keyboard.quit, "Quit"));
                    return self.render_hotkeys(frame, hotkeys, hotkeys_area);
                }
                hotkeys.extend([
                    (keyboard.add_task, "Add new task"),
                    (keyboard.edit_task, "Edit task"),
                    (keyboard.toggle_done, "Toggle done"),
                    (keyboard.delete_task, "Delete task"),
                    (keyboard.clear_completed, "Clear completed tasks"),
                    (keyboard.clear_all, "Clear all tasks"),
                    (keyboard.increase_estimate, "Increase estimate"),
                    (keyboard.decrease_estimate, "Decrease estimate"),
                    (keyboard.select_next, "Next task"),
                    (keyboard.select_previous, "Previous task"),
                    (keyboard.move_task_down, "Move task down"),
                    (keyboard.move_task_up, "Move task up"),
                    (keyboard.show_stats, "Statistics"),
                    (keyboard.quit, "Quit"),
                ]);
                hotkeys
            }
            AppMode::TaskTextInput => vec![
                (keyboard.confirm_task, "Save task"),
                (keyboard.cancel_task, "Cancel"),
            ],
        };
        self.render_hotkeys(frame, hotkeys, hotkeys_area);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if !event::poll(Duration::from_millis(100))? {
            return Ok(());
        }

        let Event::Key(key) = event::read()? else {
            return Ok(());
        };
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        self.handle_key(key);
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let keyboard = &self.config.keyboard;

        // While the question is on screen it takes every key: the quit key again also means yes.
        if self.quit_pending {
            if keyboard.confirm_quit.matches(&key) || keyboard.quit.matches(&key) {
                self.should_quit = true;
            } else if keyboard.cancel_quit.matches(&key) || keyboard.cancel_task.matches(&key) {
                self.quit_pending = false;
            }
            return;
        }

        match self.mode {
            AppMode::Normal => match key.code {
                _ if keyboard.quit.matches(&key) => self.quit_pending = true,
                _ if keyboard.toggle_pomodoro.matches(&key) => self.action_toggle_pomodoro(),
                _ if keyboard.pause_pomodoro.matches(&key) => self.action_pause_pomodoro(),
                _ if keyboard.skip_break.matches(&key) => self.action_skip_break(),
                _ if keyboard.add_task.matches(&key) => self.action_start_add_task(),
                _ if keyboard.edit_task.matches(&key) => self.action_start_edit_task(),
                _ if keyboard.toggle_done.matches(&key) => self.action_toggle_done(),
                _ if keyboard.delete_task.matches(&key) => self.action_delete_task(),
                _ if keyboard.clear_completed.matches(&key) => self.action_clear_completed(),
                _ if keyboard.clear_all.matches(&key) => self.action_clear_all(),
                _ if keyboard.increase_estimate.matches(&key) => self.action_increase_estimate(),
                _ if keyboard.decrease_estimate.matches(&key) => self.action_decrease_estimate(),
                _ if keyboard.select_next.matches(&key) => self.action_select_next(),
                _ if keyboard.select_previous.matches(&key) => self.action_select_previous(),
                _ if keyboard.move_task_down.matches(&key) => self.action_move_task_down(),
                _ if keyboard.move_task_up.matches(&key) => self.action_move_task_up(),
                _ if keyboard.show_stats.matches(&key) => self.mode = AppMode::Stats,
                _ => {}
            },
            // The page is only for looking at; the pomodoro keeps working underneath it.
            AppMode::Stats => match key.code {
                _ if keyboard.close_stats.matches(&key) || keyboard.show_stats.matches(&key) => {
                    self.mode = AppMode::Normal
                }
                _ if keyboard.quit.matches(&key) => self.quit_pending = true,
                _ if keyboard.toggle_pomodoro.matches(&key) => self.action_toggle_pomodoro(),
                _ if keyboard.pause_pomodoro.matches(&key) => self.action_pause_pomodoro(),
                _ if keyboard.skip_break.matches(&key) => self.action_skip_break(),
                _ => {}
            },
            AppMode::TaskTextInput => match key.code {
                _ if keyboard.confirm_task.matches(&key) => self.action_confirm_task(),
                _ if keyboard.cancel_task.matches(&key) => self.action_cancel_task(),
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input_buffer.push(c)
                }
                _ => {}
            },
        }
    }

    fn action_toggle_pomodoro(&mut self) {
        if self.pomodoro_state == PomodoroState::Idle {
            self.start_phase(PomodoroState::Work);
        } else if self.awaiting_break_start {
            self.start_phase(self.pomodoro_state);
        } else {
            self.stop_pomodoro();
        }
    }

    fn action_select_next(&mut self) {
        if self.selected_task + 1 < self.tasks.len() {
            self.selected_task += 1;
        }
    }

    fn action_move_task_down(&mut self) {
        if self.selected_task + 1 < self.tasks.len() {
            self.tasks.swap(self.selected_task, self.selected_task + 1);
            self.selected_task += 1;
        }
    }

    fn action_move_task_up(&mut self) {
        if self.selected_task > 0 && self.selected_task < self.tasks.len() {
            self.tasks.swap(self.selected_task, self.selected_task - 1);
            self.selected_task -= 1;
        }
    }

    fn action_select_previous(&mut self) {
        self.selected_task = self.selected_task.saturating_sub(1);
    }

    fn action_delete_task(&mut self) {
        if self.selected_task < self.tasks.len() {
            self.tasks.remove(self.selected_task);
            self.clamp_selection();
        }
    }

    fn action_clear_completed(&mut self) {
        self.tasks.retain(|task| !task.done);
        self.clamp_selection();
    }

    fn action_clear_all(&mut self) {
        self.tasks.clear();
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        self.selected_task = self.selected_task.min(self.tasks.len().saturating_sub(1));
    }

    /// Marking a task done once more than half of the running work phase has passed credits that
    /// pomodoro to it (and only once: the end of the phase then credits nobody).
    fn action_toggle_done(&mut self) {
        let credit = self.work_past_half() && !self.work_credited;
        let Some(task) = self.tasks.get_mut(self.selected_task) else {
            return;
        };
        task.done = !task.done;
        if task.done && credit {
            task.completed_pomodoros += 1;
            self.work_credited = true;
        }
    }

    fn action_increase_estimate(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            task.estimated_pomodoros += 1;
        }
    }

    fn action_decrease_estimate(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            task.estimated_pomodoros = task
                .estimated_pomodoros
                .saturating_sub(1)
                .max(MIN_ESTIMATED_POMODOROS);
        }
    }

    fn action_skip_break(&mut self) {
        if self.in_break() {
            self.stop_pomodoro();
        }
    }

    fn action_pause_pomodoro(&mut self) {
        if self.pomodoro_state == PomodoroState::Idle || self.awaiting_break_start {
            return;
        }
        match self.paused_remaining.take() {
            Some(remaining) => self.phase_ends_at = Some(Instant::now() + remaining),
            None => {
                self.paused_remaining = Some(self.remaining());
                self.phase_ends_at = None;
            }
        }
    }

    fn action_start_add_task(&mut self) {
        self.input_buffer.clear();
        self.editing_task = None;
        self.mode = AppMode::TaskTextInput;
    }

    fn action_start_edit_task(&mut self) {
        let Some(task) = self.tasks.get(self.selected_task) else {
            return;
        };
        self.input_buffer = task.text.clone();
        self.editing_task = Some(self.selected_task);
        self.mode = AppMode::TaskTextInput;
    }

    fn action_confirm_task(&mut self) {
        let text = self.input_buffer.trim();
        if !text.is_empty() {
            match self.editing_task {
                Some(i) => self.tasks[i].text = text.to_string(),
                None => {
                    self.tasks.push(Task {
                        id: 0,
                        text: text.to_string(),
                        estimated_pomodoros: MIN_ESTIMATED_POMODOROS,
                        completed_pomodoros: 0,
                        done: false,
                    });
                    self.selected_task = self.tasks.len() - 1;
                }
            }
        }
        self.action_cancel_task();
    }

    fn action_cancel_task(&mut self) {
        self.input_buffer.clear();
        self.editing_task = None;
        self.mode = AppMode::Normal;
    }
}

/// `MM:SS`, or `H:MM:SS` from one hour on.
fn format_elapsed(elapsed: Duration) -> String {
    let total = elapsed.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Renders `MM:SS` as big ASCII art, one `Line` per art row.
fn clock_lines(duration: Duration) -> Vec<Line<'static>> {
    let total_seconds = duration.as_secs();
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let clock_text = format!("{:02}:{:02}", minutes, seconds);

    let width = number_ascii_art::NUMBER_WIDTH as usize;
    let mut rows: Vec<String> = Vec::new();
    let mut last_ch = None;

    for ch in clock_text.chars() {
        let art = match ch {
            ':' => number_ascii_art::NUMBER_SEPARATOR_ASCII_ART,
            n => number_ascii_art::NUMBER_ASCII_ART[n.to_digit(10).unwrap() as usize],
        };

        // Two columns between digits, none next to the separator.
        let gap = if last_ch.is_some() && last_ch != Some(':') && ch != ':' {
            "  "
        } else {
            ""
        };

        for (i, line) in art.lines().enumerate() {
            if rows.len() <= i {
                rows.push(String::new());
            }
            rows[i].push_str(gap);
            // Trim before padding: a stray trailing space in the art would push the rest of the
            // row to the right and misalign the colon and the following digits.
            rows[i].push_str(&format!("{:<width$}", line.trim_end()));
        }

        last_ch = Some(ch);
    }

    rows.into_iter().map(Line::from).collect()
}

const USAGE: &str = "\
Terminal pomodoro timer with a task list

Usage: ptc [--preset <name>]

Options:
  --preset <name>  Use the preset <name>: its own tasks (tasks.<name>.json), the plugins named
                   <file>.<name>.lua on top of the shared ones, and `ptc.preset` in Lua plugins.
                   Names are letters, digits, `-` and `_`.
  -h, --help       Print this help
  -V, --version    Print the version";

/// What the command line asks for.
#[derive(Debug, PartialEq)]
enum Cli {
    Run { preset: Option<String> },
    Help,
    Version,
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, String> {
    let mut preset = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "-h" | "--help" => return Ok(Cli::Help),
            "-V" | "--version" => return Ok(Cli::Version),
            "--preset" => args
                .next()
                .ok_or("--preset needs a name, e.g. `ptc --preset job`")?,
            other => match other.strip_prefix("--preset=") {
                Some(name) => name.to_string(),
                None => return Err(format!("unknown argument `{other}`")),
            },
        };
        // The name ends up in file names (tasks.<name>.json, <file>.<name>.lua).
        let valid = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(format!(
                "invalid preset name `{value}`: use only letters, digits, `-` and `_`"
            ));
        }
        preset = Some(value);
    }
    Ok(Cli::Run { preset })
}

fn main() -> io::Result<()> {
    let preset = match parse_args(std::env::args().skip(1)) {
        Ok(Cli::Run { preset }) => preset,
        Ok(Cli::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Ok(Cli::Version) => {
            println!("ptc {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Err(message) => {
            eprintln!("ptc: {message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    let mut app = App::new(Config::default());
    app.preset = preset.clone();
    let mut startup_notes = Vec::new();

    if let Some(dirs) = directories::ProjectDirs::from("", "", "ptc") {
        fs::create_dir_all(dirs.data_dir())?;
        // Like the tasks, the statistics are kept per preset: stats.json or stats.<preset>.json.
        let stats_path = dirs.data_dir().join(match &preset {
            Some(preset) => format!("stats.{preset}.json"),
            None => "stats.json".to_string(),
        });
        let (stats, note) = Stats::load(&stats_path);
        startup_notes.extend(note);
        app.stats = stats;
        app.stats_path = Some(stats_path);
        let plugins_dir = dirs.config_dir().join("plugins");
        // Read by other programs (e.g. a Neovim statusline), so the path is predictable.
        let status_file = std::env::var_os("PTC_STATUS_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // The tmux and Neovim helpers fall back to /tmp too, so keep that on Unix.
                let dir = std::env::var_os("XDG_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        if cfg!(windows) {
                            std::env::temp_dir()
                        } else {
                            PathBuf::from("/tmp")
                        }
                    });
                dir.join("ptc-status.json")
            });
        match PluginHost::new(
            dirs.data_dir(),
            &plugins_dir,
            &status_file,
            preset.as_deref(),
        ) {
            Ok(host) => {
                host.load_user_plugins();
                app.plugins = Some(host);
            }
            Err(err) => eprintln!("plugins disabled: {err}"),
        }
    }
    app.start_plugins();

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    app.shutdown_plugins();
    ratatui::restore();

    // Problems are only shown now, so they don't garble the TUI.
    app.shutdown_stats();
    for note in startup_notes.into_iter().chain(app.stats_error.take()) {
        eprintln!("{note}");
    }
    if let Some(plugins) = &app.plugins {
        for error in plugins.take_errors() {
            eprintln!("{error}");
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_emits_plugin_events_on_changes() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp/app");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.load_source(
            "recorder",
            r#"
            log = {}
            ptc.on("startup", function() table.insert(log, "startup") end)
            ptc.on("tasks_changed", function(t) table.insert(log, "tasks:" .. #t) end)
            ptc.on("pomodoro_state_changed", function(e)
                table.insert(log, e.previous .. ">" .. e.state)
                -- Only a counting-down phase has an end timestamp.
                assert((e.ends_at ~= nil) == (e.state == "work"))
                assert(e.remaining_seconds > 0)
            end)
            "#,
        );
        let mut app = App::new(Config::default());
        app.plugins = Some(host);
        app.start_plugins();

        app.action_toggle_pomodoro();
        app.sync_plugins();
        app.tasks.push(Task {
            id: 0,
            text: "a".into(),
            estimated_pomodoros: 1,
            completed_pomodoros: 0,
            done: false,
        });
        app.sync_plugins();
        app.sync_plugins(); // nothing changed: no new events
        app.action_toggle_pomodoro();
        app.sync_plugins();

        let log: String = app
            .plugins
            .as_ref()
            .unwrap()
            .eval_for_test(r#"return table.concat(log, ",")"#);
        // Adding the first task makes it the current one, which is also reported.
        assert_eq!(log, "startup,idle>work,work>work,tasks:1,work>idle");
        assert!(app.plugins.as_ref().unwrap().take_errors().is_empty());
    }

    fn app_with_plugin(name: &str, source: &str) -> App {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-tmp")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let host = PluginHost::new(&dir, &dir, &dir.join("ptc-status.json"), None).unwrap();
        host.load_source("test", source);
        let mut app = App::new(Config::default());
        app.plugins = Some(host);
        app.start_plugins();
        app
    }

    #[test]
    fn default_config_survives_a_json_round_trip() {
        let config = Config::default();
        let json = serde_json::to_value(&config).unwrap();
        let again = merge_config(&config, &serde_json::json!({})).unwrap();
        assert_eq!(json, serde_json::to_value(&again).unwrap());
    }

    #[test]
    fn plugins_merge_config_including_theme_and_keys() {
        let app = app_with_plugin(
            "config_ok",
            r##"
            ptc.config({
                theme = { background = "#112233", status_work = "magenta" },
                pomodoro = { work_minutes = 30, auto_start_break = true },
                keyboard = { quit = "Ctrl+q", add_task = "n" },
            })
            ptc.config({ pomodoro = { short_break_minutes = 10 } })
            "##,
        );
        let config = &app.config;
        assert_eq!(config.theme.background, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(config.theme.status_work, Color::Magenta);
        // Untouched values keep their defaults.
        assert_eq!(config.theme.status_short_break, Color::Green);
        assert_eq!(config.pomodoro.work_minutes, 30);
        assert_eq!(config.pomodoro.short_break_minutes, 10);
        assert_eq!(config.pomodoro.long_break_minutes, 15);
        assert!(config.pomodoro.auto_start_break);
        assert_eq!(config.keyboard.quit.to_string(), "Ctrl+q");
        assert_eq!(config.keyboard.add_task.to_string(), "n");
        assert!(app.plugins.as_ref().unwrap().take_errors().is_empty());
    }

    #[test]
    fn bad_config_patches_are_rejected_whole_and_reported() {
        let app = app_with_plugin(
            "config_bad",
            r##"
            ptc.config({ pomodoro = { work_minutes = 50 }, theme = { background = "not-a-color" } })
            ptc.config({ theme = { nope = "red" } })
            ptc.config({ keyboard = { quit = "ctrl+" } })
            "##,
        );
        assert_eq!(app.config.pomodoro.work_minutes, 25);
        assert_eq!(app.plugins.as_ref().unwrap().take_errors().len(), 3);
    }

    #[test]
    fn key_bindings_parse_and_match() {
        let ctrl_j: KeyBinding = "CTRL+j".parse().unwrap();
        let event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert!(ctrl_j.matches(&event));
        let plain_j: KeyBinding = "j".parse().unwrap();
        assert!(!plain_j.matches(&event));
        assert_eq!("Enter".parse::<KeyBinding>().unwrap().code, KeyCode::Enter);
        assert_eq!(
            "space".parse::<KeyBinding>().unwrap().code,
            KeyCode::Char(' ')
        );
        assert_eq!("F5".parse::<KeyBinding>().unwrap().code, KeyCode::F(5));
        assert_eq!(
            "Ctrl++".parse::<KeyBinding>().unwrap().code,
            KeyCode::Char('+')
        );
        assert!("".parse::<KeyBinding>().is_err());
        assert!("bogus".parse::<KeyBinding>().is_err());
    }

    /// Finishes one work phase right away and returns to idle.
    fn finish_pomodoro(app: &mut App) {
        app.action_toggle_pomodoro();
        app.phase_ends_at = Some(Instant::now() - Duration::from_secs(1));
        app.update_pomodoro();
        app.stop_pomodoro();
    }

    fn app_with_one_task(auto_complete_tasks: bool) -> App {
        let mut config = Config::default();
        config.pomodoro.notifications = false;
        config.pomodoro.auto_complete_tasks = auto_complete_tasks;
        let mut app = App::new(config);
        app.tasks.push(Task {
            id: 0,
            text: "a".into(),
            estimated_pomodoros: 2,
            completed_pomodoros: 0,
            done: false,
        });
        app
    }

    #[test]
    fn tasks_keep_counting_past_the_estimate_by_default() {
        let mut app = app_with_one_task(false);
        for _ in 0..3 {
            finish_pomodoro(&mut app);
        }
        assert_eq!(app.tasks[0].completed_pomodoros, 3);
        assert!(!app.tasks[0].done);
    }

    #[test]
    fn tasks_are_marked_done_at_the_estimate_when_enabled() {
        let mut app = app_with_one_task(true);
        finish_pomodoro(&mut app);
        assert!(!app.tasks[0].done);
        finish_pomodoro(&mut app);
        assert_eq!(app.tasks[0].completed_pomodoros, 2);
        assert!(app.tasks[0].done);
        // Done tasks are not credited any more.
        finish_pomodoro(&mut app);
        assert_eq!(app.tasks[0].completed_pomodoros, 2);
    }

    fn app_with_two_tasks() -> App {
        let mut config = Config::default();
        config.pomodoro.notifications = false;
        let mut app = App::new(config);
        for text in ["a", "b"] {
            app.tasks.push(Task {
                id: 0,
                text: text.into(),
                estimated_pomodoros: 3,
                completed_pomodoros: 0,
                done: false,
            });
        }
        app
    }

    /// Starts a work phase with `seconds_left` on the clock.
    fn start_work_with(app: &mut App, seconds_left: u64) {
        app.action_toggle_pomodoro();
        app.phase_ends_at = Some(Instant::now() + Duration::from_secs(seconds_left));
    }

    #[test]
    fn marking_done_after_half_of_the_work_credits_a_pomodoro_once() {
        let mut app = app_with_two_tasks();
        start_work_with(&mut app, 10 * 60); // 15 of 25 minutes passed
        app.action_toggle_done();
        assert!(app.tasks[0].done);
        assert_eq!(app.tasks[0].completed_pomodoros, 1);

        // Unmarking and marking again must not credit the same pomodoro twice.
        app.action_toggle_done();
        app.action_toggle_done();
        assert_eq!(app.tasks[0].completed_pomodoros, 1);

        // Finishing the phase credits nobody else: it was already counted.
        app.phase_ends_at = Some(Instant::now() - Duration::from_secs(1));
        app.update_pomodoro();
        assert_eq!(app.completed_pomodoros, 1);
        assert_eq!(app.tasks[1].completed_pomodoros, 0);
        assert_eq!(app.tasks[0].completed_pomodoros, 1);
    }

    #[test]
    fn marking_done_in_the_first_half_or_outside_work_credits_nothing() {
        let mut app = app_with_two_tasks();
        start_work_with(&mut app, 20 * 60); // only 5 of 25 minutes passed
        app.action_toggle_done();
        assert!(app.tasks[0].done);
        assert_eq!(app.tasks[0].completed_pomodoros, 0);

        // Unmarking never credits.
        app.stop_pomodoro();
        app.selected_task = 1;
        app.action_toggle_done(); // idle: no work phase running
        assert_eq!(app.tasks[1].completed_pomodoros, 0);

        // A phase that is not credited still credits the first open task when it ends.
        let mut app = app_with_two_tasks();
        start_work_with(&mut app, 20 * 60);
        app.phase_ends_at = Some(Instant::now() - Duration::from_secs(1));
        app.update_pomodoro();
        assert_eq!(app.tasks[0].completed_pomodoros, 1);
    }

    fn open_task(text: &str) -> Task {
        Task {
            id: 0,
            text: text.into(),
            estimated_pomodoros: 2,
            completed_pomodoros: 0,
            done: false,
        }
    }

    #[test]
    fn plugins_get_the_current_task_and_are_told_when_it_changes() {
        let mut app = app_with_plugin(
            "current_task",
            r#"
            log = {}
            ptc.on("pomodoro_state_changed", function(e)
                local current = e.current_task and e.current_task.text or "none"
                table.insert(log, current .. "[" .. table.concat(e.changed, "+") .. "]")
            end)
            "#,
        );
        let log = |app: &App| -> String {
            app.plugins
                .as_ref()
                .unwrap()
                .eval_for_test(r#"return table.concat(log, " ")"#)
        };

        // No task: no event. Adding "a" makes it current.
        app.sync_plugins();
        for text in ["a", "b", "c"] {
            app.tasks.push(open_task(text));
            app.sync_plugins();
        }
        assert_eq!(log(&app), "a[current_task]");

        // Reordering so that another task comes first changes the current task.
        app.selected_task = 0;
        app.action_move_task_down();
        app.sync_plugins();
        // Moving a task that is not first leaves the current one alone.
        app.selected_task = 2;
        app.action_move_task_up();
        app.sync_plugins();
        // Marking the current one done hands over to the next open task.
        app.selected_task = 0;
        app.action_toggle_done();
        app.sync_plugins();
        // Deleting the current one does too.
        app.action_delete_task();
        app.sync_plugins();
        // Editing the current task's estimate is a change of its data.
        app.action_increase_estimate();
        app.sync_plugins();
        // Clearing everything leaves no current task: the field is absent.
        app.action_clear_all();
        app.sync_plugins();
        assert_eq!(
            log(&app),
            "a[current_task] b[current_task] c[current_task] c[current_task] none[current_task]"
        );
        assert!(app.plugins.as_ref().unwrap().take_errors().is_empty());
    }

    #[test]
    fn status_shows_how_long_the_user_has_been_idle_after_a_break() {
        let mut config = Config::default();
        config.pomodoro.notifications = false;
        let mut app = App::new(config);
        assert_eq!(app.status_text(), "Idle");

        // A break that finishes by itself starts the idle timer.
        app.prepare_phase(PomodoroState::ShortBreak);
        app.action_toggle_pomodoro(); // start the break
        app.phase_ends_at = Some(Instant::now() - Duration::from_secs(1));
        app.update_pomodoro();
        assert!(app.pomodoro_state == PomodoroState::Idle);
        assert_eq!(app.status_text(), "Idle for 00:00");

        app.idle_since = Some(Instant::now() - Duration::from_secs(164));
        assert_eq!(app.status_text(), "Idle for 02:44");
        app.idle_since = Some(Instant::now() - Duration::from_secs(3725));
        assert_eq!(app.status_text(), "Idle for 1:02:05");

        // Starting a pomodoro clears it, and so does coming back to idle by hand.
        app.action_toggle_pomodoro();
        assert_eq!(app.status_text(), "Focus");
        app.action_toggle_pomodoro();
        assert_eq!(app.status_text(), "Idle");
    }

    #[test]
    fn skipping_a_break_does_not_start_the_idle_timer() {
        let mut app = App::new(Config::default());
        app.prepare_phase(PomodoroState::LongBreak);
        app.action_skip_break();
        assert_eq!(app.status_text(), "Idle");
    }

    #[test]
    fn every_clock_row_has_the_same_width() {
        for minutes in 0..60u64 {
            for seconds in 0..60u64 {
                let lines = clock_lines(Duration::from_secs(minutes * 60 + seconds));
                let widths: Vec<usize> = lines.iter().map(|line| line.width()).collect();
                assert!(
                    widths.iter().all(|&w| w == widths[0]),
                    "{minutes:02}:{seconds:02} has uneven rows: {widths:?}"
                );
            }
        }
    }

    #[test]
    fn plugins_get_idle_since_only_after_a_break_finished() {
        let mut app = app_with_plugin(
            "idle_since",
            r#"
            log = {}
            ptc.on("pomodoro_state_changed", function(e)
                table.insert(log, e.state .. ":" .. (e.idle_since and "since" or "none"))
            end)
            "#,
        );
        app.config.pomodoro.notifications = false;

        // Starting and finishing a break by itself.
        app.prepare_phase(PomodoroState::ShortBreak);
        app.action_toggle_pomodoro();
        app.sync_plugins();
        app.phase_ends_at = Some(Instant::now() - Duration::from_secs(1));
        app.update_pomodoro();
        app.sync_plugins();
        // Starting a pomodoro, then stopping it by hand: plain idle, no timer.
        app.action_toggle_pomodoro();
        app.sync_plugins();
        app.action_toggle_pomodoro();
        app.sync_plugins();

        let log: String = app
            .plugins
            .as_ref()
            .unwrap()
            .eval_for_test(r#"return table.concat(log, ",")"#);
        assert_eq!(log, "short_break:none,idle:since,work:none,idle:none");
        assert!(app.plugins.as_ref().unwrap().take_errors().is_empty());
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn command_line_arguments() {
        assert_eq!(parse_args(args(&[])), Ok(Cli::Run { preset: None }));
        assert_eq!(
            parse_args(args(&["--preset", "job"])),
            Ok(Cli::Run {
                preset: Some("job".into())
            })
        );
        assert_eq!(
            parse_args(args(&["--preset=side-project_2"])),
            Ok(Cli::Run {
                preset: Some("side-project_2".into())
            })
        );
        assert_eq!(parse_args(args(&["--help"])), Ok(Cli::Help));
        assert_eq!(parse_args(args(&["-V"])), Ok(Cli::Version));

        assert!(parse_args(args(&["--preset"])).is_err());
        assert!(parse_args(args(&["--preset="])).is_err());
        assert!(parse_args(args(&["--preset", "../etc"])).is_err());
        assert!(parse_args(args(&["--preset", "a.b"])).is_err());
        assert!(parse_args(args(&["--nope"])).is_err());
        assert!(parse_args(args(&["job"])).is_err());
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn the_stats_page_opens_and_closes_with_its_keys_and_only_pomodoro_keys_work_there() {
        let mut app = App::new(Config::default());
        press(&mut app, KeyCode::Char('i'));
        assert!(matches!(app.mode, AppMode::Stats));

        // Task keys do nothing on the page; the pomodoro keys still work.
        press(&mut app, KeyCode::Char('a'));
        assert!(matches!(app.mode, AppMode::Stats));
        press(&mut app, KeyCode::Char('s'));
        assert!(app.pomodoro_state == PomodoroState::Work);

        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, AppMode::Normal));

        // The same key opens and closes it, and quit works from the page.
        press(&mut app, KeyCode::Char('i'));
        press(&mut app, KeyCode::Char('i'));
        assert!(matches!(app.mode, AppMode::Normal));
        press(&mut app, KeyCode::Char('i'));
        press(&mut app, KeyCode::Char('q'));
        assert!(app.quit_pending && !app.should_quit, "quitting asks first");
        press(&mut app, KeyCode::Char('y'));
        assert!(app.should_quit);
    }

    #[test]
    fn quitting_needs_a_confirmation_that_can_be_cancelled() {
        let mut app = App::new(Config::default());

        // Cancelled with n, then with Esc.
        for cancel in [KeyCode::Char('n'), KeyCode::Esc] {
            press(&mut app, KeyCode::Char('q'));
            assert!(app.quit_pending);
            press(&mut app, cancel);
            assert!(!app.quit_pending && !app.should_quit);
        }

        // While asking, other keys do nothing (they must not reach the task list or the timer).
        press(&mut app, KeyCode::Char('q'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('s'));
        assert!(app.quit_pending && !app.should_quit);
        assert!(matches!(app.mode, AppMode::Normal));
        assert!(app.pomodoro_state == PomodoroState::Idle);

        // The quit key again confirms, like y.
        press(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn the_quit_question_replaces_the_hotkey_list_and_is_colored() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = App::new(Config::default());
        let render = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(130, 36)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            let buffer = terminal.backend().buffer().clone();
            let text: String = (0..36)
                .flat_map(|y| (0..40).map(move |x| (x, y)))
                .map(|(x, y)| buffer[(x, y)].symbol().to_string())
                .collect();
            (buffer, text)
        };

        let (_, text) = render(&mut app);
        assert!(text.contains("Add new task") && !text.contains("Quit ptc?"));

        app.quit_pending = true;
        let (buffer, text) = render(&mut app);
        for expected in ["Quit ptc?", "Yes, quit", "No, stay"] {
            assert!(text.contains(expected), "missing {expected:?}");
        }
        assert!(!text.contains("Add new task"));
        let question = (0..40)
            .flat_map(|x| (0..36).map(move |y| (x, y)))
            .map(|(x, y)| &buffer[(x, y)])
            .find(|cell| cell.symbol() == "Q")
            .unwrap();
        assert_eq!(question.fg, app.config.theme.quit_prompt);
    }

    #[test]
    fn tasks_get_unique_stable_ids() {
        let mut app = App::new(Config::default());
        for text in ["a", "b", "c"] {
            app.tasks.push(open_task(text));
        }
        app.sync_stats();
        let ids: Vec<u64> = app.tasks.iter().map(|t| t.id).collect();
        assert!(ids.iter().all(|&id| id != 0));
        assert!(ids[0] < ids[1] && ids[1] < ids[2]);
        app.sync_stats();
        assert_eq!(ids, app.tasks.iter().map(|t| t.id).collect::<Vec<_>>());
    }

    #[test]
    fn finished_pomodoros_and_wrong_estimates_are_saved_per_preset_file() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp/stats_app");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stats.job.json");

        let mut config = Config::default();
        config.pomodoro.notifications = false;
        let mut app = App::new(config);
        app.stats_path = Some(path.clone());
        app.tasks.push(Task {
            estimated_pomodoros: 1,
            ..open_task("slow task")
        });
        for _ in 0..3 {
            finish_pomodoro(&mut app);
        }
        app.sync_stats();

        let (saved, error) = Stats::load(&path);
        assert!(error.is_none());
        assert_eq!(saved.total_pomodoros(), 3);
        let worst = saved.worst_estimates(10);
        assert_eq!(
            (worst[0].text.as_str(), worst[0].estimated, worst[0].spent),
            ("slow task", 1, 3)
        );

        // Clearing the task does not forget it.
        app.action_clear_all();
        app.sync_stats();
        assert_eq!(app.stats.worst_estimates(10).len(), 1);
        assert!(app.stats_error.is_none());
    }

    #[test]
    fn the_stats_page_shows_the_numbers_in_the_theme_colors() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = App::new(Config::default());
        let today = chrono::Local::now().date_naive();
        app.stats.record_pomodoro(today);
        app.stats.observe_tasks(&[Task {
            id: 9,
            estimated_pomodoros: 1,
            completed_pomodoros: 4,
            ..open_task("the slow one")
        }]);
        app.mode = AppMode::Stats;
        app.preset = Some("job".into());

        let mut terminal = Terminal::new(TestBackend::new(130, 36)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let text: String = (0..36)
            .flat_map(|y| (0..130).map(move |x| (x, y)))
            .map(|(x, y)| buffer[(x, y)].symbol().to_string())
            .collect();
        for expected in [
            "STATISTICS",
            "preset job",
            "MOST WRONG ESTIMATES",
            "the slow one",
            "1→4",
            "+3",
            "LAST 15 DAYS",
        ] {
            assert!(text.contains(expected), "missing {expected:?}");
        }
        // The task list is not drawn on this page.
        assert!(!text.contains("TASKS"));

        // The title uses the theme color, not a default one.
        let title = (0..130)
            .map(|x| &buffer[(x, 1)])
            .find(|cell| cell.symbol() == "S")
            .unwrap();
        assert_eq!(title.fg, app.config.theme.stats_title);
    }

    #[test]
    fn the_tasks_title_is_colored_and_shows_the_preset() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = App::new(Config::default());
        let draw = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(130, 20)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            terminal.backend().buffer().clone()
        };
        let row = |buffer: &ratatui::buffer::Buffer, y: u16| -> String {
            (40..130)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        };

        let buffer = draw(&mut app);
        let line = row(&buffer, 1);
        assert!(
            line.contains("TASKS") && !line.contains("preset"),
            "{line:?}"
        );
        let title_cell = (40..130)
            .map(|x| &buffer[(x, 1)])
            .find(|cell| cell.symbol() == "T")
            .unwrap();
        assert_eq!(title_cell.fg, app.config.theme.tasks_title);

        app.preset = Some("job".into());
        let buffer = draw(&mut app);
        assert!(row(&buffer, 1).contains("TASKS  ·  preset job"));
        let preset_cell = (40..130)
            .map(|x| &buffer[(x, 1)])
            .find(|cell| cell.symbol() == "j")
            .unwrap();
        assert_eq!(preset_cell.fg, app.config.theme.tasks_title_dim);
    }
}
