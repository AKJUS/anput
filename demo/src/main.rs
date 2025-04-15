pub mod components;
pub mod game_states;
pub mod resources;
pub mod systems;
pub mod utils;

use anput::prelude::*;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{DisableMouseCapture, EnableMouseCapture, poll, read},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use game_states::main_menu::MainMenuState;
use resources::{
    assets::Assets,
    game_state::{GameStateChange, GameStateStack},
};
use std::{
    error::Error,
    path::Path,
    time::{Duration, Instant},
};
use utils::image::ImageAssetFactory;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(debug_assertions)]
    {
        std::env::set_current_dir(Path::new(std::env!("CARGO_MANIFEST_DIR")))?;
    }

    let universe = Universe::default()
        .with_basics(10240, 10240)
        .with_resource(GameStateStack::default())?
        .with_resource(GameStateChange::push(MainMenuState::default()))?
        .with_resource(Assets::new(ImageAssetFactory).with_root_relative("assets"))?;

    run(universe)
}

fn run(mut universe: Universe) -> Result<(), Box<dyn Error>> {
    let mut stdout = std::io::stdout();
    let mut timer = Instant::now();
    let mut scheduler = GraphScheduler::<true>::default();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;

    while let Ok(available) = poll(Duration::ZERO) {
        let change = universe
            .resources
            .get_mut::<true, GameStateChange>()?
            .take();
        let exit = GameStateStack::as_resource(&mut universe, |stack, universe| {
            stack.execute_change(universe, change)?;
            Ok(stack.is_empty())
        })?;

        if exit {
            break;
        }

        if available {
            let event = read()?;
            GameStateStack::as_resource(&mut universe, |stack, universe| {
                for state in stack.states() {
                    state.on_event(universe, &event)?;
                }
                Ok(())
            })?;
        }

        if timer.elapsed() >= Duration::from_millis(16) {
            execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            GameStateStack::as_resource(&mut universe, |stack, universe| {
                for state in stack.states() {
                    state.on_frame_begin(universe)?;
                }
                Ok(())
            })?;
            scheduler.run(&mut universe)?;
            GameStateStack::as_resource(&mut universe, |stack, universe| {
                for state in stack.states() {
                    state.on_frame_end(universe)?;
                }
                Ok(())
            })?;
            execute!(stdout)?;
            timer = Instant::now();
        }
    }

    execute!(stdout, Show, DisableMouseCapture, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
