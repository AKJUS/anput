pub mod collisions;
pub mod components;
pub mod constraints;
pub mod density_fields;
pub mod queries;
pub mod solvers;
pub mod utils;

pub mod third_party {
    pub use vek;
}

use crate::{
    collisions::{
        CollisionProfile, ContactDetection, ContactsCache, DensityFieldSpatialExtractor,
        RepulsiveCollisionCallbacks, RepulsiveCollisionSolver, collect_contacts,
        dispatch_contact_events,
    },
    components::{
        AngularVelocity, BodyDensityFieldRelation, BodyParentRelation, BodyParticleRelation,
        ExternalForces, LinearVelocity, Mass, ParticleConstraintRelation, PhysicsBody,
        PhysicsMaterial, PhysicsParticle, Position,
    },
    density_fields::DensityFieldBox,
    queries::shape::ShapeOverlapQuery,
    solvers::{
        apply_external_forces, apply_gravity, cache_current_as_previous_state,
        integrate_velocities, recalculate_velocities,
    },
};
use anput::{scheduler::GraphSchedulerPlugin, view::TypedWorldView, world::Relation};
use serde::{Deserialize, Serialize};
use vek::Vec3;

pub type Scalar = f32;
pub use std::f32 as scalar;

pub type PhysicsAccessBundleColumns = (
    PhysicsBody,
    PhysicsParticle,
    PhysicsMaterial,
    Mass,
    Position,
    LinearVelocity,
    AngularVelocity,
    ExternalForces,
    CollisionProfile,
    DensityFieldBox,
    ContactDetection,
    Relation<BodyParentRelation>,
    Relation<BodyParticleRelation>,
    Relation<BodyDensityFieldRelation>,
    Relation<ParticleConstraintRelation>,
);

pub type PhysicsAccessView = TypedWorldView<PhysicsAccessBundleColumns>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsSimulation {
    pub delta_time: Scalar,
    pub gravity: Vec3<Scalar>,
}

impl Default for PhysicsSimulation {
    fn default() -> Self {
        Self {
            delta_time: 1.0 / 20.0,
            gravity: Default::default(),
        }
    }
}

impl PhysicsSimulation {
    pub fn inverse_delta_time(&self) -> Scalar {
        if self.delta_time.abs() > Scalar::EPSILON {
            1.0 / self.delta_time
        } else {
            0.0
        }
    }
}

pub struct PhysicsPlugin<const LOCKING: bool> {
    simulation: PhysicsSimulation,
    shape_overlap_query: ShapeOverlapQuery,
    install_repulsive_collision: bool,
    install_apply_gravity: bool,
    install_apply_external_forces: bool,
    install_integrate_velocities: bool,
    install_collect_contacts: bool,
    install_dispatch_contact_events: bool,
    repulsive_collision_callbacks: RepulsiveCollisionCallbacks,
}

impl<const LOCKING: bool> Default for PhysicsPlugin<LOCKING> {
    fn default() -> Self {
        Self {
            simulation: Default::default(),
            shape_overlap_query: Default::default(),
            install_repulsive_collision: true,
            install_apply_gravity: true,
            install_apply_external_forces: true,
            install_integrate_velocities: true,
            install_collect_contacts: true,
            install_dispatch_contact_events: true,
            repulsive_collision_callbacks: Default::default(),
        }
    }
}

impl<const LOCKING: bool> PhysicsPlugin<LOCKING> {
    pub fn barebones() -> Self {
        Self {
            simulation: PhysicsSimulation::default(),
            shape_overlap_query: Default::default(),
            install_repulsive_collision: false,
            install_apply_gravity: false,
            install_apply_external_forces: false,
            install_integrate_velocities: false,
            install_collect_contacts: false,
            install_dispatch_contact_events: false,
            repulsive_collision_callbacks: Default::default(),
        }
    }

    pub fn simulation(mut self, simulation: PhysicsSimulation) -> Self {
        self.simulation = simulation;
        self
    }

    pub fn shape_overlap_query(mut self, query: ShapeOverlapQuery) -> Self {
        self.shape_overlap_query = query;
        self
    }

