use iced::Color;

pub fn dark_border(color: Color) -> Color {
    darken(color, 0.5)
}

pub fn darken(color: Color, factor: f32) -> Color {
    Color::from_rgb(
        color.r * factor,
        color.g * factor,
        color.b * factor,
    )
}