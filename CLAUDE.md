# pomodoro-time-chamber

Terminal pomodoro + task list app in Rust (edition 2024), built on `ratatui` (crossterm backend).

## Commands
- `cargo build` / `cargo run`. The binary is named `ptc` (`[[bin]]` in `Cargo.toml`); the package name stays `pomodoro-time-chamber`.
- `cargo test`: plugin host tests (`src/plugins.rs`) and an App-level event test (`src/main.rs`); they write under `target/test-tmp/`.

## Layout
- `src/main.rs`: everything else (config, `App`, drawing, event handling, `clock_lines`).
- `src/plugins.rs`: Lua plugin host (`PluginHost`, built on `mlua` with vendored Lua 5.4).
- `plugins/bootstrap.lua`: defines `ptc.on` and the event dispatcher. `plugins/core_persist.lua`: the core plugin that saves/loads tasks. All core plugins (`CORE_PLUGINS` in `src/plugins.rs`) are embedded with `include_str!` and loaded before user plugins; add new always-on ones there.
- `plugins/core_status_file.lua`: always-on core plugin that writes the pomodoro status JSON to `ptc.status_file` (`$PTC_STATUS_FILE`, else `$XDG_RUNTIME_DIR/ptc-status.json`, else `/tmp`; computed in `main`). `contrib/nvim/lua/ptc.lua`: the Neovim-side module that reads that file (Neovim runs LuaJIT, so keep it Lua 5.1 compatible, e.g. no `//`).
- `src/number_ascii_art.rs`: big block-digit art for the clock (`NUMBER_ASCII_ART`, `NUMBER_SEPARATOR_ASCII_ART`, `NUMBER_WIDTH`).

## Architecture
- `Config { theme: Theme, keyboard: Keyboard, pomodoro: PomodoroConfig }` with hardcoded `Default`s. Planned: load from a file, falling back to these defaults. `PomodoroConfig` holds work/short break/long break minutes (25/5/15), `long_break_interval` (4) and `auto_start_break` (false). Keep all colors, key bindings and durations in these structs, not as literals elsewhere.
- `App` owns `config`, `mode` (`Normal` / `TaskTextInput`), `input_buffer`, `tasks`. `run` loop: draw, then `handle_events` (100ms poll).
- UI is immediate-mode: `draw` re-renders everything each frame (no manual redraw/resize handling).
- Screen: left sidebar (fixed `SIDEBAR_WIDTH` = 40: clock + hotkey hints), right tasks panel (fills the rest).
- Pomodoro: `PomodoroState` (Idle/Work/ShortBreak/LongBreak) + `phase_ends_at: Option<Instant>`, advanced by `update_pomodoro` each loop iteration. Work -> break (long every `long_break_interval` pomodoros; see Breaks) -> Idle; the user starts each new pomodoro. One key (`toggle_pomodoro`, `s`) starts/stops. Clock shows remaining time (the work duration while idle).
- Breaks: with `auto_start_break` false, a finished work phase leaves the break set up but not running (`awaiting_break_start`, full time in `paused_remaining`, status `(ready)`); `s` starts it (hint "Start break"), and `b` (`skip_break`, listed only during a break, whether waiting, running or paused) skips the break and returns to Idle. Pause is unavailable while waiting. When true the break starts immediately.
- Desktop notifications (`notify-rust`, `PomodoroConfig.notifications`, default true) are sent from `update_pomodoro` when a work phase ends (break started or ready) and when a break ends, via `App::notify` on a background thread; errors are ignored. User-initiated start/stop/skip does not notify.
- Pause: `p` (`pause_pomodoro`) toggles pause while not idle. Pausing stores `paused_remaining` and clears `phase_ends_at`; resuming restores the end time. The hint is only listed when not idle, and the status bar shows `(paused)`.
- Tasks: `selected_task` is moved with vim keys (`j`/`k`, in `Keyboard`) and highlighted with `theme.selection`. `e` edits the selected task by reusing `TaskTextInput` mode with `editing_task: Some(i)` (`None` = adding). New tasks become selected. Each `Task` has `estimated_pomodoros` (starts at 1, raised with `+`, lowered with `-`, never below `MIN_ESTIMATED_POMODOROS` = 1) and `completed_pomodoros`; every visible row shows `completed/estimated` right-aligned (except the row being edited). Tasks have a `done` flag shown by the marker block (`x` toggles it). A finished work phase credits the first task that is not done, regardless of the selection, and marks it done once `completed >= estimated`. `d` deletes the selected task immediately (no confirmation, no undo); selection is clamped afterwards. `c` clears all done tasks and `C` (shift+c) clears every task, both immediately with no confirmation or undo.
- Key bindings are `KeyBinding { code, modifiers }` (`plain(..)` / `ctrl(..)`); always match with `KeyBinding::matches` (ignores SHIFT) so `j` and `Ctrl+j` stay distinct. `Ctrl+j`/`Ctrl+k` (`move_task_down`/`move_task_up`) swap the selected task with its neighbor and the selection follows it.
- Plugins (Lua, `mlua`): events only. Plugins cannot add hotkeys or UI, and are NOT sandboxed (`io`/`os` are available on purpose). `PluginHost::new` loads the embedded core plugin, then `load_user_plugins` loads `*.lua` from `<config_dir>/ptc/plugins/` in name order (`directories::ProjectDirs`; data dir is `<data_dir>/ptc`, created at startup). API: `ptc.on(event, fn)` (unknown events error), `ptc.set_tasks(list)`, `ptc.config(table)` (merges a partial config: `{ theme = {...}, keyboard = {...}, pomodoro = {...} }`), `ptc.json.encode/decode`, `ptc.data_dir`, `ptc.plugins_dir`, `ptc.status_file`. Events: `startup`, `quit` (no payload), `tasks_changed` (list of `{text, estimated_pomodoros, completed_pomodoros, done}`), `pomodoro_state_changed` (`{state, previous, paused, waiting, completed_pomodoros, remaining_seconds, ends_at?}`; `ends_at` is a Unix timestamp, omitted via `skip_serializing_if` unless counting down, because Lua would see a serde `None` as a truthy null sentinel; states `idle|work|short_break|long_break`). Events are derived by diffing after every loop iteration (`App::sync_plugins`), never emitted from individual actions, so new state changes are picked up automatically. Tasks set by a plugin are not echoed back as `tasks_changed`. Handler errors never crash the app: they are collected and printed to stderr after the TUI exits. Adding an event = add it to `EVENTS` in `bootstrap.lua` and emit it from `sync_plugins`.
- Config merging: `Config`, `Theme`, `Keyboard` and `PomodoroConfig` derive serde with `deny_unknown_fields`. `ptc.config` patches are queued in the host and applied by `App::apply_plugin_config` (at startup after load, and after every loop iteration), via `merge_config` (config -> JSON, deep merge, JSON -> config). A patch with an unknown key or bad value is rejected whole and reported as a plugin error. Colors are ratatui `Color` strings (`"red"`, `"#112233"`); key bindings are strings parsed by `KeyBinding::from_str` (`"q"`, `"Ctrl+j"`, `"Enter"`, `"F5"`, `"Space"`; modifiers/names case-insensitive, single characters case-sensitive). New config fields must keep serde-compatible types, so the same mechanism can later load a config file.
- Task list scrolling: `draw_tasks` (which takes `&mut self`) updates `scroll_offset` so the cursor row (selected task, or the new-task input line) stays visible. A `Scrollbar` (`theme.scrollbar`) is drawn in the panel's right padding column only when the list overflows.
- Tasks panel bottom status bar (`theme.tasks_status_bar*`): right-aligned `Estimated: Xh YYm` from `estimated_time_left` = remaining (estimated - completed) pomodoros of open tasks x work time, plus the breaks between them (long break cadence continues from `completed_pomodoros`). It ignores the time left in a pomodoro already running. The left side of the bar shows the task count (`5 Tasks`, `1 Task`). It also shows the wall-clock finish time (`Finish at HH:MM`, local time via `chrono`, now + estimate).
- Sidebar top to bottom: title (`POMODORO TIME CHAMBER`, `theme.title`), clock, blank, status bar, blank, hotkey hints.
- Below the clock, a one-line status bar is colored per state via `theme.status_*`.
- Hotkey hints in the sidebar are generated from `Keyboard` (`{key} → {label}`; the key uses `theme.hotkey_key`, the rest `theme.hotkey`), so rebinding updates them. Keys are padded to the widest key in the list so all labels start at the same column.

