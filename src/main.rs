mod widgets;
pub mod utils;
pub mod assets;

use iced::{
    theme::Palette, widget::{button::{self, Button, Status}, column, container, row, svg::{self, Handle, Svg} }, Border, Color, Element, Length, Theme
};
use widgets::timer;
use assets::{PLAY_SVG, TITLE_SVG};

use crate::utils::colors;

const BUTTON_WIDTH: f32 = 50.0;

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
        let timer_height = 300.0;
        let timer_padding = 20.0;

        container(column![
            Svg::new(Handle::from_memory(TITLE_SVG.to_vec()))
                .width(Length::Fill)
                .height(Length::Fixed(200.0)),
            container(timer::timer(
                timer_height / 2.0,
                format!("{:02}:{:02}", self.minutes, self.seconds),
                0.5,
                timer::Status::Working
            )).padding(5).center_x(Length::FillPortion(1)).height(Length::Fixed(timer_height + timer_padding)),
            row![
                container(Button::new(Svg::new(Handle::from_memory(PLAY_SVG.to_vec())).width(BUTTON_WIDTH * 0.3).height(BUTTON_WIDTH * 0.3)).width(BUTTON_WIDTH).height(BUTTON_WIDTH).on_press(Message::StartTime).style(make_button_style))
                    .padding(5)
                    .width(Length::FillPortion(1))
                    .align_right(Length::Fill),
                container(
                    Button::new(if self.state == PomodoroState::Paused {
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
        min_size: Some(iced::Size::new(600.0, 400.0)),
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

fn make_button_style(theme: &Theme, status: Status) -> button::Style {
    button::Style {
        background: Some(match status {
            Status::Hovered => colors::dark_border(theme.palette().primary),
            _ => theme.palette().primary,
        }.into()),
        border: Border {
            radius: (BUTTON_WIDTH / 2.0).into(), // round, half the width
            width: 2.0,
            color: colors::dark_border(theme.palette().primary)
        },
        ..button::Style::default()
    }
}