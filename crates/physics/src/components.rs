use crate::{PhysicsAccessView, Scalar};
use anput::{
    entity::Entity,
    query::TypedLookupFetch,
    world::{Relation, World},
};
use serde::{Deserialize, Serialize};
use vek::{Quaternion, Vec3};

#[derive(Clone)]
pub struct BodyAccessInfo {
    pub entity: Entity,
    pub view: PhysicsAccessView,
}

impl BodyAccessInfo {
    pub fn new(entity: Entity, view: PhysicsAccessView) -> Self {
        Self { entity, view }
    }

    pub fn of_world(entity: Entity, world: &World) -> Self {
        Self::new(entity, PhysicsAccessView::new(world))
    }

    pub fn particles<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING> + 'a>(
        &'a self,
    ) -> impl Iterator<Item = Fetch::Value> + 'a {
        self.view
            .entity::<LOCKING, &Relation<BodyParticleRelation>>(self.entity)
            .map(|relations| {
                self.view
                    .lookup::<LOCKING, Fetch>(relations.iter().map(|(_, entity)| entity))
            })
            .into_iter()
            .flatten()
    }

    pub fn density_fields<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING> + 'a>(
        &'a self,
    ) -> impl Iterator<Item = Fetch::Value> + 'a {
        self.view
            .entity::<LOCKING, &Relation<BodyDensityFieldRelation>>(self.entity)
            .map(|relations| {
                self.view
                    .lookup::<LOCKING, Fetch>(relations.iter().map(|(_, entity)| entity))
            })
            .into_iter()
            .flatten()
    }
}

pub struct PhysicsBody;
pub struct PhysicsParticle;
pub struct BodyParticleRelation;
pub struct BodyDensityFieldRelation;
pub struct ParticleConstraintRelation;
pub struct BodyParentRelation;

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Mass {
    value: Scalar,
    inverse: Scalar,
}

impl Mass {
    pub fn new(value: Scalar) -> Self {
        Self {
            value,
            inverse: if value != 0.0 { 1.0 / value } else { 0.0 },
        }
    }

    pub fn new_inverse(inverse: Scalar) -> Self {
        Self {
            value: if inverse != 0.0 { 1.0 / inverse } else { 0.0 },
            inverse,
        }
    }

    pub fn value(&self) -> Scalar {
        self.value
    }

    pub fn inverse(&self) -> Scalar {
        self.inverse
    }
}

impl PartialEq for Mass {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub current: Vec3<Scalar>,
    previous: Vec3<Scalar>,
}

impl Position {
    pub fn new(current: impl Into<Vec3<Scalar>>) -> Self {
        let current = current.into();
        Self {
            current,
            previous: current,
        }
    }

    pub fn previous(&self) -> Vec3<Scalar> {
        self.previous
    }

    pub fn change(&self) -> Vec3<Scalar> {
        self.current - self.previous
    }

    pub fn cache_current_as_previous(&mut self) {
        self.previous = self.current;
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.current == other.current
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rotation {
    pub current: Quaternion<Scalar>,
    previous: Quaternion<Scalar>,
}

impl Rotation {
    pub fn new(current: impl Into<Quaternion<Scalar>>) -> Self {
        let current = current.into();
        Self {
            current,
            previous: current,
        }
    }

    pub fn previous(&self) -> Quaternion<Scalar> {
        self.previous
    }

    pub fn change(&self) -> Quaternion<Scalar> {
        self.current * self.previous.conjugate()
    }

    pub fn cache_current_as_previous(&mut self) {
        self.previous = self.current;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct LinearVelocity {
    pub value: Vec3<Scalar>,
}

impl LinearVelocity {
    pub fn new(value: Vec3<Scalar>) -> Self {
        Self { value }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct AngularVelocity {
    pub value: Vec3<Scalar>,
}

impl AngularVelocity {
    pub fn new(value: Vec3<Scalar>) -> Self {
        Self { value }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalForces {
    pub force: Vec3<Scalar>,
    pub torque: Vec3<Scalar>,
    pub linear_impulse: Vec3<Scalar>,
    pub angular_impulse: Vec3<Scalar>,
}

impl ExternalForces {
    pub fn accumulate_force(&mut self, force: Vec3<Scalar>) {
        self.force += force;
    }

    pub fn accumulate_torque(&mut self, torque: Vec3<Scalar>) {
        self.torque += torque;
    }

    pub fn accumulate_linear_impulse(&mut self, impulse: Vec3<Scalar>) {
        self.linear_impulse += impulse;
    }

    pub fn accumulate_angular_impulse(&mut self, impulse: Vec3<Scalar>) {
        self.angular_impulse += impulse;
    }

    pub fn clear_continuous(&mut self) {
        self.force = Vec3::zero();
        self.torque = Vec3::zero();
    }

    pub fn clear_instantaneous(&mut self) {
        self.linear_impulse = Vec3::zero();
        self.angular_impulse = Vec3::zero();
    }

    pub fn clear(&mut self) {
        self.clear_continuous();
        self.clear_instantaneous();
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gravity {
    pub value: Vec3<Scalar>,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsMaterial {
    pub friction: Scalar,
    pub restitution: Scalar,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            friction: 0.5,
            restitution: 0.0,
        }
    }
}
