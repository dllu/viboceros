//! Geometry primitives and numerical policy for Viboceros.
//!
//! Public constructors reject non-finite values. Operations that can overflow
//! or produce degenerate geometry return [`GeometryError`] rather than letting
//! invalid values enter the model.

mod bounds;
mod brep;
mod circular;
mod curve;
mod ellipse;
mod error;
mod frame;
mod integration;
mod interpolation;
mod line;
mod mesh;
mod morph;
mod nurbs;
mod nurbs2;
mod nurbs_surface;
mod plane;
mod point;
mod point2;
mod point_cloud;
mod polyline;
mod tolerance;
mod transform;
mod vector;

pub use bounds::BoundingBox3;
pub use brep::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    SurfaceIso,
};
pub use circular::{Circle3, CircularArc3};
pub use curve::{CurveRef, CurveSample, MAX_CURVE_DIVISION_POINTS};
pub use ellipse::Ellipse3;
pub use error::GeometryError;
pub use frame::Frame3;
pub use interpolation::{
    CurveInterpolationOptions, CurveKnotSpacing, InterpolatedCurveClosure,
    MAX_CURVE_INTERPOLATION_POINTS,
};
pub use line::LineSegment;
pub use mesh::{MeshFaceExtraction, MeshTopology, TriangleMesh};
pub use morph::{PointMorph, SurfacePointMorph};
pub use nurbs::{ControlPointCurveClosure, NurbsCurve, WeightedPoint3};
pub use nurbs_surface::NurbsSurface;
pub use nurbs2::{NurbsCurve2, WeightedPoint2};
pub use plane::{Plane, intersect_three_planes};
pub use point::Point3;
pub use point_cloud::PointCloud3;
pub use point2::Point2;
pub use polyline::{
    JoinedPolyline3, MAX_REGULAR_POLYGON_SIDES, Polyline3, PolylineClosure, join_polylines,
};
pub use tolerance::Tolerance;
pub use transform::AffineTransform3;
pub use vector::{UnitVector3, Vector3};

/// Scalar type used throughout the geometry kernel.
pub type Real = f64;

/// OpenNURBS-compatible range for per-object surface wire density.
pub const MIN_SURFACE_WIRE_DENSITY: i32 = -1;
pub const MAX_SURFACE_WIRE_DENSITY: i32 = 99;
pub const DEFAULT_SURFACE_WIRE_DENSITY: i32 = 1;

/// Resource ceiling for wire parameters or exact wireframe curves returned by
/// one geometry operation.
pub const MAX_SURFACE_WIRES: usize = 1_000_000;

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
