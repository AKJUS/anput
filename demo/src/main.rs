pub mod components;
pub mod game;
pub mod resources;
pub mod systems;
pub mod utils;

use crate::game::Game;
use spitfire_draw::utils::Vertex;
use spitfire_glow::app::App;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "tracing")]
    let _guard = {
        use tracing_chrome::ChromeLayerBuilder;
        use tracing_subscriber::prelude::*;

        std::env::set_current_dir(std::path::Path::new(std::env!("CARGO_MANIFEST_DIR")))?;
        let (chrome_layer, _guard) = ChromeLayerBuilder::new()
            .file("./trace.json")
            .include_args(true)
            .include_locations(true)
            .build();
        tracing_subscriber::registry().with(chrome_layer).init();
        _guard
    };

    App::<Vertex>::default().run(Game::default());
    Ok(())
}
