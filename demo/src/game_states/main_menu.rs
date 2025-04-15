use crate::{
    components::{
        drawable::{Drawable, screen_rect},
        sprite::Sprite,
    },
    resources::{
        assets::Assets,
        game_state::{GameState, GameStateChange},
    },
    utils::image::{Image, ImageContent},
};
use anput::prelude::*;
use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode, KeyEventKind},
    queue,
};
use std::{
    error::Error,
    io::{Write, stdout},
};

#[derive(Debug, Default)]
pub struct MainMenuState {
    logo: Image,
}

impl GameState for MainMenuState {
    fn on_enter(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        self.logo = universe
            .resources
            .get_mut::<true, Assets<ImageContent>>()?
            .get("logo.txt")?
            .into();
        Ok(())
    }

    fn on_event(&mut self, universe: &mut Universe, event: &Event) -> Result<(), Box<dyn Error>> {
        let mut change = universe.resources.get_mut::<true, GameStateChange>()?;

        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Enter => {}
                    KeyCode::Esc => {
                        *change = GameStateChange::clear();
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn on_frame_end(&mut self, _: &mut Universe) -> Result<(), Box<dyn Error>> {
        let mut stream = stdout();
        let screen_rect = screen_rect()?;

        Sprite::new(self.logo.clone()).draw(&mut stream, Default::default(), screen_rect)?;

        queue!(stream, MoveTo(0, self.logo.content().size().y as _))?;
        writeln!(&mut stream, "Press ENTER to start!")?;
        writeln!(&mut stream, "Press ESC to exit!")?;

        Ok(())
    }
}
