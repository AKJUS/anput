use std::cmp::Ordering;

use crate::{
    Scalar,
    components::BodyAccessInfo,
    density_fields::{DensityField, DensityRange},
};
use vek::{Aabb, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeOverlapQuery {
    pub density_threshold: Scalar,
    pub voxelization_size_limit: Scalar,
    pub region_limit: Option<Aabb<Scalar>>,
    pub depth_limit: usize,
}

impl Default for ShapeOverlapQuery {
    fn default() -> Self {
        Self {
            density_threshold: 0.5,
            voxelization_size_limit: 1.0,
            region_limit: None,
            depth_limit: usize::MAX,
        }
    }
}

impl ShapeOverlapQuery {
    pub fn query_field_pair(
        &self,
        field: [&dyn DensityField; 2],
        info: [&BodyAccessInfo; 2],
        result: &mut Vec<ShapeOverlapCell>,
    ) -> Option<Aabb<Scalar>> {
        self.query_field_pair_mapped(field, info, result, |cell| cell)
    }

    pub fn query_field_pair_mapped<T>(
        &self,
        field: [&dyn DensityField; 2],
        info: [&BodyAccessInfo; 2],
        result: &mut Vec<T>,
        converter: impl Fn(ShapeOverlapCell) -> T,
    ) -> Option<Aabb<Scalar>> {
        let mut a = field[0].aabb(info[0]);
        let mut b = field[1].aabb(info[1]);
        if let Some(region_limit) = self.region_limit {
            a = a.intersection(region_limit);
            b = b.intersection(region_limit);
        }
        let aabb = intersecting_aabb_for_subdivisions(a, b)?;
        let mut stack = vec![(aabb, 0)];
        while let Some((region, depth)) = stack.pop() {
            let a = field[0].density_at_region(region, info[0]);
            let b = field[1].density_at_region(region, info[1]);
            if a.max.min(b.max) <= self.density_threshold {
                continue;
            }
            if region
                .size()
                .into_iter()
                .any(|v| v > self.voxelization_size_limit)
                && (a.has_separation() || b.has_separation())
                && depth < self.depth_limit
            {
                stack.extend(aabb_cell_subdivide(region).map(|region| (region, depth + 1)));
                continue;
            }
            let center = region.center();
            let density = [a, b];
            let resolution = Vec3::from(region.size()) * 0.5;
            let normal =
                [0, 1].map(|index| field[index].normal_at_point(center, resolution, info[index]));
            // TODO: remove?
            // potentially wrong way to compensate for shapes not reporting valid normals.
            let normal = match normal.map(|normal| normal.is_approx_zero()) {
                [true, true] | [false, false] => normal,
                [true, false] => [-normal[1], normal[1]],
                [false, true] => [normal[0], -normal[0]],
            };
            result.push(converter(ShapeOverlapCell {
                region,
                density,
                normal,
            }));
        }
        Some(aabb)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeOverlapCell {
    pub region: Aabb<Scalar>,
    pub density: [DensityRange; 2],
    pub normal: [Vec3<Scalar>; 2],
}

impl ShapeOverlapCell {
    pub fn area(&self) -> Scalar {
        self.region
            .size()
            .into_iter()
            .filter(|v| *v > Scalar::EPSILON)
            .product::<Scalar>()
    }

    pub fn normal_response(&self, responding_body_index: usize) -> Vec3<Scalar> {
        let surface_body_index = (responding_body_index + 1) % 2;
        self.normal[responding_body_index].reflected(self.normal[surface_body_index])
    }
}

pub fn intersecting_aabb_for_subdivisions(
    a: Aabb<Scalar>,
    b: Aabb<Scalar>,
) -> Option<Aabb<Scalar>> {
    Some(a.intersection(b)).filter(|aabb| {
        aabb.size()
            .into_iter()
            .filter(|value| *value >= Scalar::EPSILON)
            .count()
            > 1
    })
}

pub fn aabb_cell_subdivide(aabb: Aabb<Scalar>) -> [Aabb<Scalar>; 2] {
    let axis = aabb
        .size()
        .into_array()
        .into_iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
        .unwrap()
        .0;
    let center = (aabb.min[axis] + aabb.max[axis]) * 0.5;
    let mut a = aabb;
    let mut b = aabb;
    a.max[axis] = center;
    b.min[axis] = center;
    [a, b]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)]

    use super::*;
    use crate::{
        components::{
            BodyDensityFieldRelation, BodyParentRelation, BodyParticleRelation, PhysicsBody,
            PhysicsParticle, Position,
        },
        density_fields::{DensityFieldBox, aabb::AabbDensityField, sphere::SphereDensityField},
    };
    use anput::world::World;

    #[test]
    fn test_aabb() {
        let a = Aabb {
            min: Vec3::new(-2.0, -1.0, 0.0),
            max: Vec3::new(2.0, 1.0, 0.0),
        };
        let b = Aabb {
            min: Vec3::new(-1.0, -2.0, 0.0),
            max: Vec3::new(1.0, 2.0, 0.0),
        };

        let aabb = intersecting_aabb_for_subdivisions(a, b).unwrap();
        assert_eq!(
            aabb,
            Aabb {
                min: Vec3::new(-1.0, -1.0, 0.0),
                max: Vec3::new(1.0, 1.0, 0.0),
            }
        );

        let subdivided = aabb_cell_subdivide(aabb);
        assert_eq!(
            subdivided,
            [
                Aabb {
                    min: Vec3 {
                        x: -1.0,
                        y: -1.0,
                        z: 0.0
                    },
                    max: Vec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0
                    }
                },
                Aabb {
                    min: Vec3 {
                        x: -1.0,
                        y: 0.0,
                        z: 0.0
                    },
                    max: Vec3 {
                        x: 1.0,
                        y: 1.0,
                        z: 0.0
                    }
                }
            ]
        );

        let a = Aabb {
            min: Vec3::new(-2.0, 0.0, 0.0),
            max: Vec3::new(2.0, 0.0, 0.0),
        };
        let b = Aabb {
            min: Vec3::new(-1.0, 0.0, 0.0),
            max: Vec3::new(1.0, 0.0, 0.0),
        };

        assert!(intersecting_aabb_for_subdivisions(a, b).is_none());

        let a = Aabb {
            min: Vec3::new(-2.0, 0.0, -1.0),
            max: Vec3::new(2.0, 0.0, -1.0),
        };
        let b = Aabb {
            min: Vec3::new(-1.0, 0.0, 1.0),
            max: Vec3::new(1.0, 0.0, 1.0),
        };

        assert!(intersecting_aabb_for_subdivisions(a, b).is_none());

        let a = Aabb {
            min: Vec3::new(-10.0, -10.0, 0.0),
            max: Vec3::new(0.0, 10.0, 0.0),
        };
        let b = Aabb {
            min: Vec3::new(-5.0, -1.0, 0.0),
            max: Vec3::new(5.0, 1.0, 0.0),
        };

        let c = intersecting_aabb_for_subdivisions(a, b).unwrap();

        assert_eq!(
            c,
            Aabb {
                min: Vec3 {
                    x: -5.0,
                    y: -1.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0
                }
            }
        );

        let [a, b] = aabb_cell_subdivide(c);

        assert_eq!(
            a,
            Aabb {
                min: Vec3 {
                    x: -5.0,
                    y: -1.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: -2.5,
                    y: 1.0,
                    z: 0.0
                }
            }
        );
        assert_eq!(
            b,
            Aabb {
                min: Vec3 {
                    x: -2.5,
                    y: -1.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0
                }
            }
        );

        let [a, b] = aabb_cell_subdivide(a);

        assert_eq!(
            a,
            Aabb {
                min: Vec3 {
                    x: -5.0,
                    y: -1.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: -3.75,
                    y: 1.0,
                    z: 0.0
                }
            }
        );
        assert_eq!(
            b,
            Aabb {
                min: Vec3 {
                    x: -3.75,
                    y: -1.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: -2.5,
                    y: 1.0,
                    z: 0.0
                }
            }
        );

        let [a, b] = aabb_cell_subdivide(b);

        assert_eq!(
            a,
            Aabb {
                min: Vec3 {
                    x: -3.75,
                    y: -1.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: -2.5,
                    y: 0.0,
                    z: 0.0
                }
            }
        );
        assert_eq!(
            b,
            Aabb {
                min: Vec3 {
                    x: -3.75,
                    y: 0.0,
                    z: 0.0
                },
                max: Vec3 {
                    x: -2.5,
                    y: 1.0,
                    z: 0.0
                }
            }
        );
    }

    #[test]
    fn test_shape_overlap_query() {
        let mut world = World::default();

        let a = world
            .spawn((
                PhysicsBody,
                DensityFieldBox::new(AabbDensityField {
                    aabb: Aabb {
                        min: Vec3::new(-2.0, -2.0, 0.0),
                        max: Vec3::new(0.0, 0.0, 0.0),
                    },
                    density: 1.0,
                }),
            ))
            .unwrap();
        world
            .relate::<true, _>(BodyDensityFieldRelation, a, a)
            .unwrap();
        world.relate::<true, _>(BodyParentRelation, a, a).unwrap();

        let b = world
            .spawn((
                PhysicsBody,
                PhysicsParticle,
                Position::new(Vec3::new(0.0, 0.0, 0.0)),
                DensityFieldBox::new(SphereDensityField::<true>::new_hard(1.0, 1.0)),
            ))
            .unwrap();
        world.relate::<true, _>(BodyParticleRelation, b, b).unwrap();
        world
            .relate::<true, _>(BodyDensityFieldRelation, b, b)
            .unwrap();
        world.relate::<true, _>(BodyParentRelation, b, b).unwrap();

        let field_a = world
            .entity::<true, &DensityFieldBox>(a)
            .unwrap()
            .as_any()
            .downcast_ref::<AabbDensityField>()
            .unwrap();
        let info_a = BodyAccessInfo::of_world(a, &world);

        let field_b = world
            .entity::<true, &DensityFieldBox>(b)
            .unwrap()
            .as_any()
            .downcast_ref::<SphereDensityField<true>>()
            .unwrap();
        let info_b = BodyAccessInfo::of_world(b, &world);

        let mut cells = vec![];
        ShapeOverlapQuery {
            density_threshold: 0.5,
            voxelization_size_limit: 0.5,
            ..Default::default()
        }
        .query_field_pair([field_a, field_b], [&info_a, &info_b], &mut cells);
        assert_eq!(
            cells,
            vec![
                ShapeOverlapCell {
                    region: Aabb {
                        min: Vec3 {
                            x: -0.5,
                            y: -0.5,
                            z: 0.0
                        },
                        max: Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0
                        }
                    },
                    density: [
                        DensityRange { min: 1.0, max: 1.0 },
                        DensityRange { min: 1.0, max: 1.0 }
                    ],
                    normal: [
                        Vec3 {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0
                        },
                        Vec3 {
                            x: -0.70710677,
                            y: -0.70710677,
                            z: 0.0
                        }
                    ]
                },
                ShapeOverlapCell {
                    region: Aabb {
                        min: Vec3 {
                            x: -1.0,
                            y: -0.5,
                            z: 0.0
                        },
                        max: Vec3 {
                            x: -0.5,
                            y: 0.0,
                            z: 0.0
                        }
                    },
                    density: [
                        DensityRange { min: 1.0, max: 1.0 },
                        DensityRange { min: 0.0, max: 1.0 }
                    ],
                    normal: [
                        Vec3 {
                            x: 0.0,
                            y: 1.0,
                            z: 0.0
                        },
                        Vec3 {
                            x: -0.94868326,
                            y: -0.31622776,
                            z: 0.0
                        }
                    ]
                },
                ShapeOverlapCell {
                    region: Aabb {
                        min: Vec3 {
                            x: -0.5,
                            y: -1.0,
                            z: 0.0
                        },
                        max: Vec3 {
                            x: 0.0,
                            y: -0.5,
                            z: 0.0
                        }
                    },
                    density: [
                        DensityRange { min: 1.0, max: 1.0 },
                        DensityRange { min: 0.0, max: 1.0 }
                    ],
                    normal: [
                        Vec3 {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0
                        },
                        Vec3 {
                            x: -0.31622776,
                            y: -0.94868326,
                            z: 0.0
                        }
                    ]
                },
                ShapeOverlapCell {
                    region: Aabb {
                        min: Vec3 {
                            x: -1.0,
                            y: -1.0,
                            z: 0.0
                        },
                        max: Vec3 {
                            x: -0.5,
                            y: -0.5,
                            z: 0.0
                        }
                    },
                    density: [
                        DensityRange { min: 1.0, max: 1.0 },
                        DensityRange { min: 0.0, max: 1.0 }
                    ],
                    normal: [
                        Vec3 {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0
                        },
                        Vec3 {
                            x: -0.7071068,
                            y: -0.7071068,
                            z: 0.0
                        }
                    ]
                }
            ],
        );
    }
}
