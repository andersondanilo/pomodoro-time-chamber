use std::{
    io,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

mod number_ascii_art;

const SIDEBAR_WIDTH: u16 = 40;
const MIN_ESTIMATED_POMODOROS: u32 = 1;

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
    /// Task marker color when the task is not done (`[ ]`).
    task_unmarked: Color,
    /// Task marker color when the task is done (`[x]`).
    task_marked: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            background: Color::Rgb(0x1a, 0x1b, 0x26),
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
            task_unmarked: Color::Gray,
            task_marked: Color::Green,
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
        KeyBinding { code, modifiers: KeyModifiers::NONE }
    }

    fn ctrl(code: KeyCode) -> Self {
        KeyBinding { code, modifiers: KeyModifiers::CONTROL }
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
        write!(f, "{}", self.code)
    }
}

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
            select_next: KeyBinding::plain(KeyCode::Char('j')),
            select_previous: KeyBinding::plain(KeyCode::Char('k')),
            move_task_down: KeyBinding::ctrl(KeyCode::Char('j')),
            move_task_up: KeyBinding::ctrl(KeyCode::Char('k')),
            confirm_task: KeyBinding::plain(KeyCode::Enter),
            cancel_task: KeyBinding::plain(KeyCode::Esc),
        }
    }
}

struct PomodoroConfig {
    work_minutes: u64,
    short_break_minutes: u64,
    long_break_minutes: u64,
    /// A long break replaces the short one after this many pomodoros.
    long_break_interval: u32,
    /// Start the break automatically when a work phase ends; otherwise wait for the start key.
    auto_start_break: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        PomodoroConfig {
            work_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_interval: 4,
            auto_start_break: false,
        }
    }
}

#[derive(Default)]
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
}

struct Task {
    text: String,
    /// Always at least `MIN_ESTIMATED_POMODOROS`.
    estimated_pomodoros: u32,
    completed_pomodoros: u32,
    done: bool,
}

struct App {
    config: Config,
    mode: AppMode,
    input_buffer: String,
    tasks: Vec<Task>,
    selected_task: usize,
    /// Index of the first task shown in the list; keeps the cursor row visible.
    scroll_offset: usize,
    /// Index of the task being edited; `None` when adding a new one.
    editing_task: Option<usize>,
    should_quit: bool,
    pomodoro_state: PomodoroState,
    /// When the current phase ends; `None` while idle.
    phase_ends_at: Option<Instant>,
    /// Time left in the current phase while paused (`phase_ends_at` is `None` then).
    paused_remaining: Option<Duration>,
    /// A break is set up (time in `paused_remaining`) but has not been started yet.
    awaiting_break_start: bool,
    completed_pomodoros: u32,
}

