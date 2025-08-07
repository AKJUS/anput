use crate::{
    Scalar,
    components::Position,
    density_fields::{BodyAccessInfo, DensityField, DensityRange},
};
use vek::{Aabb, Vec3};

pub struct SphereDensityField<const LOCKING: bool> {
    pub density: Scalar,
    pub radius: Scalar,
    pub edge_thickness: Scalar,
}

impl<const LOCKING: bool> SphereDensityField<LOCKING> {
    pub fn new_hard(density: Scalar, radius: Scalar) -> Self {
        SphereDensityField {
            density,
            radius,
            edge_thickness: 0.0,
        }
    }

    pub fn new_soft(density: Scalar, radius: Scalar) -> Self {
        SphereDensityField {
            density,
            radius: 0.0,
            edge_thickness: radius,
        }
    }

    pub fn new_soft_edge(density: Scalar, radius: Scalar, edge_thickness: Scalar) -> Self {
        SphereDensityField {
            density,
            radius,
            edge_thickness,
        }
    }

    #[inline]
    pub fn total_radius(&self) -> Scalar {
        self.radius + self.edge_thickness
    }
}

impl<const LOCKING: bool> DensityField for SphereDensityField<LOCKING> {
    fn aabb(&self, info: &BodyAccessInfo) -> Aabb<Scalar> {
        info.particles::<LOCKING, &Position>()
            .map(|position| Aabb {
                min: position.current - self.total_radius(),
                max: position.current + self.total_radius(),
            })
            .reduce(|accum, aabb| accum.union(aabb))
            .unwrap_or_default()
    }

    fn density_at_point(&self, point: Vec3<Scalar>, info: &BodyAccessInfo) -> Scalar {
        info.particles::<LOCKING, &Position>()
            .map(|position| {
                let distance = position.current.distance(point);
                if distance < self.radius {
                    self.density
                } else {
                    1.0 - ((distance - self.radius) / self.edge_thickness).clamp(0.0, 1.0)
                }
            })
            .reduce(|accum, density| accum.max(density))
            .unwrap_or_default()
    }

    fn density_at_region(&self, region: Aabb<Scalar>, info: &BodyAccessInfo) -> DensityRange {
        [
            region.center(),
            Vec3::new(region.min.x, region.min.y, region.min.z),
            Vec3::new(region.max.x, region.min.y, region.min.z),
            Vec3::new(region.min.x, region.max.y, region.min.z),
            Vec3::new(region.max.x, region.max.y, region.min.z),
            Vec3::new(region.min.x, region.min.y, region.max.z),
            Vec3::new(region.max.x, region.min.y, region.max.z),
            Vec3::new(region.min.x, region.max.y, region.max.z),
            Vec3::new(region.max.x, region.max.y, region.max.z),
        ]
        .into_iter()
        .map(|point| DensityRange::converged(self.density_at_point(point, info)))
        .reduce(|accum, density| accum.min_max(&density))
        .unwrap_or_default()
    }

    fn normal_at_point(
        &self,
        point: Vec3<Scalar>,
        _: Vec3<Scalar>,
        info: &BodyAccessInfo,
    ) -> Vec3<Scalar> {
        info.particles::<LOCKING, &Position>()
            .map(|position| {
                let direction = point - position.current;
                if direction.is_approx_zero() {
                    position.change()
                } else {
                    direction
                }
            })
            .reduce(|accum, direction| accum + direction)
            .and_then(|normal| normal.try_normalized())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        components::{
            BodyDensityFieldRelation, BodyParentRelation, BodyParticleRelation, PhysicsBody,
            PhysicsParticle,
        },
        density_fields::{DensityFieldBox, DensityRange},
    };
    use anput::world::World;

    #[test]
    fn test_sphere_density_field() {
        let mut world = World::default();
        let object = world
            .spawn((
                PhysicsBody,
                PhysicsParticle,
                Position::new(Vec3::new(1.0, 2.0, 3.0)),
                DensityFieldBox::new(SphereDensityField::<true>::new_hard(1.0, 10.0)),
            ))
            .unwrap();
        world
            .relate::<true, _>(BodyParticleRelation, object, object)
            .unwrap();
        world
            .relate::<true, _>(BodyDensityFieldRelation, object, object)
            .unwrap();
        world
            .relate::<true, _>(BodyParentRelation, object, object)
            .unwrap();

        let sphere = world
            .entity::<true, &DensityFieldBox>(object)
            .unwrap()
            .as_any()
            .downcast_ref::<SphereDensityField<true>>()
            .unwrap();
        let info = BodyAccessInfo::of_world(object, &world);

        assert_eq!(
            sphere.aabb(&info),
            Aabb {
                min: Vec3::new(-9.0, -8.0, -7.0),
                max: Vec3::new(11.0, 12.0, 13.0),
            }
        );

        assert_eq!(
            sphere.density_at_point(Vec3::new(1.0, 2.0, 3.0), &info),
            1.0
        );
        assert_eq!(
            sphere.density_at_point(Vec3::new(-9.0, -8.0, -7.0), &info),
            0.0
        );
        assert_eq!(
            sphere.density_at_point(Vec3::new(11.0, 12.0, 13.0), &info),
            0.0
        );

        assert_eq!(
            sphere.density_at_region(
                Aabb {
                    min: Vec3::new(-9.0, -8.0, -7.0),
                    max: Vec3::new(11.0, 12.0, 13.0)
                },
                &info
            ),
            DensityRange { min: 0.0, max: 1.0 }
        );
        assert_eq!(
            sphere.density_at_region(
                Aabb {
                    min: Vec3::new(-4.0, -3.0, -2.0),
                    max: Vec3::new(6.0, 7.0, 8.0)
                },
                &info
            ),
            DensityRange { min: 1.0, max: 1.0 }
        );
        assert_eq!(
            sphere.density_at_region(
                Aabb {
                    min: Vec3::new(10.0, 10.0, 10.0),
                    max: Vec3::new(11.0, 11.0, 11.0)
                },
                &info
            ),
            DensityRange { min: 0.0, max: 0.0 }
        );

        assert_eq!(
            sphere.normal_at_point(Vec3::new(1.0, 2.0, 3.0), Default::default(), &info),
            Vec3::zero()
        );
        assert_eq!(
            sphere.normal_at_point(Vec3::new(2.0, 2.0, 3.0), Default::default(), &info),
            Vec3::new(1.0, 0.0, 0.0)
        );
        assert_eq!(
            sphere.normal_at_point(Vec3::new(0.0, 2.0, 3.0), Default::default(), &info),
            Vec3::new(-1.0, 0.0, 0.0)
        );
        assert_eq!(
            sphere.normal_at_point(Vec3::new(1.0, 3.0, 3.0), Default::default(), &info),
            Vec3::new(0.0, 1.0, 0.0)
        );
        assert_eq!(
            sphere.normal_at_point(Vec3::new(1.0, 1.0, 3.0), Default::default(), &info),
            Vec3::new(0.0, -1.0, 0.0)
        );
        assert_eq!(
            sphere.normal_at_point(Vec3::new(2.0, 3.0, 3.0), Default::default(), &info),
            Vec3::new(1.0, 1.0, 0.0).normalized()
        );
    }
}
