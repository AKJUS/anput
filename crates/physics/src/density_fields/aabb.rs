use crate::{
    Scalar,
    components::BodyAccessInfo,
    density_fields::{DensityField, DensityRange},
};
use std::cmp::Ordering;
use vek::{Aabb, Vec3};

pub struct AabbDensityField {
    pub aabb: Aabb<Scalar>,
    pub density: Scalar,
}

impl DensityField for AabbDensityField {
    fn aabb(&self, _: &BodyAccessInfo) -> Aabb<Scalar> {
        self.aabb
    }

    fn density_at_point(&self, point: Vec3<Scalar>, _: &BodyAccessInfo) -> Scalar {
        if self.aabb.contains_point(point) {
            self.density
        } else {
            0.0
        }
    }

    fn density_at_region(&self, region: Aabb<Scalar>, _: &BodyAccessInfo) -> DensityRange {
        if self.aabb.contains_aabb(region) {
            DensityRange::converged(self.density)
        } else if self.aabb.collides_with_aabb(region) {
            DensityRange {
                min: 0.0,
                max: self.density,
            }
        } else {
            Default::default()
        }
    }

    fn normal_at_point(
        &self,
        point: Vec3<Scalar>,
        _: Vec3<Scalar>,
        _: &BodyAccessInfo,
    ) -> Vec3<Scalar> {
        let size = self.aabb.size().into_array();
        [
            (
                size[0],
                (point.x - self.aabb.min.x).abs(),
                Vec3::new(-1.0, 0.0, 0.0),
            ),
            (
                size[1],
                (point.y - self.aabb.min.y).abs(),
                Vec3::new(0.0, -1.0, 0.0),
            ),
            (
                size[2],
                (point.z - self.aabb.min.z).abs(),
                Vec3::new(0.0, 0.0, -1.0),
            ),
            (
                size[0],
                (self.aabb.max.x - point.x).abs(),
                Vec3::new(1.0, 0.0, 0.0),
            ),
            (
                size[1],
                (self.aabb.max.y - point.y).abs(),
                Vec3::new(0.0, 1.0, 0.0),
            ),
            (
                size[2],
                (self.aabb.max.z - point.z).abs(),
                Vec3::new(0.0, 0.0, 1.0),
            ),
        ]
        .into_iter()
        .filter(|(size, _, _)| *size > Scalar::EPSILON)
        .min_by(|(_, a, _), (_, b, _)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
        .map(|(_, _, normal)| normal)
        .unwrap_or_default()
    }
}
