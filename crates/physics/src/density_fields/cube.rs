use std::cmp::Ordering;

use crate::{Scalar, components::BodyAccessInfo, density_fields::DensityField};
use vek::{Aabb, Vec3};

pub struct CubeDensityField<const LOCKING: bool> {
    pub density: Scalar,
    pub extents: Vec3<Scalar>,
    pub edge_thickness: Vec3<Scalar>,
}

impl<const LOCKING: bool> CubeDensityField<LOCKING> {
    pub fn new_hard(density: Scalar, extents: Vec3<Scalar>) -> Self {
        Self {
            density,
            extents,
            edge_thickness: Default::default(),
        }
    }

    pub fn new_soft(density: Scalar, extents: Vec3<Scalar>) -> Self {
        Self {
            density,
            extents: Default::default(),
            edge_thickness: extents,
        }
    }

    pub fn new_soft_edge(
        density: Scalar,
        extents: Vec3<Scalar>,
        edge_thickness: Vec3<Scalar>,
    ) -> Self {
        Self {
            density,
            extents,
            edge_thickness,
        }
    }

    #[inline]
    pub fn total_extents(&self) -> Vec3<Scalar> {
        self.extents + self.edge_thickness
    }
}

impl<const LOCKING: bool> DensityField for CubeDensityField<LOCKING> {
    fn aabb(&self, info: &BodyAccessInfo) -> Aabb<Scalar> {
        info.world_space_particles::<LOCKING, ()>()
            .map(|(matrix, _)| {
                let extents = self.total_extents();
                let mut aabb = Aabb::new_empty(matrix.mul_point(Default::default()));
                for corner in [
                    Vec3::new(-extents.x, -extents.y, -extents.z),
                    Vec3::new(extents.x, -extents.y, -extents.z),
                    Vec3::new(extents.x, extents.y, -extents.z),
                    Vec3::new(-extents.x, extents.y, -extents.z),
                    Vec3::new(-extents.x, -extents.y, extents.z),
                    Vec3::new(extents.x, -extents.y, extents.z),
                    Vec3::new(extents.x, extents.y, extents.z),
                    Vec3::new(-extents.x, extents.y, extents.z),
                ] {
                    aabb.expand_to_contain_point(matrix.mul_point(corner));
                }
                aabb
            })
            .reduce(|accum, aabb| accum.union(aabb))
            .unwrap_or_default()
    }

