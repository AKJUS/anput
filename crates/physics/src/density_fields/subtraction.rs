use crate::{
    Scalar,
    components::BodyAccessInfo,
    density_fields::{DensityField, DensityFieldBox, DensityRange},
};
use vek::{Aabb, Vec3};

pub struct SubtractionDensityField {
    pub fields: Vec<DensityFieldBox>,
}

impl DensityField for SubtractionDensityField {
    fn aabb(&self, info: &BodyAccessInfo) -> Aabb<Scalar> {
        self.fields
            .iter()
            .map(|field| field.aabb(info))
            .reduce(|accum, aabb| accum.union(aabb))
            .unwrap_or_default()
    }

    fn density_at_point(&self, point: Vec3<Scalar>, info: &BodyAccessInfo) -> Scalar {
        self.fields
            .iter()
            .map(|field| field.density_at_point(point, info))
            .reduce(|accum, normal| accum - normal)
            .unwrap_or_default()
    }

    fn density_at_region(&self, region: Aabb<Scalar>, info: &BodyAccessInfo) -> DensityRange {
        self.fields
            .iter()
            .map(|field| field.density_at_region(region, info))
            .reduce(|accum, range| accum - range)
            .unwrap_or_default()
    }

    fn normal_at_point(
        &self,
        point: Vec3<Scalar>,
        resolution: Vec3<Scalar>,
        info: &BodyAccessInfo,
    ) -> Vec3<Scalar> {
        self.fields
            .iter()
            .map(|field| field.normal_at_point(point, resolution, info))
            .reduce(|accum, normal| accum - normal)
            .and_then(|normal| normal.try_normalized())
            .unwrap_or_default()
    }
}
