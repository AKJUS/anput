use anput::prelude::*;
use crossterm::event::Event;
use std::error::Error;

pub trait GameState: Send + Sync + 'static {
    #[allow(unused_variables)]
    fn on_enter(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn on_exit(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn on_event(&mut self, universe: &mut Universe, event: &Event) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn on_frame_begin(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn on_frame_end(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl GameState for () {}

#[derive(Default)]
pub enum GameStateChange {
    #[default]
    None,
    Swap(Box<dyn GameState>),
    Push(Box<dyn GameState>),
    Pop,
    Clear,
}

impl GameStateChange {
    pub fn swap(state: impl GameState) -> Self {
        Self::Swap(Box::new(state))
    }

    pub fn push(state: impl GameState) -> Self {
        Self::Push(Box::new(state))
    }

    pub fn pop() -> Self {
        Self::Pop
    }

    pub fn clear() -> Self {
        Self::Clear
    }

    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

#[derive(Default)]
pub struct GameStateStack {
    states: Vec<Box<dyn GameState>>,
}

impl GameStateStack {
    pub fn as_resource<R>(
        universe: &mut Universe,
        f: impl FnOnce(&mut Self, &mut Universe) -> Result<R, Box<dyn Error>>,
    ) -> Result<R, Box<dyn Error>> {
        let mut stack = std::mem::take(&mut *universe.resources.get_mut::<true, GameStateStack>()?);
        let result = f(&mut stack, universe)?;
        *universe.resources.get_mut::<true, GameStateStack>()? = stack;
        Ok(result)
    }

    pub fn states(&mut self) -> impl Iterator<Item = &mut dyn GameState> {
        self.states.iter_mut().map(|state| &mut **state)
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn execute_change(
        &mut self,
        universe: &mut Universe,
        change: GameStateChange,
    ) -> Result<(), Box<dyn Error>> {
        match change {
            GameStateChange::None => {}
            GameStateChange::Swap(mut state) => {
                if let Some(mut state) = self.states.pop() {
                    state.on_exit(universe)?;
                }
                state.on_enter(universe)?;
                self.states.push(state);
            }
            GameStateChange::Push(mut state) => {
                state.on_enter(universe)?;
                self.states.push(state);
            }
            GameStateChange::Pop => {
                if let Some(mut state) = self.states.pop() {
                    state.on_exit(universe)?;
                }
            }
            GameStateChange::Clear => {
                while let Some(mut state) = self.states.pop() {
                    state.on_exit(universe)?;
                }
            }
        }
        Ok(())
    }
}
