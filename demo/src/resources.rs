use anput::{systems::SystemContext, universe::UniverseCondition};
use spitfire_input::{ArrayInputCombinator, CardinalInputCombinator, InputActionRef};
use std::time::{Duration, Instant};

pub struct Clock {
    pub fixed_step_timer: Instant,
    pub variable_step_timer: Instant,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            fixed_step_timer: Instant::now(),
            variable_step_timer: Instant::now(),
        }
    }
}

impl Clock {
    pub fn fixed_step_elapsed(&self) -> Duration {
        self.fixed_step_timer.elapsed()
    }

    pub fn variable_step_elapsed(&self) -> Duration {
        self.variable_step_timer.elapsed()
    }
}

#[derive(Default)]
pub struct Inputs {
    pub mouse_xy: ArrayInputCombinator<2>,
    pub mouse_trigger: InputActionRef,
    pub movement: CardinalInputCombinator,
    pub jump: InputActionRef,
    pub reset_movement: InputActionRef,
    pub switch_render_mode: InputActionRef,
    pub switch_spawn_mode: InputActionRef,
    pub toggle_simulation: InputActionRef,
}

#[derive(Debug, Default)]
pub struct Globals {
    pub render_mode: RenderMode,
    pub spawn_mode: SpawnMode,
    pub paused_simulation: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    #[default]
    Objects,
    DensityFields,
}

impl RenderMode {
    pub fn switch(&mut self) {
        *self = match self {
            RenderMode::Objects => RenderMode::DensityFields,
            RenderMode::DensityFields => RenderMode::Objects,
        };
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SpawnMode {
    #[default]
    Cube,
    Sphere,
}

impl SpawnMode {
    pub fn switch(&mut self) {
        *self = match self {
            SpawnMode::Cube => SpawnMode::Sphere,
            SpawnMode::Sphere => SpawnMode::Cube,
        };
    }
}

pub struct ShouldRunSimulation;

impl UniverseCondition for ShouldRunSimulation {
    fn evaluate(context: SystemContext) -> bool {
        !context
            .universe
            .resources
            .get::<true, Globals>()
            .map(|globals| globals.paused_simulation)
            .unwrap_or_default()
    }
}
