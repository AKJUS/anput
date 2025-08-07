use crate::{
    PhysicsSimulation, Scalar,
    components::{
        AngularVelocity, ExternalForces, Gravity, LinearVelocity, Mass, Position, Rotation,
    },
    utils::quat_from_axis_angle,
};
use anput::{query::Query, systems::SystemContext, universe::Res, world::World};
use std::error::Error;

pub fn apply_external_forces<const LOCKING: bool>(
    context: SystemContext,
) -> Result<(), Box<dyn Error>> {
    let (world, simulation, query) = context.fetch::<(
        &World,
        Res<LOCKING, &PhysicsSimulation>,
        Query<
            LOCKING,
            (
                &mut ExternalForces,
                &Mass,
                &mut LinearVelocity,
                Option<&mut AngularVelocity>,
            ),
        >,
    )>()?;

    for (external_forces, mass, linear_velocity, angular_velocity) in query.query(world) {
        linear_velocity.value += external_forces.force * mass.inverse() * simulation.delta_time;
        linear_velocity.value += external_forces.linear_impulse * mass.inverse();

        if let Some(angular_velocity) = angular_velocity {
            angular_velocity.value +=
                external_forces.torque * mass.inverse() * simulation.delta_time;
            angular_velocity.value += external_forces.angular_impulse * mass.inverse();
        }

        external_forces.clear();
    }

    Ok(())
}

pub fn integrate_velocities<const LOCKING: bool>(
    context: SystemContext,
) -> Result<(), Box<dyn Error>> {
    let (world, simulation, query) = context.fetch::<(
        &World,
        Res<LOCKING, &PhysicsSimulation>,
        Query<
            LOCKING,
            (
                &mut Position,
                Option<&mut Rotation>,
                &LinearVelocity,
                Option<&AngularVelocity>,
            ),
        >,
    )>()?;

    for (position, rotation, linear_velocity, angular_velocity) in query.query(world) {
        position.current += linear_velocity.value * simulation.delta_time;

        if let Some(rotation) = rotation
            && let Some(angular_velocity) = angular_velocity
        {
            let angle = angular_velocity.value.magnitude() * simulation.delta_time;
            if angle.abs() > Scalar::EPSILON {
                let axis = angular_velocity.value / angle;
                rotation.current =
                    (rotation.current * quat_from_axis_angle(axis, angle)).normalized();
            }
        }
    }

    Ok(())
}

pub fn cache_current_as_previous_state<const LOCKING: bool>(
    context: SystemContext,
) -> Result<(), Box<dyn Error>> {
    let (world, query) = context.fetch::<(
        &World,
        Query<LOCKING, (&mut Position, Option<&mut Rotation>)>,
    )>()?;

    for (position, rotation) in query.query(world) {
        position.cache_current_as_previous();
        if let Some(rotation) = rotation {
            rotation.cache_current_as_previous();
        }
    }

    Ok(())
}

pub fn recalculate_velocities<const LOCKING: bool>(
    context: SystemContext,
) -> Result<(), Box<dyn Error>> {
    let (world, simulation, query) = context.fetch::<(
        &World,
        Res<LOCKING, &PhysicsSimulation>,
        Query<
            LOCKING,
            (
                &Position,
                Option<&Rotation>,
                &mut LinearVelocity,
                Option<&mut AngularVelocity>,
            ),
        >,
    )>()?;

    let inverse_delta_time = simulation.inverse_delta_time();

    for (position, rotation, linear_velocity, angular_velocity) in query.query(world) {
        linear_velocity.value += position.change() * inverse_delta_time;

        if let Some(rotation) = rotation
            && let Some(velocity) = angular_velocity
        {
            let (angle, axis) = rotation.change().into_angle_axis();
            velocity.value += axis * (angle * inverse_delta_time);
        }
    }

    Ok(())
}

pub fn apply_gravity<const LOCKING: bool>(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, simulation, query) = context.fetch::<(
        &World,
        Res<LOCKING, &PhysicsSimulation>,
        Query<LOCKING, (Option<&Gravity>, &mut ExternalForces)>,
    )>()?;

    for (gravity, external_forces) in query.query(world) {
        let gravity = gravity.map(|v| v.value).unwrap_or(simulation.gravity);
        external_forces.accumulate_linear_impulse(gravity * simulation.delta_time);
    }

    Ok(())
}