impl App {
    fn new(config: Config) -> Self {
        App {
            config,
            mode: AppMode::Normal,
            input_buffer: String::new(),
            tasks: Vec::new(),
            selected_task: 0,
            scroll_offset: 0,
            editing_task: None,
            should_quit: false,
            pomodoro_state: PomodoroState::Idle,
            phase_ends_at: None,
            paused_remaining: None,
            awaiting_break_start: false,
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
    }

    /// Sets up a break without starting its countdown.
    fn prepare_phase(&mut self, state: PomodoroState) {
        self.pomodoro_state = state;
        self.phase_ends_at = None;
        self.paused_remaining = Some(self.phase_duration(state));
        self.awaiting_break_start = true;
    }

    fn stop_pomodoro(&mut self) {
        self.pomodoro_state = PomodoroState::Idle;
        self.phase_ends_at = None;
        self.paused_remaining = None;
        self.awaiting_break_start = false;
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
                if let Some(task) = self.tasks.iter_mut().find(|task| !task.done) {
                    task.completed_pomodoros += 1;
                    if task.completed_pomodoros >= task.estimated_pomodoros {
                        task.done = true;
                    }
                }
                let interval = self.config.pomodoro.long_break_interval.max(1);
                let next = if self.completed_pomodoros % interval == 0 {
                    PomodoroState::LongBreak
                } else {
                    PomodoroState::ShortBreak
                };
                if self.config.pomodoro.auto_start_break {
                    self.start_phase(next);
                } else {
                    self.prepare_phase(next);
                }
            }
            _ => self.stop_pomodoro(),
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
        }
        Ok(())
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
        self.draw_tasks(frame, right);
    }

    /// A task row: the marker in its theme color, then the text.
    fn task_line(&self, done: bool, text: String) -> Line<'static> {
        let theme = &self.config.theme;
        let (marker, color) = if done {
            ("[x]", theme.task_marked)
        } else {
            ("[ ]", theme.task_unmarked)
        };
        Line::from(vec![
            Span::styled(marker, Style::new().fg(color)),
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
            .map(|task| task.estimated_pomodoros.saturating_sub(task.completed_pomodoros))
            .sum();

        let interval = self.config.pomodoro.long_break_interval.max(1);
        let mut total = self.phase_duration(PomodoroState::Work) * pending;
        // A break follows every pomodoro except the last one.
        for i in 1..pending {
            let state = if (self.completed_pomodoros + i) % interval == 0 {
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
        let count = match self.tasks.len() {
            1 => "1 Task".to_string(),
            n => format!("{n} Tasks"),
        };
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
        frame.render_widget(Paragraph::new("TASKS").style(Style::new().bold()), title_area);

        let editing = matches!(self.mode, AppMode::TaskTextInput);
        let mut lines: Vec<Line> = self
            .tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                if editing && self.editing_task == Some(i) {
                    self.task_line(task.done, format!("{}█", self.input_buffer))
                } else {
                    self.task_line(task.done, task.text.clone())
                }
            })
            .collect();

        let adding = editing && self.editing_task.is_none();
        if adding {
            lines.push(self.task_line(false, format!("{}█", self.input_buffer)));
        }

        // Scroll just enough to keep the cursor row (selected task or new task) visible.
        let height = tasks_area.height as usize;
        let cursor = if adding { self.tasks.len() } else { self.selected_task };
        if cursor < self.scroll_offset {
            self.scroll_offset = cursor;
        } else if height > 0 && cursor >= self.scroll_offset + height {
            self.scroll_offset = cursor + 1 - height;
        }
        self.scroll_offset = self.scroll_offset.min(lines.len().saturating_sub(height));

        // Highlight the whole row (up to the panel's right padding), not just the text.
        if !self.tasks.is_empty() && !adding {
            let row = Rect::new(
                tasks_area.x,
                tasks_area.y + (self.selected_task - self.scroll_offset) as u16,
                tasks_area.width,
                1,
            );
            frame.render_widget(
                Block::new().style(Style::new().bg(self.config.theme.selection)),
                row,
            );

            let task = &self.tasks[self.selected_task];
            if !editing {
                let counter = format!(
                    "({}/{})",
                    task.completed_pomodoros, task.estimated_pomodoros
                );
                frame.render_widget(Paragraph::new(counter).right_aligned(), row);
            }
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
            Paragraph::new(if self.awaiting_break_start {
                format!("{} (ready)", self.pomodoro_state.label())
            } else if self.is_paused() {
                format!("{} (paused)", self.pomodoro_state.label())
            } else {
                self.pomodoro_state.label().to_string()
            })
            .centered()
            .style(status_style),
            status_area,
        );

        let keyboard = &self.config.keyboard;
        let hotkeys = match self.mode {
            AppMode::Normal => {
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
                    (keyboard.quit, "Quit"),
                ]);
                hotkeys
            }
            AppMode::TaskTextInput => vec![
                (keyboard.confirm_task, "Save task"),
                (keyboard.cancel_task, "Cancel"),
            ],
        };
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
        frame.render_widget(Paragraph::new(hotkeys), hotkeys_area);
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

        let keyboard = &self.config.keyboard;
        match self.mode {
            AppMode::Normal => match key.code {
                _ if keyboard.quit.matches(&key) => self.should_quit = true,
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
        Ok(())
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

    fn action_toggle_done(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            task.done = !task.done;
        }
    }

    fn action_increase_estimate(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            task.estimated_pomodoros += 1;
        }
    }

    fn action_decrease_estimate(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            task.estimated_pomodoros =
                task.estimated_pomodoros.saturating_sub(1).max(MIN_ESTIMATED_POMODOROS);
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
            rows[i].push_str(&format!("{:<width$}", line));
        }

        last_ch = Some(ch);
    }

    rows.into_iter().map(Line::from).collect()
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new(Config::default()).run(&mut terminal);
    ratatui::restore();
    result
}
