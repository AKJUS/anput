use crate::{
    components::PlayerControlled,
    game::{Clock, Inputs},
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
use send_wrapper::SendWrapper;
use std::error::Error;

const SPEED: f32 = 100.0;
const JUMP: f32 = 200.0;

pub fn control_player(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, clock, inputs, contacts, query) = context.fetch::<(
        &World,
        Res<true, &Clock>,
        Res<true, &SendWrapper<Inputs>>,
        Res<true, &ContactsCache>,
        Query<
            true,
            (
                Entity,
                &mut ExternalForces,
                &mut LinearVelocity,
                &mut Position,
                Include<PlayerControlled>,
            ),
        >,
    )>()?;

    let delta_time = clock.variable_step_elapsed().as_secs_f32();

    for (entity, forces, velocity, position, _) in query.query(world) {
        forces.accumulate_linear_impulse(Vec3::new(
            inputs.movement.get() * SPEED * delta_time,
            0.0,
            0.0,
        ));

        if inputs.jump.get().is_pressed() && contacts.has_blocking_contact_of(entity) {
            forces.accumulate_linear_impulse(Vec3::new(0.0, -JUMP, 0.0));
        }

        if inputs.reset.get().is_hold() {
            velocity.value = Vec3::zero();
            forces.clear();
            position.cache_current_as_previous();
        }
    }

    Ok(())
}
