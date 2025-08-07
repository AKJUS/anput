pub mod components;
pub mod control_player;
pub mod diagnostics;
pub mod game;
pub mod renderers;
pub mod utils;

use crate::game::Game;
use spitfire_draw::utils::Vertex;
use spitfire_glow::app::App;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(debug_assertions)]
    {
        std::env::set_current_dir(std::path::Path::new(std::env!("CARGO_MANIFEST_DIR")))?;
    }

    App::<Vertex>::default().run(Game::default());
    Ok(())
}
