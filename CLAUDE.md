# pomodoro-time-chamber

Terminal pomodoro + task list app in Rust (edition 2024), built on `ratatui` (crossterm backend).

## Commands
- `cargo build` / `cargo run`. The binary is named `ptc` (`[[bin]]` in `Cargo.toml`); the package name stays `pomodoro-time-chamber`.
- No tests yet.

## Layout
- `src/main.rs`: everything else (config, `App`, drawing, event handling, `clock_lines`).
- `src/number_ascii_art.rs`: big block-digit art for the clock (`NUMBER_ASCII_ART`, `NUMBER_SEPARATOR_ASCII_ART`, `NUMBER_WIDTH`).

## Architecture
- `Config { theme: Theme, keyboard: Keyboard, pomodoro: PomodoroConfig }` with hardcoded `Default`s. Planned: load from a file, falling back to these defaults. `PomodoroConfig` holds work/short break/long break minutes (25/5/15) and `long_break_interval` (4). Keep all colors, key bindings and durations in these structs, not as literals elsewhere.
- `App` owns `config`, `mode` (`Normal` / `TaskTextInput`), `input_buffer`, `tasks`. `run` loop: draw, then `handle_events` (100ms poll).
- UI is immediate-mode: `draw` re-renders everything each frame (no manual redraw/resize handling).
- Screen: left sidebar (fixed `SIDEBAR_WIDTH` = 40: clock + hotkey hints), right tasks panel (fills the rest).
- Pomodoro: `PomodoroState` (Idle/Work/ShortBreak/LongBreak) + `phase_ends_at: Option<Instant>`, advanced by `update_pomodoro` each loop iteration. Work -> break (long every `long_break_interval` pomodoros) -> Idle; the user starts each new pomodoro. One key (`toggle_pomodoro`, `s`) starts/stops. Clock shows remaining time (the work duration while idle).
- Pause: `p` (`pause_pomodoro`) toggles pause while not idle. Pausing stores `paused_remaining` and clears `phase_ends_at`; resuming restores the end time. The hint is only listed when not idle, and the status bar shows `(paused)`.
- Tasks: `selected_task` is moved with vim keys (`j`/`k`, in `Keyboard`) and highlighted with `theme.selection`. `e` edits the selected task by reusing `TaskTextInput` mode with `editing_task: Some(i)` (`None` = adding). New tasks become selected. Each `Task` has `estimated_pomodoros` (raised with `+`, lowered with `-`) and `completed_pomodoros`; the selected row shows `(completed/estimated)` right-aligned once the estimate is > 0. Tasks have a `done` flag shown as `[x]`/`[ ]` (`x` toggles it). A finished work phase credits the first task that is not done, regardless of the selection, and marks it done once `completed >= estimated` (only when estimated > 0). `d` deletes the selected task immediately (no confirmation, no undo); selection is clamped afterwards.
- Sidebar top to bottom: title (`POMODORO TIME CHAMBER`, `theme.title`), clock, blank, status bar, blank, hotkey hints.
- Below the clock, a one-line status bar is colored per state via `theme.status_*`.
- Hotkey hints in the sidebar are generated from `Keyboard` (`{key} - {label}`), so rebinding updates them.

## Conventions / preferences
- **No color may be hardcoded, not even one.** Every color (including `Color::Red`, `Color::White`, RGB values, etc.) must be a field of `Theme`, with its default defined only in `Theme::default()`. Never write a color literal in drawing code.
- No borders; sections are separated by different background colors (`theme.background` vs `theme.panel_background`).
- Panel title is `TASKS`: bold, uppercase, no `|` decorations.
- Sidebar padding is 3 columns left and right; tasks panel uses its own padding.
- Task marker is `[ ]` (gray, `theme.task_unmarked`), or `[x]` when done (green, `theme.task_marked`); only the marker is colored, built by `App::task_line`.
- Art strings start with a `\n`, so the first row of each digit is blank; `clock_lines` relies on that (6 rows).

## Status / TODO
- Timer has no pause, no sound/notification, and breaks don't auto-roll into the next pomodoro.
- Config file loading not implemented (`Color`/`KeyCode` would need serde support).

## Maintenance
Update this file whenever architecture, conventions or TODOs change.
