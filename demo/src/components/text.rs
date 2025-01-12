use super::drawable::Drawable;
use crossterm::style::Color;
use std::{error::Error, io::Stdout};
use vek::{Rect, Vec2};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TextHorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TextVerticalAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct Text {
    pub content: String,
    pub color: Color,
    pub background_color: Color,
    pub horizontal_align: TextHorizontalAlign,
    pub vertical_align: TextVerticalAlign,
}

impl Text {
    pub fn new(content: impl ToString) -> Self {
        Self {
            content: content.to_string(),
            color: Color::Reset,
            background_color: Color::Reset,
            horizontal_align: Default::default(),
            vertical_align: Default::default(),
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn with_horizontal_align(mut self, align: TextHorizontalAlign) -> Self {
        self.horizontal_align = align;
        self
    }

    pub fn with_vertical_align(mut self, align: TextVerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }
}

impl Drawable for Text {
    fn draw(
        &self,
        _stream: &mut Stdout,
        _position: Vec2<isize>,
        _screen_rect: Rect<isize, isize>,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
