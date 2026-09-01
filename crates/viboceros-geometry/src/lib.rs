//! Geometry primitives and numerical policy for Viboceros.
//!
//! Public constructors reject non-finite values. Operations that can overflow
//! or produce degenerate geometry return [`GeometryError`] rather than letting
//! invalid values enter the model.

mod bounds;
mod circular;
mod curve;
mod ellipse;
mod error;
mod integration;
mod line;
mod mesh;
mod nurbs;
mod nurbs_surface;
mod plane;
mod point;
mod point_cloud;
mod polyline;
mod tolerance;
mod transform;
mod vector;

pub use bounds::BoundingBox3;
pub use circular::{Circle3, CircularArc3};
pub use curve::{CurveRef, MAX_CURVE_DIVISION_POINTS};
pub use ellipse::Ellipse3;
pub use error::GeometryError;
pub use line::LineSegment;
pub use mesh::{MeshFaceExtraction, MeshTopology, TriangleMesh};
pub use nurbs::{NurbsCurve, WeightedPoint3};
pub use nurbs_surface::NurbsSurface;
pub use plane::{Plane, intersect_three_planes};
pub use point::Point3;
pub use point_cloud::PointCloud3;
pub use polyline::{
    JoinedPolyline3, MAX_REGULAR_POLYGON_SIDES, Polyline3, PolylineClosure, join_polylines,
};
pub use tolerance::Tolerance;
pub use transform::AffineTransform3;
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
