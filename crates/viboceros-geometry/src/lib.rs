//! Geometry primitives and numerical policy for Viboceros.
//!
//! Public constructors reject non-finite values. Operations that can overflow
//! or produce degenerate geometry return [`GeometryError`] rather than letting
//! invalid values enter the model.

mod subd;
pub use subd::SubD;
mod area_mass_properties;
mod mass_integration;
pub use area_mass_properties::AreaMassProperties;
mod volume_mass_properties;
pub use volume_mass_properties::{SurfaceVolumeMoments, VolumeBoundary, VolumeMassProperties};
mod arc_deform;
mod bezier;
mod binary_accumulator;
mod bounds;
mod brep;
mod catenary;
mod circle_curve;
mod circle_deform;
pub use circle_deform::circle_deformable_minimum_points;
mod circle_fit;
mod circle_tangent;
mod tangent_arc;
pub use circle_tangent::{CircleTangentConstraint, CircleTangentSolution, try_circle_tangent};
pub use tangent_arc::try_tangent_arc;
mod radial_fit;
mod sphere_fit;
pub use circle_fit::MAX_CIRCLE_FIT_POINTS;
pub use sphere_fit::{MAX_SPHERE_FIT_POINTS, SphereFit};
mod circular;
mod curvature_comb;
mod curve;
pub use curvature_comb::{
    CurvatureComb, CurvatureHair, curvature_comb, curvature_comb_with_subdivision,
};
pub use curve::{ArcLengthCheckpoint, ArcLengthSampler};
mod curve_distance;
pub use curve_distance::{
    CurveDistanceExtrema, CurveDistanceWitness, LineDeviation, line_deviation,
    preserve_transverse_curve_ends,
};
mod curve_blend_pair;
mod curve_chamfer_pair;
mod curve_connect_pair;
mod curve_continuity;
mod curve_edit;
mod curve_edit_points;
pub use curve_edit_points::{CurveEditPoint, CurveEditPointInterpolation};
mod curve_evaluate;
mod curve_fillet_operations;
mod curve_fillet_pair;
pub use curve_fillet_operations::{
    CurveFilletContact, CurveFilletContactProducer, CurveFilletOperations,
    CurveFilletPairOperations,
};
mod curve_fit;
mod curve_frame;
mod curve_join;
mod curve_length_subcurve;
mod curve_match;
mod curve_offset;
mod curve_pair_support;
mod curve_parameter_map;
mod curve_rebuild;
mod curve_segment;
mod curve_shortness;
mod curve_through;
mod curve_trim;
mod curve_tween;
mod surface_curve_offset;
mod surface_geodesic;
mod surface_interpolation;
mod surface_short_path;
pub use surface_curve_offset::{
    SurfaceCurveExtension, SurfaceCurveOffset, SurfaceCurveOffsetGapStyle,
    SurfaceCurveOffsetOptions, SurfaceCurveOffsetPieces, SurfaceCurveOffsetThroughPoint,
    surface_curve_offset_distance_through_point, try_extend_curve_normally_on_surface,
    try_extend_curve_normally_on_surface_with_operation, try_extend_curve_on_surface,
    try_extend_curve_on_surface_with_seams, try_extend_curve_on_surface_with_solver,
    try_offset_curve_on_surface, try_offset_curve_on_surface_with_solver,
    try_offset_curves_on_surface, try_offset_curves_on_surface_with_options,
    try_offset_curves_on_surface_with_solver,
};
pub use surface_interpolation::{SurfaceInterpolatedCurve, try_interpolate_curve_on_surface};
pub use surface_short_path::{SurfaceShortPath, try_short_path_on_surface};
mod surface_rebuild;
pub use surface_geodesic::{PreparedSurfaceGeodesic, SurfaceGeodesicPoint};
pub use surface_rebuild::{
    NurbsInterpolationSystem, UniformInterpolationSystem, try_rebuild_nurbs_surface,
    try_rebuild_nurbs_surface_with_operations, try_rebuild_open_nurbs_surface_with_operations,
};
mod endpoint_interpolation;
mod surface_blend;
pub use endpoint_interpolation::{EndpointInterpolation, try_interpolate_endpoints};
mod surface_tween;
pub use surface_blend::{
    SurfaceBlendContinuity, SurfaceBlendDiagnostics, SurfaceBlendInterpolation,
    SurfaceBlendOperations, SurfaceBlendOptions, SurfaceBlendResult, try_blend_nurbs_surfaces,
    try_blend_nurbs_surfaces_with_operations, try_interpolate_surface_blend,
};
pub use surface_tween::{
    MAX_SURFACE_TWEEN_CONTROLS, MAX_SURFACE_TWEEN_COUNT, try_tween_nurbs_surfaces,
    try_tween_nurbs_surfaces_refitted, try_tween_nurbs_surfaces_refitted_with_operations,
    try_tween_nurbs_surfaces_sampled, try_tween_nurbs_surfaces_sampled_with_grid,
    try_tween_nurbs_surfaces_with_rebuild,
};
mod edge_surface;
pub use edge_surface::arrange_edge_surface_curves;
mod ellipse;
mod error;
mod exact_scalar;
mod interval;
pub use exact_scalar::{interpolate_scalar, remap_scalar, scaled_quotient};
mod finite_sum;
mod frame;
mod integration;
mod interpolation;
mod intersection;
mod line;
mod loft;
mod mesh;
mod tessellation;
pub use tessellation::{SurfaceTessellation, TessellationCorner, TessellationNormalSource};
mod twist;
pub use twist::TwistPointMorph;
mod bend;
pub use bend::BendPointMorph;
mod stretch;
pub use stretch::StretchPointMorph;
mod taper;
pub use taper::TaperPointMorph;
mod maelstrom;
pub use maelstrom::MaelstromPointMorph;
mod morph;
mod nurbs;
mod nurbs2;
mod nurbs_surface;
#[path = "../../../third_party/opennurbs_rust/box_topology.rs"]
mod opennurbs_box_topology;
#[path = "../../../third_party/opennurbs_rust/chord_adjust.rs"]
mod opennurbs_chord_adjust;
mod parameter;
pub use parameter::check_trim_interval as validate_curve_trim_interval;
mod plane;
mod point;
mod point2;
mod point_cloud;
mod point_grid;
pub use point_grid::{PointGridInterpolationSolution, PointGridInterpolationSystem};
mod point_projection;
mod polycurve;
mod polyline;
mod section_basis;
mod smoothing;
pub use smoothing::{SmoothingCoordinates, SmoothingOptions};
mod spiral;
mod spline_collocation;
mod surface_curvature;
mod surface_pullback;
pub use surface_pullback::PreparedSurfaceCurveBounds;
mod sweep;
mod tolerance;
mod transform;
mod units;
mod vector;
pub use finite_sum::FiniteSum;
pub use units::{LengthUnitSystem, UnitError};

