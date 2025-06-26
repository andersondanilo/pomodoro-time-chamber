use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::{border, Theme};
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size};
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

pub fn timer(radius: f32, text: String, progress: f32, status: Status) -> Timer {
    Timer::new(radius, text, progress, status)
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Timer
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(self.radius * 2.0, self.radius * 2.0))
    }

    fn draw(
        &self,
        _state: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let style = theme.style(&self.status);

        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: border::rounded(self.radius),
                ..renderer::Quad::default()
            },
            style.background,
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// The [`Background`] of the text input.
    pub background: Color,
}

pub trait Catalog: Sized {
    /// The [`Style`] of a class with the given status.
    fn style(&self, status: &Status) -> Style;
}

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    fn style(&self, _status: &Status) -> Style {
        let palette = self.extended_palette();

        Style {
            background: colors::dark_border(palette.primary.base.color),
        }
    }
}



impl<Message, Theme, Renderer> From<Timer>
    for Element<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    fn from(timer: Timer) -> Self {
        Self::new(timer)
    }
}