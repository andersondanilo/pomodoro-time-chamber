use std::{thread, time::Duration};
use pancurses::{curs_set, endwin, has_colors, init_pair, initscr, noecho, start_color, ColorPair, Input, Window, COLOR_BLACK, COLOR_BLUE, COLOR_RED, COLOR_WHITE};

mod number_ascii_art;

const COLOR_NORMAL_INDEX: u8 = 0;
const COLOR_POMODORO_INDEX: u8 = 1;
const COLOR_HOTKEY_INDEX: u8 = 2;

enum AppMode {
    Normal,
    TaskTextInput,
}

struct Task {
    text: String,
}

struct TaskTextInputState {
    input_buffer: String,
    y: i32,
    x: i32,
}

struct App<'a> {
    win: &'a Window,
    mode: AppMode,
    current_task_text: Option<TaskTextInputState>,
    tasks: Vec<Task>,
}

impl<'a> App<'a> {
    fn new(win: &'a Window) -> Self {
        App {
            win,
            mode: AppMode::Normal,
            current_task_text: None,
            tasks: Vec::new(),
        }
    }

    fn initialize(&mut self) {
        let win = self.win;
        win.keypad(true);
        win.nodelay(true);
        win.refresh();
        noecho();
        curs_set(0);

        if has_colors() {
            start_color();
        }

        init_pair(COLOR_NORMAL_INDEX as i16, COLOR_WHITE, COLOR_BLACK);
        init_pair(COLOR_POMODORO_INDEX as i16, COLOR_RED, COLOR_BLACK);
        init_pair(COLOR_HOTKEY_INDEX as i16, COLOR_BLUE, COLOR_BLACK);

        self.draw();
    }

    fn redraw(&self) {
        self.win.clear();
        self.draw();
    }

    fn draw(&self) {
        let win = self.win;
        let max_x = win.get_max_x();
        let right_column_size = 40;

        win.attron(ColorPair(COLOR_NORMAL_INDEX));

        self.draw_utf8_box(0, 0, win.get_max_x() - right_column_size, win.get_max_y());

        win.mv(0, 2);
        win.printw("| Tasks |");

        let start_left_column = max_x - right_column_size + 3;

        self.draw_clock(Duration::from_secs(125), 0, start_left_column);

        win.attron(ColorPair(COLOR_HOTKEY_INDEX));
        win.mvaddstr(7, start_left_column, "a - Add new task");
        win.mvaddstr(8, start_left_column, "q - Quit");
        win.attroff(ColorPair(COLOR_NORMAL_INDEX));
    }

    fn draw_utf8_box(&self, x: i32, y: i32, w: i32, h: i32) {
        let win = self.win;
        win.mvaddstr(y, x, "┌");
        win.mvaddstr(y, x + w - 1, "┐");
        win.mvaddstr(y + h - 1, x, "└");

        win.mvaddstr(y + h - 1, x + w - 1, "┘");


        for i in (x + 1)..(x + w - 1) {
            win.mvaddstr(y, i, "─");
            win.mvaddstr(y + h - 1, i, "─");
        }
        for j in (y + 1)..(y + h - 1) {
            win.mvaddstr(j, x, "│");
            win.mvaddstr(j, x + w - 1, "│");
        }
    }

    fn draw_clock(&self, duration: Duration, y: i32, x: i32) {
        let total_seconds = duration.as_secs();
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let clock_text = format!(
            "{:02}:{:02}",
            minutes,
            seconds,
        );
        let mut next_x = x;
        let mut last_ch = None;

        for (i, ch) in clock_text.chars().enumerate() {
            let art = match ch {
                ':' => number_ascii_art::NUMBER_SEPARATOR_ASCII_ART,
                n => number_ascii_art::NUMBER_ASCII_ART[n.to_digit(10).unwrap() as usize],
            };

            if i > 0 {
                if last_ch == Some(':') || ch == ':' {
                    next_x += 6;
                } else {
                    next_x += number_ascii_art::NUMBER_WIDTH + 2;
                }
            }

            self.draw_ascii_art(art, next_x, y);

            last_ch = Some(ch);
        }
    }

    fn draw_ascii_art(&self, art: &str, x: i32, y: i32) {
        let win = self.win;
        win.attron(ColorPair(COLOR_POMODORO_INDEX));
        for (i, line) in art.lines().enumerate() {
            win.mvaddstr(y + i as i32, x, line);
        }
        win.attroff(ColorPair(COLOR_POMODORO_INDEX));
    }

    fn tick(&mut self) -> Option<()> {
        let win = self.win;
        let should_countinue = match self.mode {
            AppMode::Normal => {
                match win.getch() {
                    Some(Input::Character('q')) => None,
                    Some(Input::Character('a')) => {
                        self.action_start_add_task();
                        Some(())
                    },
                    Some(Input::KeyResize) => {
                        self.redraw();
                        Some(())
                    },
                    _ => {Some(())}
                }
            }
            AppMode::TaskTextInput => {
                Some(())
            }
        };
        
        thread::sleep(Duration::from_millis(100));
        should_countinue
    }

    fn finish(&mut self) {
        endwin();
    }

    fn action_start_add_task(&mut self) {
        let (y, x) = ((self.tasks.len() as i32) + 2, 3);
        self.win.mvaddstr(y, x - 1, "🍅");
        self.win.mvaddstr(y, x - 1, "TEST");
        self.mode = AppMode::TaskTextInput;
        self.current_task_text = Some(TaskTextInputState {
            input_buffer: String::new(),
            y,
            x,
        });
    }
}

fn main() {
    let win = initscr();

    let mut app = App::new(&win);
    app.initialize();

    loop {
        if app.tick().is_none() {
            break;
        }
    }

    app.finish();
}