## Conventions / preferences
- **No color may be hardcoded, not even one.** Every color (including `Color::Red`, `Color::White`, RGB values, etc.) must be a field of `Theme`, with its default defined only in `Theme::default()`. Never write a color literal in drawing code.
- No borders; sections are separated by different background colors (`theme.background` vs `theme.panel_background`).
- Panel title is `TASKS`: bold, uppercase, no `|` decorations.
- Sidebar padding is 3 columns left and right; tasks panel uses its own padding.
- Task marker is a 3-column colored block (no brackets): background (`theme.task_unmarked`, defaults to the same color as `theme.background`, the left column) when open, green background (`theme.task_marked`) with a bold `x` in the middle (`theme.task_marked_text`) when done; only the marker is colored, built by `App::task_line`.
- Art strings start with a `\n`, so the first row of each digit is blank; `clock_lines` relies on that (6 rows).

## Status / TODO
- Timer has no sound/notification, and a finished break doesn't auto-roll into the next pomodoro.
- Config file loading not implemented (`Color`/`KeyCode` would need serde support).

## Releases / commits
- Releases use release-please (`.github/workflows/release.yml`, `release-please-config.json`, `.release-please-manifest.json`): it opens a release PR from commits on `master`; merging it tags `vX.Y.Z` and the same workflow attaches the Linux binary (`ptc-vX.Y.Z-x86_64-linux.tar.gz`). Never edit the version in `Cargo.toml` by hand.
- **Commit messages must be Conventional Commits** (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `feat!:` for breaking), otherwise release-please ignores them. Earlier history is not in that format. The version is computed from them (fix = patch, feat = minor, `!`/`BREAKING CHANGE` = major). The first release is forced to 1.0.0 by the `chore: release 1.0.0` commit with a `Release-As: 1.0.0` footer; do not add `Release-As` again unless deliberately forcing a version. **Always write commit messages in this format.**

## Maintenance
Update this file whenever architecture, conventions or TODOs change.
`README.md` documents hotkeys, config options (theme/keyboard/pomodoro tables with defaults), the plugin API and events for users; update it whenever any of those change (new hotkey, theme field, config option, event or `ptc.*` function).
