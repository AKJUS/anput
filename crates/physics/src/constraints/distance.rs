use crate::{
    PhysicsSimulation, Scalar,
    components::{Mass, PhysicsParticle, Position, Rotation},
    utils::quat_from_axis_angle,
};
use anput::{
    query::{Include, Lookup},
    systems::SystemContext,
    universe::Res,
    world::World,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

/// Relation between two particles.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DistanceConstraint {
    pub distance: Scalar,
    pub compliance: Scalar,
    pub lambda: Scalar,
}

pub fn solve_distance_constraint<const LOCKING: bool>(
    context: SystemContext,
) -> Result<(), Box<dyn Error>> {
    let (world, simulation, particle_lookup) = context.fetch::<(
        &World,
        Res<LOCKING, &PhysicsSimulation>,
        Lookup<
            LOCKING,
            (
                &mut Position,
                Option<&mut Rotation>,
                &Mass,
                Include<PhysicsParticle>,
            ),
        >,
    )>()?;

    let mut particle_lookup = particle_lookup.lookup_access(world);

    for (from, constraint, to) in world.relations_mut::<LOCKING, DistanceConstraint>() {
        let Some((from_position, from_rotation, from_mass, _)) = particle_lookup.access(from)
        else {
            continue;
        };
        let Some((to_position, to_rotation, to_mass, _)) = particle_lookup.access(to) else {
            continue;
        };

        let from_weight = from_mass.inverse();
        let to_weight = to_mass.inverse();
        let delta = to_position.current - from_position.current;
        let distance = delta.magnitude();
        if distance < Scalar::EPSILON {
            continue;
        }
        let normal = delta / distance;
        let error = distance - constraint.distance;
        let alpha = constraint.compliance / (simulation.delta_time * simulation.delta_time);
        let lambda = -(error + alpha * constraint.lambda) / (from_weight + to_weight + alpha);
        let impulse = normal * lambda;

        constraint.lambda += lambda;
        from_position.current -= impulse * from_weight;
        to_position.current += impulse * to_weight;
        if let Some(from_rotation) = from_rotation {
            let angular_correction = normal.cross(-impulse) * from_weight;
            let angle = angular_correction.magnitude();
            if angle > Scalar::EPSILON {
                let axis = angular_correction / angle;
                let delta = quat_from_axis_angle(axis, angle);
                from_rotation.current = (from_rotation.current * delta).normalized();
            }
        }
        if let Some(to_rotation) = to_rotation {
            let angular_correction = normal.cross(impulse) * to_weight;
            let angle = angular_correction.magnitude();
            if angle > Scalar::EPSILON {
                let axis = angular_correction / angle;
                let delta = quat_from_axis_angle(axis, angle);
                to_rotation.current = (to_rotation.current * delta).normalized();
            }
        }
    }

    Ok(())
}
