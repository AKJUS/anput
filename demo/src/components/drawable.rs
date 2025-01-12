use crossterm::terminal::size;
use std::{error::Error, io::Stdout};
use vek::{Rect, Vec2};

pub fn screen_rect() -> Result<Rect<isize, isize>, Box<dyn Error>> {
    let screen_size = size()?;
    Ok(Rect::new(
        0isize,
        0,
        screen_size.0 as isize,
        screen_size.1 as isize,
    ))
}

pub trait Drawable {
    fn draw(
        &self,
        stream: &mut Stdout,
        position: Vec2<isize>,
        screen_rect: Rect<isize, isize>,
    ) -> Result<(), Box<dyn Error>>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DrawOrder(pub isize);