pub use bezier::MAX_BEZIER_CONTROL_POINTS;
pub use bounds::BoundingBox3;
pub use brep::{
    Brep, BrepBooleanOperation, BrepBoundaryRestoration, BrepConvexBooleanPlan,
    BrepConvexIntersection, BrepConvexRegion, BrepCoplanarPartitionComponent,
    BrepDifferenceComponent, BrepEdge, BrepEdgeMergeScope, BrepFace, BrepHoleRemoval,
    BrepJoinComponent, BrepJoinReport, BrepLoop, BrepLoopType, BrepOpenSplitComponent,
    BrepPolyhedralBooleanComponent, BrepPolyhedralBooleanPlan, BrepPolyhedralBoundaryComponent,
    BrepPolyhedralRegion, BrepPolyhedralShell, BrepSetIntersection, BrepSolidOrientation,
    BrepSplitComponent, BrepSurfaceShrinkMode, BrepSurfaceSplitComponent, BrepTrim, BrepTrimType,
    BrepUnionComponent, BrepVertex, BrepWireCage, RectangularSurfaceCorner,
    RectangularSurfaceCornerCut, SurfaceIso, boolean_planar_breps, boolean_polyhedral_breps,
    boolean_projected_planar_breps, convex_brep_boundary_interactions,
    convex_brep_subtraction_interactions, intersect_convex_brep_sets, intersect_convex_breps,
    intersect_polyhedral_brep_sets, intersect_polyhedral_breps, join_breps, join_breps_with_report,
    polyhedral_brep_boundary_interactions, polyhedral_brep_subtraction_interactions,
    split_open_polyhedral_brep, split_open_polyhedral_brep_with_lineage, split_polyhedral_brep,
    split_polyhedral_brep_by_trimmed_sheets, split_polyhedral_brep_with_surfaces,
    subtract_convex_breps, subtract_polyhedral_breps, union_convex_breps, union_polyhedral_breps,
};
pub use catenary::{
    CatenaryConstruction, CatenaryCurve, CatenaryOutput, CatenarySolution,
    DEFAULT_CATENARY_POINT_COUNT, MAX_CATENARY_POINT_COUNT, MIN_POLYLINE_CATENARY_POINT_COUNT,
    MIN_SMOOTH_CATENARY_POINT_COUNT, try_catenary,
};
pub use circle_curve::Circle3;
pub use circular::CircularArc3;
pub use curve::{
    CurveDivisionPoint, CurveRef, CurveSample, MAX_CURVE_DIVISION_POINTS, try_control_polygon_plane,
};
pub use curve_blend_pair::{
    CurveBlendContinuity, CurveBlendOptions, try_blend_curve, try_blend_curve_with_interpolation,
};
pub use curve_chamfer_pair::{
    CurveChamferExtensionStyles, try_chamfer_curves_joined,
    try_chamfer_curves_joined_with_operations, try_chamfer_curves_joined_with_styles,
    try_chamfer_curves_parts, try_chamfer_curves_parts_with_operations,
    try_chamfer_curves_parts_with_styles,
};
pub use curve_connect_pair::{
    CurveArcExtensionStyle, CurveOtherExtensionStyle, try_connect_curves_joined,
    try_connect_curves_joined_with_arc_style, try_connect_curves_joined_with_operations,
    try_connect_curves_joined_with_styles, try_connect_curves_parts,
    try_connect_curves_parts_with_arc_style, try_connect_curves_parts_with_operations,
    try_connect_curves_parts_with_styles,
};
pub use curve_continuity::{CurveContinuityLevel, CurveContinuityReport, curve_end_continuity};
pub use curve_edit::{Curve3, CurveClosure};
pub use curve_fillet_pair::{
    CurveFilletExtensionStyles, try_fillet_curves_joined, try_fillet_curves_joined_with_operations,
    try_fillet_curves_joined_with_producers, try_fillet_curves_joined_with_styles,
    try_fillet_curves_parts, try_fillet_curves_parts_with_operations,
    try_fillet_curves_parts_with_producers, try_fillet_curves_parts_with_styles,
};
pub use curve_fit::{
    ConstrainedCurveFit, CurveFitConstraint, CurveFitInterpolation, CurveFitStation,
    FixedInterpolationSystem, MAX_CURVE_FIT_CONTROL_POINTS, MAX_CURVE_FIT_DEGREE,
    try_fit_cubic_curve_at_stations, try_fit_cubic_curve_at_stations_with_solver,
    try_fit_cubic_curve_with_constraints, try_fit_cubic_curve_with_constraints_with_solver,
    try_fit_curve, try_fit_curve_with_solver,
};
pub use curve_frame::{FrameQueryRestart, FrameTransportOptions};
pub use curve_join::{
    CurveJoinOptions, CurveJoinStyle, JoinedCurve3, join_curves, join_curves_with_endpoint_filter,
};
pub use curve_match::{
    CurveMatchOperations, CurveMatchPreserveEnd, try_average_match_curve_ends,
    try_average_match_curve_ends_with_operations, try_match_curve_end,
    try_match_curve_end_with_operations,
};
pub use curve_offset::{
    CurveOffsetCornerStyle, CurveOffsetIntersections, CurveOffsetOperations, CurveOffsetSmooth,
    CurveOffsetSplit,
};
pub use curve_rebuild::{
    CurveRebuildInterpolation, MAX_CURVE_REBUILD_DEGREE, MAX_CURVE_REBUILD_POINT_COUNT,
    try_interpolate_affine_greville_with_solver, try_rebuild_curve,
    try_rebuild_curve_with_interpolation,
};
pub use curve_segment::CurveSegment3;
pub use curve_through::{
    CurveThroughConstruction, MAX_CURVE_THROUGH_DEGREE, sort_and_cull_points,
    try_curve_through_points, try_curve_through_points_with_interpolation,
};
pub use curve_tween::{
    CurveTweenMatchMethod, MAX_CURVE_TWEEN_COUNT, MAX_CURVE_TWEEN_OUTPUT_CONTROL_POINTS,
    MAX_CURVE_TWEEN_REFIT_CONTROL_POINTS, MAX_CURVE_TWEEN_SAMPLE_NUMBER,
    MIN_CURVE_TWEEN_SAMPLE_NUMBER, try_tween_nurbs_curves, try_tween_nurbs_curves_with_sampling,
};
pub use ellipse::Ellipse3;
pub use error::GeometryError;
pub use frame::Frame3;
pub use interpolation::{
    CubicInterpolationSystem, CurveInterpolationOptions, CurveKnotSpacing,
    InterpolatedCurveClosure, MAX_CURVE_INTERPOLATION_POINTS,
};
pub use intersection::{
    BrepBrepIntersectionEvent, CurveBrepIntersection, CurveBrepIntersectionEvent, CurveBrepOverlap,
    CurveSurfaceIntersection, CurveSurfaceIntersectionEvent, CurveSurfaceIntersectionParameters,
    CurveSurfaceOverlap, SurfaceBrepIntersectionEvent, SurfaceIntersectionInput,
    SurfaceIntersectionOutput, SurfaceSurfaceIntersectionEvent, SurfaceSurfaceIntersectionProposal,
    brep_brep_intersection_events, brep_brep_intersection_events_with_operation,
    clip_surface_curve_to_face_with_operation, curve_brep_intersection_events,
    curve_brep_intersection_events_with_operations, curve_surface_intersection_events,
    curve_surface_intersection_events_with_operation, curve_surface_intersections,
    surface_brep_intersection_events, surface_brep_intersection_events_with_operation,
    surface_face_intersection_events_with_operations, surface_surface_intersection_events,
    surface_surface_intersection_events_with_operation, transformed_curve_brep_intersection_events,
};
pub use line::LineSegment;
pub use loft::{
    LoftInterpolation, LoftStyle, MAX_LOFT_SECTION_CONTROLS, MAX_LOFT_SECTIONS,
    prepare_loft_sections_with_operations, try_loft_nurbs_curves,
    try_loft_nurbs_curves_with_operations, try_loft_nurbs_curves_with_preparation,
};
pub use mesh::{
    MAX_MESH_BOX_FACES, MAX_MESH_CONE_FACES, MAX_MESH_CYLINDER_FACES, MAX_MESH_ELLIPSOID_FACES,
    MAX_MESH_ICO_SPHERE_SUBDIVISIONS, MAX_MESH_PLANE_FACES, MAX_MESH_QUAD_SPHERE_SUBDIVISIONS,
    MAX_MESH_SPHERE_FACES, MAX_MESH_TORUS_FACES, MAX_MESH_TRUNCATED_CONE_FACES, MeshAlignSelection,
    MeshCapFaceStyle, MeshConeOptions, MeshCylinderOptions, MeshDiagnosticEdge, MeshEdgeFilter,
    MeshEdgeMatchOptions, MeshEdgeMatchResult, MeshEdgePick, MeshEllipsoidOptions,
    MeshExtrudeDirection, MeshExtrudeSelection, MeshFace, MeshFaceBoundaryGraph,
    MeshFaceCullResult, MeshFaceExtraction, MeshHoleFill, MeshJoinComponent, MeshJoinOptions,
    MeshNgon, MeshOffsetDirection, MeshPartBoundary, MeshSingleFaceComponents, MeshSolid,
    MeshSubdivisionSphereOptions, MeshTopology, MeshTorusOptions, MeshTruncatedConeOptions,
    MeshUvSphereOptions, NonPlanarQuadCriterion, QuadSplitMethod, SolidPointLocation, TriangleMesh,
    align_mesh_vertices, join_meshes, match_mesh_edges,
};
pub use mesh::{MAX_MESH_PATCH_POINTS, MeshPatchOptions};
pub use mesh::{MeshClosestLocation, MeshProjectionIndex};
pub use morph::{
    CurveFlowLocalRadii, CurveFlowOptions, CurveFlowPointMorph, CurveFlowTargetSurface,
    CurvePreviewCage, MAX_MORPH_CURVE_CONTROL_POINTS, MAX_MORPH_SURFACE_AXIS_CONTROLS,
    MAX_MORPH_SURFACE_SAMPLES, MorphInterpolationSolver, MorphPreviewOperations, PointMorph,
    SurfaceFlowFrame, SurfaceFlowOptions, SurfaceFlowPointMorph, SurfacePointMorph,
    SurfacePreviewCage,
};
pub use nurbs::{
    ConicFoci, ControlPointCurveClosure, CurveCurveIntersection, CurveCurveIntersectionEvent,
    CurveCurveIntersectionParameters, CurveCurveOverlap, CurveExtensionBoundary,
    CurveExtensionOperations, CurveExtensionSide, CurveExtensionStyle, CurveSoftEdit, NurbsCurve,
    NurbsCurveParameterSampler, NurbsCurveSamplingSpan, WeightedPoint3, degree_change_knots,
    knot_group_removal_knots, knot_removal_knots,
};
pub use nurbs_surface::{
    NurbsSurface, SurfaceExtensionEdge, SurfaceJet2, SurfaceKnotDirection, SurfaceSoftEdit,
};
pub use nurbs2::{NurbsCurve2, WeightedPoint2};
pub use parameter::ParameterSide;
pub use plane::{Plane, intersect_three_planes};
pub use point::Point3;
pub use point_cloud::{PointCloud3, PointCloudChannels, PointCloudPlane, PointCloudProjection};
pub use point_grid::{MAX_POINT_GRID_AXIS_COUNT, MAX_POINT_GRID_DEGREE};
pub use point_projection::{MAX_LINE_FIT_POINTS, MAX_PLANE_FIT_POINTS, PointProjection3};
pub use point2::Point2;
pub use polycurve::{MAX_POLYCURVE_SEGMENTS, PolyCurve3};
pub use polyline::{JoinedPolyline3, MAX_REGULAR_POLYGON_SIDES, Polyline3, join_polylines};
pub use section_basis::{
    WeightScale as SectionWeightScale,
    prepare_with_operations as prepare_nurbs_sections_with_operations,
};
pub use spiral::{
    DEFAULT_SWEPT_SPIRAL_POINTS_PER_TURN, MAX_SPIRAL_CONTROL_POINTS,
    MIN_SWEPT_SPIRAL_POINTS_PER_TURN,
};
pub use surface_curvature::SurfaceCurvature;
pub use sweep::{Sweep1, SweepBlend, SweepFrameStyle, SweepSection, SweepSurfaceOperations};
pub use tolerance::Tolerance;
pub use transform::{AffineNormalTransform3, AffineTransform3};
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

mod curve_simplify;
#[cfg(test)]
mod grip_deletion_tests;
#[cfg(test)]
mod grip_weight_tests;
pub use curve_simplify::{CurveSimplification, simplify_curve, simplify_curve_with_split};

/// Fixed coordinate coincidence used for curve seams and junctions.
pub use nurbs::curve_points_coincident;
