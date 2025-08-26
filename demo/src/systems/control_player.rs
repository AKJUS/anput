use crate::{
    components::PlayerControlled,
    resources::{Clock, Globals, Inputs},
};
use anput::{
    entity::Entity,
    query::{Include, Query},
    systems::SystemContext,
    universe::Res,
    world::World,
};
use anput_physics::{
    collisions::ContactsCache,
    components::{ExternalForces, LinearVelocity, Position},
    third_party::vek::Vec3,
};
use std::error::Error;

const SPEED: f32 = 100.0;
const JUMP: f32 = 200.0;

pub fn control_player(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, clock, inputs, contacts, mut globals, player_query, movable_query) = context
        .fetch::<(
            &World,
            Res<true, &Clock>,
            Res<true, &Inputs>,
            Res<true, &ContactsCache>,
            Res<true, &mut Globals>,
            Query<true, (Entity, &mut ExternalForces, Include<PlayerControlled>)>,
            Query<true, (&mut ExternalForces, &mut LinearVelocity, &mut Position)>,
        )>()?;

    let delta_time = clock.variable_step_elapsed().as_secs_f32();

    for (entity, forces, _) in player_query.query(world) {
        let [x, y] = inputs.movement.get();
        forces.accumulate_linear_impulse(Vec3::new(
            x * SPEED * delta_time,
            y * SPEED * delta_time,
            0.0,
        ));

        if inputs.jump.get().is_pressed() && contacts.has_blocking_contact_of(entity) {
            forces.accumulate_linear_impulse(Vec3::new(0.0, -JUMP, 0.0));
        }
    }

    if inputs.reset_movement.get().is_hold() {
        for (forces, velocity, position) in movable_query.query(world) {
            forces.clear();
            velocity.value = Vec3::zero();
            position.cache_current_as_previous();
        }
    }

    if inputs.switch_render_mode.get().is_pressed() {
        globals.render_mode.switch();
    }

    if inputs.switch_spawn_mode.get().is_pressed() {
        globals.spawn_mode.switch();
    }

    if inputs.toggle_simulation.get().is_pressed() {
        globals.paused_simulation = !globals.paused_simulation;
    }

    Ok(())
}
