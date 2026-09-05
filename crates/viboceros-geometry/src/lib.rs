//! Geometry primitives and numerical policy for Viboceros.
//!
//! Public constructors reject non-finite values. Operations that can overflow
//! or produce degenerate geometry return [`GeometryError`] rather than letting
//! invalid values enter the model.

mod bounds;
mod brep;
mod catenary;
mod circle_curve;
mod circular;
mod curve;
mod curve_edit;
mod curve_evaluate;
mod curve_fit;
mod curve_frame;
mod curve_join;
mod curve_parameter_map;
mod curve_rebuild;
mod curve_segment;
mod curve_through;
mod curve_trim;
mod curve_tween;
mod edge_surface;
mod ellipse;
mod error;
mod frame;
mod integration;
mod interpolation;
mod intersection;
mod line;
mod loft;
mod mesh;
mod morph;
mod nurbs;
mod nurbs2;
mod nurbs_surface;
mod parameter;
mod plane;
mod point;
mod point2;
mod point_cloud;
mod point_grid;
mod polycurve;
mod polyline;
mod section_basis;
mod spiral;
mod spline_collocation;
mod surface_curvature;
mod surface_pullback;
mod sweep;
mod tolerance;
mod transform;
mod vector;

pub use bounds::BoundingBox3;
pub use brep::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    RectangularSurfaceCorner, RectangularSurfaceCornerCut, SurfaceIso,
};
pub use catenary::{
    CatenaryConstruction, CatenaryCurve, CatenaryOutput, CatenarySolution,
    DEFAULT_CATENARY_POINT_COUNT, MAX_CATENARY_POINT_COUNT, MIN_POLYLINE_CATENARY_POINT_COUNT,
    MIN_SMOOTH_CATENARY_POINT_COUNT, try_catenary,
};
pub use circle_curve::Circle3;
pub use circular::CircularArc3;
pub use curve::{CurveRef, CurveSample, MAX_CURVE_DIVISION_POINTS};
pub use curve_edit::{Curve3, CurveClosure};
pub use curve_fit::{MAX_CURVE_FIT_CONTROL_POINTS, MAX_CURVE_FIT_DEGREE, try_fit_curve};
pub use curve_frame::FrameTransportOptions;
pub use curve_join::{CurveJoinOptions, CurveJoinStyle, JoinedCurve3, join_curves};
pub use curve_rebuild::{
    MAX_CURVE_REBUILD_DEGREE, MAX_CURVE_REBUILD_POINT_COUNT, try_rebuild_curve,
};
pub use curve_segment::CurveSegment3;
pub use curve_through::{
    CurveThroughConstruction, MAX_CURVE_THROUGH_DEGREE, sort_and_cull_points,
    try_curve_through_points,
};
pub use curve_tween::{
    CurveTweenMatchMethod, MAX_CURVE_TWEEN_COUNT, MAX_CURVE_TWEEN_OUTPUT_CONTROL_POINTS,
    MAX_CURVE_TWEEN_REFIT_CONTROL_POINTS, MAX_CURVE_TWEEN_SAMPLE_NUMBER,
    MIN_CURVE_TWEEN_SAMPLE_NUMBER, try_tween_nurbs_curves,
};
pub use ellipse::Ellipse3;
pub use error::GeometryError;
pub use frame::Frame3;
pub use interpolation::{
    CurveInterpolationOptions, CurveKnotSpacing, InterpolatedCurveClosure,
    MAX_CURVE_INTERPOLATION_POINTS,
};
pub use intersection::{
    BrepBrepIntersectionEvent, CurveBrepIntersection, CurveBrepIntersectionEvent, CurveBrepOverlap,
    CurveSurfaceIntersection, CurveSurfaceIntersectionEvent, CurveSurfaceOverlap,
    SurfaceBrepIntersectionEvent, SurfaceSurfaceIntersectionEvent, brep_brep_intersection_events,
    curve_brep_intersection_events, curve_surface_intersection_events, curve_surface_intersections,
    surface_brep_intersection_events, surface_surface_intersection_events,
    transformed_curve_brep_intersection_events,
};
pub use line::LineSegment;
pub use loft::{LoftStyle, MAX_LOFT_SECTION_CONTROLS, MAX_LOFT_SECTIONS, try_loft_nurbs_curves};
pub use mesh::{
    MAX_MESH_BOX_FACES, MAX_MESH_CONE_FACES, MAX_MESH_CYLINDER_FACES, MAX_MESH_ELLIPSOID_FACES,
    MAX_MESH_ICO_SPHERE_SUBDIVISIONS, MAX_MESH_PLANE_FACES, MAX_MESH_QUAD_SPHERE_SUBDIVISIONS,
    MAX_MESH_SPHERE_FACES, MAX_MESH_TORUS_FACES, MAX_MESH_TRUNCATED_CONE_FACES, MeshCapFaceStyle,
    MeshConeOptions, MeshCylinderOptions, MeshEdgeFilter, MeshEllipsoidOptions, MeshFace,
    MeshFaceExtraction, MeshHoleFill, MeshSubdivisionSphereOptions, MeshTopology, MeshTorusOptions,
    MeshTruncatedConeOptions, MeshUvSphereOptions, TriangleMesh,
};
pub use morph::{
    MAX_MORPH_CURVE_CONTROL_POINTS, MAX_MORPH_SURFACE_AXIS_CONTROLS, MAX_MORPH_SURFACE_SAMPLES,
    PointMorph, SurfacePointMorph,
};
pub use nurbs::{
    ControlPointCurveClosure, CurveCurveIntersection, CurveCurveIntersectionEvent,
    CurveCurveOverlap, CurveExtensionBoundary, CurveExtensionSide, CurveExtensionStyle, NurbsCurve,
    WeightedPoint3,
};
pub use nurbs_surface::{NurbsSurface, SurfaceExtensionEdge, SurfaceJet2, SurfaceKnotDirection};
pub use nurbs2::{NurbsCurve2, WeightedPoint2};
pub use parameter::ParameterSide;
pub use plane::{Plane, intersect_three_planes};
pub use point::Point3;
pub use point_cloud::PointCloud3;
pub use point_grid::{MAX_POINT_GRID_AXIS_COUNT, MAX_POINT_GRID_DEGREE};
pub use point2::Point2;
pub use polycurve::{MAX_POLYCURVE_SEGMENTS, PolyCurve3};
pub use polyline::{JoinedPolyline3, MAX_REGULAR_POLYGON_SIDES, Polyline3, join_polylines};
pub use spiral::{
    DEFAULT_SWEPT_SPIRAL_POINTS_PER_TURN, MAX_SPIRAL_CONTROL_POINTS,
    MIN_SWEPT_SPIRAL_POINTS_PER_TURN,
};
pub use surface_curvature::SurfaceCurvature;
pub use sweep::{Sweep1, SweepBlend, SweepFrameStyle, SweepSection};
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
