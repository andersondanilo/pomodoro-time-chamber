use iced::{
    theme::Palette, widget::{button, column, container, row, text}, Background, Color, Element, Length, Theme
};

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

#[derive(Default)]
struct PomodoroTimeChamber {
    minutes: u32,
    seconds: u32,
    state: PomodoroState,
}

#[derive(Clone, Debug)]
enum Message {
    StartTime,
    PauseTime,
}

impl PomodoroTimeChamber {
    fn view(&self) -> Element<Message> {
        container(column![
            container(text(format!("{:02}:{:02}", self.minutes, self.seconds)).size(50))
                .center_x(Length::Fill)
                .padding(20)
                .width(Length::Fill)
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgb(1.0, 0.0, 0.0))),
                    text_color: Some(iced::Color::WHITE),
                    ..container::Style::default()
                }),
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
    .theme(app_theme)
    .run()
}
fn app_theme(state: &PomodoroTimeChamber) -> Theme {
    let primary = Color::from_rgb(0.957, 0.459, 0.325);
    let background = Color::from_rgb(0.984, 0.961, 0.937);
    Theme::custom("App theme".into(), Palette {
        background,
        primary,
        ..Palette::LIGHT
    })
}
