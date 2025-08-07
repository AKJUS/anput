use crate::{components::PlayerControlled, game::Inputs};
use anput::{
    query::{Include, Query},
    systems::SystemContext,
    universe::Res,
    world::World,
};
use anput_physics::{components::ExternalForces, third_party::vek::Vec3};
use send_wrapper::SendWrapper;
use std::error::Error;

const SPEED: f32 = 10.0;

pub fn control_player(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, inputs, query) = context.fetch::<(
        &World,
        Res<true, &SendWrapper<Inputs>>,
        Query<true, (&mut ExternalForces, Include<PlayerControlled>)>,
    )>()?;

    for (forces, _) in query.query(world) {
        let [x, y] = inputs.movement.get();
        forces.accumulate_linear_impulse(Vec3::new(x * SPEED, y * SPEED, 0.0));
    }

    Ok(())
}
