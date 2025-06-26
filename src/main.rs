mod widgets;
pub mod utils;

use iced::{
    theme::Palette, widget::{button, column, container, row, text}, Background, Color, Element, Length, Theme
};
use widgets::timer;



#[derive(Eq, PartialEq)]
enum PomodoroState {
    Work,
    ShortBreak,
    LongBreak,
    Paused,
    Stopped,
}

impl Default for PomodoroState {
    fn default() -> Self {
        PomodoroState::Stopped
    }
}

struct PomodoroTimeChamber {
    minutes: u32,
    seconds: u32,
    state: PomodoroState,
    app_theme: Theme,
}

impl Default for PomodoroTimeChamber {
    fn default() -> Self {
        Self {
            minutes: 0,
            seconds: 0,
            state: PomodoroState::default(),
            app_theme: make_app_theme(),
        }
    }
}


#[derive(Clone, Debug)]
enum Message {
    StartTime,
    PauseTime,
}

impl PomodoroTimeChamber {
    fn view(&self) -> Element<Message> {
        container(column![
            container(timer::timer(
                100.0,
                format!("{:02}:{:02}", self.minutes, self.seconds),
                0.5,
                timer::Status::Working
            )).padding(5).center_x(Length::FillPortion(1)),
            row![
                container(button("Start").on_press(Message::StartTime))
                    .padding(5)
                    .width(Length::FillPortion(1))
                    .align_right(Length::Fill),
                container(
                    button(if self.state == PomodoroState::Paused {
                        "Stop"
                    } else {
                        "Pause"
                    })
                    .on_press(Message::PauseTime)
                )
                .padding(5)
                .width(Length::FillPortion(1))
                .align_left(Length::Fill),
            ],
        ]).into()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::StartTime => {
                self.state = PomodoroState::Work;
            }
            Message::PauseTime => {
                self.state = match self.state {
                    PomodoroState::Paused => PomodoroState::Stopped,
                    _ => PomodoroState::Paused,
                }
            }
        }
    }
}

fn main() -> iced::Result {
    iced::application(
        "Pomodoro Time Chamber",
        PomodoroTimeChamber::update,
        PomodoroTimeChamber::view,
    )
    .window(iced::window::Settings {
        size: iced::Size::new(400.0, 300.0),
        ..iced::window::Settings::default()
    })
    .theme(make_app_theme_with_app)
    .run()
}
fn make_app_theme_with_app(state: &PomodoroTimeChamber) -> Theme {
    make_app_theme()
}

fn make_app_theme() -> Theme {
    let primary = Color::from_rgb(0.957, 0.459, 0.325);
    let background = Color::from_rgb(0.984, 0.961, 0.937);
    Theme::custom("App theme".into(), Palette {
        background,
        primary,
        ..Palette::LIGHT
    })
}
