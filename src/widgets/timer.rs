use std::f32::consts::PI;
use std::sync::Arc;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::Length::Fill;
use iced::{border, Font, Pixels, Point, Radians, Renderer, Theme};
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size};
use iced::widget::canvas;
use crate::utils::colors;

pub enum Status {
    Working,
    Resting,
}

pub struct Timer {
    radius: f32,
    text: String,
    progress: f32,
    status: Status,
}

impl Timer {
    pub fn new(radius: f32, text: String, progress: f32, status: Status) -> Self {
        Self { radius, text, progress, status }
    }
}

pub fn timer<'a, Message: 'a>(radius: f32, text: String, progress: f32, status: Status) -> Element<'a, Message> {
    canvas::Canvas::new(Timer::new(radius, text, progress, status))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

impl<Message> canvas::Program<Message> for Timer
{
    // No internal state
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor
    ) -> Vec<canvas::Geometry> {
        let style = theme.style(&self.status);
        

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let circle1 = canvas::Path::circle(center, self.radius);
        frame.fill(&circle1, style.bg_border1);

        let start_angle: Radians = (3.0 * PI / 2.0).into();
        let sweep: Radians = (self.progress * 2.0 * PI).into();
        let end_angle: Radians = start_angle - sweep;

        let slice = canvas::Path::new(|builder| {
            builder.move_to(center);
            builder.arc(canvas::path::Arc {
                center,
                radius: self.radius * 0.95,
                start_angle,
                end_angle
            });
            builder.line_to(bounds.center());
            builder.close();
        });
        frame.fill(&slice, style.bg_border2);

        let circle2 = canvas::Path::circle(center, self.radius * 0.85);
        frame.fill(&circle2, style.bg_display);

        let bold_font = Font {
            weight: iced::font::Weight::Bold,
            ..Font::default()
        };

        let text_position = Point {
            x: (center.x - self.radius * 0.65).round(),
            y: (center.y - self.radius * 0.35).round(),
        };

        let text = iced::widget::canvas::Text {
            content: self.text.clone(),
            position: text_position,
            color: style.text,
            size: Pixels::from((self.radius * 0.5).round() as u16),
            font: bold_font,
            ..Default::default()
        };

        frame.fill_text(text);

        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// The [`Background`] of the text input.
    pub bg_border1: Color,
    pub bg_border2: Color,
    pub bg_display: Color,
    pub text: Color
}

pub trait Catalog: Sized {
    /// The [`Style`] of a class with the given status.
    fn style(&self, status: &Status) -> Style;
}

impl Catalog for Theme {
    fn style(&self, _status: &Status) -> Style {
        let palette = self.extended_palette();

        Style {
            bg_border1: colors::dark_border(palette.primary.base.color),
            bg_border2: palette.primary.base.color,
            bg_display: Color::from_rgb(21.0 / 256.0, 31.0 / 256.0, 34.0 / 256.0),
            text: Color::WHITE,
        }
    }
}