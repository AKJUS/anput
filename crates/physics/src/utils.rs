use crate::Scalar;
use vek::{Quaternion, Vec3};

pub fn quat_from_axis_angle(axis: Vec3<Scalar>, angle: Scalar) -> Quaternion<Scalar> {
    let half_angle = angle * 0.5;
    let (sin_half, cos_half) = half_angle.sin_cos();
    let axis = axis.normalized();

    Quaternion {
        x: axis.x * sin_half,
        y: axis.y * sin_half,
        z: axis.z * sin_half,
        w: cos_half,
    }
}
