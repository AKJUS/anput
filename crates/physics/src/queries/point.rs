use crate::{Scalar, components::BodyAccessInfo, density_fields::DensityField};
use vek::Vec3;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PointOverlapQuery {
    pub position: Vec3<Scalar>,
    pub resolution: Vec3<Scalar>,
    pub density_threshold: Scalar,
}

impl PointOverlapQuery {
    pub fn query_field(
        &self,
        field: &dyn DensityField,
        info: &BodyAccessInfo,
    ) -> Option<PointOverlapResult> {
        let density = field.density_at_point(self.position, info);
        if density >= self.density_threshold {
            let normal = field.normal_at_point(self.position, self.resolution, info);
            Some(PointOverlapResult {
                point: self.position,
                density,
                normal,
            })
        } else {
            None
        }
    }
}

pub struct PointOverlapResult {
    pub point: Vec3<Scalar>,
    pub density: Scalar,
    pub normal: Vec3<Scalar>,
}
