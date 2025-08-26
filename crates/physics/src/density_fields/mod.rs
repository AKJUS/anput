pub mod aabb;
pub mod addition;
pub mod cube;
pub mod multiplication;
pub mod sphere;
pub mod subtraction;

use crate::{Scalar, components::BodyAccessInfo};
use std::{
    any::Any,
    ops::{Add, AddAssign, Deref, DerefMut, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
};
use vek::{Aabb, Vec3};

pub struct DensityFieldBox(Box<dyn DensityField>);

impl DensityFieldBox {
    pub fn new(field: impl DensityField + 'static) -> Self {
        Self(Box::new(field))
    }

    pub fn as_any(&self) -> &dyn Any {
        &*self.0
    }

    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut *self.0
    }
}

impl Deref for DensityFieldBox {
    type Target = dyn DensityField;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl DerefMut for DensityFieldBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}

pub trait DensityField: Send + Sync + Any {
    /// Returns the AABB that contains the density field.
    #[allow(unused_variables)]
    fn aabb(&self, info: &BodyAccessInfo) -> Aabb<Scalar>;

    /// Returns the precise density at the given point.
    /// Reported densities are useful for narrow area queries.
    ///
    /// Value 0.0 means no shape occupancy, 1.0 means full occupancy and values
    /// in between are considered density gradient.
    /// Shape might report scaled densities, e.g. reporting 0.5 on entire shape.
    ///
    /// Scaled densities are useful to represent soft shapes like fluid or fog.
    /// This allows for collision queries to achieve partial penetration based
    /// on density threshold, so if we have an object that has hard body core
    /// and soft outline, another object might partially penetrate that object
    /// outline and still not report collision.
    fn density_at_point(&self, point: Vec3<Scalar>, info: &BodyAccessInfo) -> Scalar;

    /// Returns the approximate minimum and maximum density at the given region.
    /// Reported densities are useful for broad area queries.
    ///
    /// This is useful for queries that require density at a larger region,
    /// for example if some collision queries do quad/oct tree subdivision
    /// use min-max to tell if region should be subdivided further or not.
    ///
    /// The default implementation samples densities at the center and corners
    /// using `density_at_point` and then reduces the results to find the
    /// minimum and maximum density. You should implement this method if you
    /// want to provide a more efficient or more precise way to compute the
    /// density range at a specific region.
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

    /// Returns normalized "surface" normal at the given point.
    ///
    /// It represents the direction of the density change gradient from more
    /// dense to less dense change at that point.
    /// Resolution parameter is used in case when only way to calculate normal
    /// is by multisampling, so resolution can be used for sampling offsets
    /// around queried point.
    ///
    /// The default implementation returns zero meaning no particular direction.
    /// You should limit returning zero only to cases where there is really no
    /// way to tell the density gradient at given point, like constant fields.
    #[allow(unused_variables)]
    fn normal_at_point(
        &self,
        point: Vec3<Scalar>,
        resolution: Vec3<Scalar>,
        info: &BodyAccessInfo,
    ) -> Vec3<Scalar> {
        Default::default()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DensityRange {
    pub min: Scalar,
    pub max: Scalar,
}

impl DensityRange {
    pub fn converged(density: Scalar) -> Self {
        Self {
            min: density,
            max: density,
        }
    }

    pub fn separation(&self) -> Scalar {
        (self.max - self.min).abs()
    }

    pub fn has_converged(&self) -> bool {
        self.separation() < Scalar::EPSILON
    }

    pub fn has_separation(&self) -> bool {
        !self.has_converged()
    }

    pub fn average(&self) -> Scalar {
        (self.min + self.max) * 0.5
    }

    pub fn min_max(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn clamp(&self) -> Self {
        Self {
            min: self.min.clamp(0.0, 1.0),
            max: self.max.clamp(0.0, 1.0),
        }
    }
}

impl Add for DensityRange {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            min: self.min + other.min,
            max: self.max + other.max,
        }
    }
}

impl AddAssign for DensityRange {
    fn add_assign(&mut self, other: Self) {
        self.min += other.min;
        self.max += other.max;
    }
}

impl Sub for DensityRange {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self {
            min: self.min - other.min,
            max: self.max - other.max,
        }
    }
}

impl SubAssign for DensityRange {
    fn sub_assign(&mut self, other: Self) {
        self.min -= other.min;
        self.max -= other.max;
    }
}

impl Mul for DensityRange {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        Self {
            min: self.min * other.min,
            max: self.max * other.max,
        }
    }
}

impl MulAssign for DensityRange {
    fn mul_assign(&mut self, other: Self) {
        self.min *= other.min;
        self.max *= other.max;
    }
}

impl Mul<Scalar> for DensityRange {
    type Output = Self;

    fn mul(self, scalar: Scalar) -> Self::Output {
        Self {
            min: self.min * scalar,
            max: self.max * scalar,
        }
    }
}

impl MulAssign<Scalar> for DensityRange {
    fn mul_assign(&mut self, scalar: Scalar) {
        self.min *= scalar;
        self.max *= scalar;
    }
}

impl Div for DensityRange {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        Self {
            min: self.min / other.min,
            max: self.max / other.max,
        }
    }
}

impl DivAssign for DensityRange {
    fn div_assign(&mut self, other: Self) {
        self.min /= other.min;
        self.max /= other.max;
    }
}

impl Div<Scalar> for DensityRange {
    type Output = Self;

    fn div(self, scalar: Scalar) -> Self::Output {
        Self {
            min: self.min / scalar,
            max: self.max / scalar,
        }
    }
}

impl DivAssign<Scalar> for DensityRange {
    fn div_assign(&mut self, scalar: Scalar) {
        self.min /= scalar;
        self.max /= scalar;
    }
}
