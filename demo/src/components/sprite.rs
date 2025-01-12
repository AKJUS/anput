use super::drawable::Drawable;
use crate::utils::image::Image;
use crossterm::{cursor::MoveTo, queue, style::Color};
use std::{
    error::Error,
    io::{Stdout, Write},
};
use vek::{Rect, Vec2};

#[derive(Debug, Clone)]
pub struct Sprite {
    pub image: Image,
    pub color: Color,
    pub background_color: Color,
    pub pivot: Vec2<isize>,
}

impl Sprite {
    pub fn new(image: Image) -> Self {
        Self {
            image,
            color: Color::Reset,
            background_color: Color::Reset,
            pivot: Default::default(),
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

    pub fn with_pivot(mut self, pivot: Vec2<isize>) -> Self {
        self.pivot = pivot;
        self
    }
}

impl Drawable for Sprite {
    fn draw(
        &self,
        stream: &mut Stdout,
        position: Vec2<isize>,
        screen_rect: Rect<isize, isize>,
    ) -> Result<(), Box<dyn Error>> {
        let position = position - self.pivot;
        let size = self.image.content().size().as_::<isize>();
        let rect = Rect::new(position.x, position.y, size.x, size.y);
        let final_rect = rect.intersection(screen_rect);
        if final_rect.w == 0 && final_rect.h == 0 {
            return Ok(());
        }
        let image = self.image.content();
        for y in final_rect.y..(final_rect.y + final_rect.h) {
            queue!(stream, MoveTo(final_rect.x as _, y as _))?;
            for x in final_rect.x..(final_rect.x + final_rect.w) {
                if let Some(symbol) = image.get((Vec2::new(x, y) - position).as_::<usize>()) {
                    write!(stream, "{}", symbol)?;
                }
            }
        }
        Ok(())
    }
}
