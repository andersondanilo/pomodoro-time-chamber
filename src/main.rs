use std::marker::PhantomData;

use iced::{
    widget::{button, column, container, row, shader::wgpu::Instance, text, Column, Row},
    Application, Background, Color, Length,
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
struct PomodoroTimerChamber {
    minutes: u32,
    seconds: u32,
    state: PomodoroState,
}

#[derive(Clone, Debug)]
enum Message {
    StartTimer,
    PauseTimer,
}

impl PomodoroTimerChamber {
    fn view(&self) -> Column<Message> {
        column![
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
                container(button("Start").on_press(Message::StartTimer))
                    .padding(5)
                    .width(Length::FillPortion(1))
                    .align_right(Length::Fill),
                container(
                    button(if self.state == PomodoroState::Paused {
                        "Stop"
                    } else {
                        "Pause"
                    })
                    .on_press(Message::PauseTimer)
                )
                .padding(5)
                .width(Length::FillPortion(1))
                .align_left(Length::Fill),
            ],
        ]
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::StartTimer => {
                self.state = PomodoroState::Work;
            }
            Message::PauseTimer => {
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
        PomodoroTimerChamber::update,
        PomodoroTimerChamber::view,
    )
    .window(iced::window::Settings {
        size: iced::Size::new(400.0, 300.0),
        ..iced::window::Settings::default()
    })
    .run()
}