    fn density_at_point(&self, point: Vec3<Scalar>, info: &BodyAccessInfo) -> Scalar {
        info.world_space_particles::<LOCKING, ()>()
            .map(|(matrix, _)| {
                let point = matrix.inverted().mul_point(point).into_array();
                let extents = self.extents.into_array();
                let edge_thickness = self.edge_thickness.into_array();
                (0..2)
                    .map(|index| {
                        let point = point[index].abs();
                        let extent = extents[index];
                        let edge = edge_thickness[index];
                        let distance = point - extent;
                        (edge, distance)
                    })
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
                    .map(|(edge, distance)| {
                        let factor = if distance < 0.0 {
                            1.0
                        } else if edge > Scalar::EPSILON {
                            1.0 - (distance / edge).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        factor * self.density
                    })
                    .unwrap_or_default()
            })
            .reduce(|accum, density| accum.max(density))
            .unwrap_or_default()
    }

    fn normal_at_point(
        &self,
        point: Vec3<Scalar>,
        _: Vec3<Scalar>,
        info: &BodyAccessInfo,
    ) -> Vec3<Scalar> {
        let extents = self.total_extents().into_array();
        info.world_space_particles::<LOCKING, ()>()
            .map(|(matrix, _)| {
                let inv_matrix = matrix.inverted();
                let point = inv_matrix.mul_point(point);
                if point.is_approx_zero() {
                    return Vec3::zero();
                }
                let direction = [
                    (
                        extents[0],
                        (point.x + extents[0]).abs(),
                        Vec3::new(-1.0, 0.0, 0.0),
                    ),
                    (
                        extents[1],
                        (point.y + extents[1]).abs(),
                        Vec3::new(0.0, -1.0, 0.0),
                    ),
                    (
                        extents[2],
                        (point.z + extents[2]).abs(),
                        Vec3::new(0.0, 0.0, -1.0),
                    ),
                    (
                        extents[0],
                        (extents[0] - point.x).abs(),
                        Vec3::new(1.0, 0.0, 0.0),
                    ),
                    (
                        extents[1],
                        (extents[1] - point.y).abs(),
                        Vec3::new(0.0, 1.0, 0.0),
                    ),
                    (
                        extents[2],
                        (extents[2] - point.z).abs(),
                        Vec3::new(0.0, 0.0, 1.0),
                    ),
                ]
                .into_iter()
                .filter(|(size, _, _)| *size > Scalar::EPSILON)
                .min_by(|(_, a, _), (_, b, _)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
                .map(|(_, _, normal)| normal)
                .unwrap_or_default();
                matrix.mul_direction(direction)
            })
            .reduce(|accum, direction| accum + direction)
            // TODO: maybe weight directions so ones closer to the surface have more influence?
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
            PhysicsParticle, Position,
        },
        density_fields::{DensityFieldBox, DensityRange},
    };
    use anput::world::World;

    #[test]
    fn test_cube_density_field() {
        let mut world = World::default();
        let object = world
            .spawn((
                PhysicsBody,
                PhysicsParticle,
                Position::new(Vec3::new(1.0, 2.0, 3.0)),
                DensityFieldBox::new(CubeDensityField::<true>::new_hard(1.0, 10.0.into())),
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

        let cube = world
            .entity::<true, &DensityFieldBox>(object)
            .unwrap()
            .as_any()
            .downcast_ref::<CubeDensityField<true>>()
            .unwrap();
        let info = BodyAccessInfo::of_world(object, &world);

        assert_eq!(
            cube.aabb(&info),
            Aabb {
                min: Vec3::new(-9.0, -8.0, -7.0),
                max: Vec3::new(11.0, 12.0, 13.0),
            }
        );

        assert_eq!(cube.density_at_point(Vec3::new(1.0, 2.0, 3.0), &info), 1.0);
        assert_eq!(
            cube.density_at_point(Vec3::new(-9.0, -8.0, -7.0), &info),
            0.0
        );
        assert_eq!(
            cube.density_at_point(Vec3::new(11.0, 12.0, 13.0), &info),
            0.0
        );
        assert_eq!(
            cube.density_at_point(Vec3::new(-8.0, -7.0, -6.0), &info),
            1.0
        );
        assert_eq!(
            cube.density_at_point(Vec3::new(10.0, 11.0, 12.0), &info),
            1.0
        );

        assert_eq!(
            cube.density_at_region(
                Aabb {
                    min: Vec3::new(-8.0, -7.0, -6.0),
                    max: Vec3::new(9.0, 10.0, 11.0)
                },
                &info
            ),
            DensityRange { min: 1.0, max: 1.0 }
        );
        assert_eq!(
            cube.density_at_region(
                Aabb {
                    min: Vec3::new(-10.0, -9.0, -8.0),
                    max: Vec3::new(12.0, 13.0, 14.0)
                },
                &info
            ),
            DensityRange { min: 0.0, max: 1.0 }
        );
        assert_eq!(
            cube.density_at_region(
                Aabb {
                    min: Vec3::new(100.0, 100.0, 100.0),
                    max: Vec3::new(200.0, 200.0, 200.0)
                },
                &info
            ),
            DensityRange { min: 0.0, max: 0.0 }
        );

        assert_eq!(
            cube.normal_at_point(Vec3::new(1.0, 2.0, 3.0), Default::default(), &info),
            Vec3::new(0.0, 0.0, 0.0)
        );
        assert_eq!(
            cube.normal_at_point(Vec3::new(2.0, 2.0, 3.0), Default::default(), &info),
            Vec3::new(1.0, 0.0, 0.0)
        );
        assert_eq!(
            cube.normal_at_point(Vec3::new(0.0, 2.0, 3.0), Default::default(), &info),
            Vec3::new(-1.0, 0.0, 0.0)
        );
        assert_eq!(
            cube.normal_at_point(Vec3::new(1.0, 3.0, 3.0), Default::default(), &info),
            Vec3::new(0.0, 1.0, 0.0)
        );
        assert_eq!(
            cube.normal_at_point(Vec3::new(1.0, 1.0, 3.0), Default::default(), &info),
            Vec3::new(0.0, -1.0, 0.0)
        );
    }
}