    pub fn install_repulsive_collision(mut self, install: bool) -> Self {
        self.install_repulsive_collision = install;
        self
    }

    pub fn install_apply_gravity(mut self, install: bool) -> Self {
        self.install_apply_gravity = install;
        self
    }

    pub fn install_apply_external_forces(mut self, install: bool) -> Self {
        self.install_apply_external_forces = install;
        self
    }

    pub fn install_integrate_velocities(mut self, install: bool) -> Self {
        self.install_integrate_velocities = install;
        self
    }

    pub fn install_collect_contacts(mut self, install: bool) -> Self {
        self.install_collect_contacts = install;
        self
    }

    pub fn repulsive_collision_callbacks(mut self, callbacks: RepulsiveCollisionCallbacks) -> Self {
        self.repulsive_collision_callbacks = callbacks;
        self
    }

    pub fn make(self) -> GraphSchedulerPlugin<LOCKING> {
        let Self {
            simulation,
            shape_overlap_query,
            install_repulsive_collision,
            install_apply_gravity,
            install_apply_external_forces,
            install_integrate_velocities,
            install_collect_contacts,
            install_dispatch_contact_events,
            repulsive_collision_callbacks,
        } = self;

        GraphSchedulerPlugin::<LOCKING>::default()
            .name("physics_simulation")
            .resource(simulation)
            .resource(ContactsCache::default())
            .plugin_setup(|plugin| {
                plugin
                    .name("pre_simulation")
                    .maybe_setup(|plugin| {
                        if install_apply_gravity {
                            Some(plugin.system_setup(apply_gravity::<LOCKING>, |system| {
                                system.name("apply_gravity")
                            }))
                        } else {
                            None
                        }
                    })
                    .maybe_setup(|plugin| {
                        if install_apply_external_forces {
                            Some(
                                plugin.system_setup(apply_external_forces::<LOCKING>, |system| {
                                    system.name("apply_external_forces")
                                }),
                            )
                        } else {
                            None
                        }
                    })
                    .maybe_setup(|plugin| {
                        if install_integrate_velocities {
                            Some(
                                plugin.system_setup(integrate_velocities::<LOCKING>, |system| {
                                    system.name("integrate_velocities")
                                }),
                            )
                        } else {
                            None
                        }
                    })
                    .plugin(
                        anput_spatial::make_plugin::<LOCKING, DensityFieldSpatialExtractor>()
                            .name("extract_spatial_info"),
                    )
                    .maybe_setup(|plugin| {
                        if install_collect_contacts {
                            Some(plugin.system_setup(collect_contacts::<LOCKING>, |system| {
                                system.name("collect_contacts").local(shape_overlap_query)
                            }))
                        } else {
                            None
                        }
                    })
                    .maybe_setup(|plugin| {
                        if install_dispatch_contact_events {
                            Some(
                                plugin.system_setup(dispatch_contact_events::<LOCKING>, |system| {
                                    system.name("dispatch_contact_events")
                                }),
                            )
                        } else {
                            None
                        }
                    })
                    .maybe_setup(|plugin| {
                        if install_repulsive_collision {
                            Some(plugin.system_setup(
                                RepulsiveCollisionSolver::<LOCKING>,
                                |system| {
                                    system
                                        .name("RepulsiveCollisionSolver")
                                        .local(repulsive_collision_callbacks)
                                },
                            ))
                        } else {
                            None
                        }
                    })
            })
            .plugin_setup(|plugin| {
                plugin
                    .name("pre_solvers")
                    .system_setup(cache_current_as_previous_state::<LOCKING>, |system| {
                        system.name("cache_current_as_previous_state")
                    })
            })
            .plugin_setup(|plugin| plugin.name("solvers"))
            .plugin_setup(|plugin| {
                plugin
                    .name("post_solvers")
                    .system_setup(recalculate_velocities::<LOCKING>, |system| {
                        system.name("recalculate_velocities")
                    })
            })
            .plugin_setup(|plugin| plugin.name("post_simulation"))
    }
}
