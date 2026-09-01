//! Geometry primitives and numerical policy for Viboceros.
//!
//! Public constructors reject non-finite values. Operations that can overflow
//! or produce degenerate geometry return [`GeometryError`] rather than letting
//! invalid values enter the model.

mod bounds;
mod error;
mod line;
mod nurbs;
mod plane;
mod point;
mod tolerance;
mod vector;

pub use bounds::BoundingBox3;
pub use error::GeometryError;
pub use line::LineSegment;
pub use nurbs::{NurbsCurve, WeightedPoint3};
pub use plane::{Plane, intersect_three_planes};
pub use point::Point3;
pub use tolerance::Tolerance;
pub use vector::{UnitVector3, Vector3};

/// Scalar type used throughout the geometry kernel.
pub type Real = f64;

#[inline]
pub(crate) fn require_finite(
    values: impl IntoIterator<Item = Real>,
    context: &'static str,
) -> Result<(), GeometryError> {
    if values.into_iter().all(Real::is_finite) {
        Ok(())
    } else {
        Err(GeometryError::NonFinite { context })
    }
}
