use thiserror::Error;

use crate::{MAX_SURFACE_WIRE_DENSITY, MAX_SURFACE_WIRES, MIN_SURFACE_WIRE_DENSITY, Real};

/// Failures produced while constructing or evaluating geometry.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum GeometryError {
    #[error("{context} contains a non-finite value")]
    NonFinite { context: &'static str },

    #[error("{context} is degenerate at the requested tolerance")]
    Degenerate { context: &'static str },

    #[error("tolerance components must be finite and strictly positive")]
    InvalidTolerance,

    #[error("at least one point is required")]
    EmptyPointSet,

    #[error("invalid polycurve: {context}")]
    InvalidPolyCurve { context: &'static str },

    #[error("point-cloud search radius must be finite and non-negative")]
    InvalidPointCloudSearchRadius,

    #[error("the linear system is singular at the requested tolerance")]
    SingularSystem,

    #[error("a NURBS degree must be at least one")]
    InvalidDegree,

    #[error(
        "a degree {degree} NURBS direction requires at least {required} control points, got {actual}"
    )]
    InsufficientControlPoints {
        degree: usize,
        required: usize,
        actual: usize,
    },

    #[error("a closed control-point curve requires at least three input controls, got {actual}")]
    InsufficientClosedControlPoints { actual: usize },

    #[error(
        "a NURBS direction with this degree and control-point count requires {expected} knots, got {actual}"
    )]
    InvalidKnotCount { expected: usize, actual: usize },

    #[error("a NURBS control net requires {expected} points, got {actual}")]
    InvalidControlNetSize { expected: usize, actual: usize },

    #[error("invalid NURBS control net: {context}")]
    InvalidControlNet { context: &'static str },

    #[error("invalid NURBS knot vector: {context}")]
    InvalidKnotVector { context: &'static str },

    #[error("NURBS knot multiplicity must be from 1 through {maximum}, got {actual}")]
    InvalidKnotMultiplicity { actual: usize, maximum: usize },

    #[error(
        "at a NURBS domain endpoint, knot multiplicity must be 1 or the direction degree {degree}, got {actual}"
    )]
    InvalidEndpointKnotMultiplicity { actual: usize, degree: usize },

    #[error("a NURBS curve split parameter must lie strictly inside its domain")]
    InvalidCurveSplitParameter,

    #[error("a NURBS curve trim interval must be finite, increasing, and inside its domain")]
    InvalidCurveTrimInterval,

    #[error("a NURBS curve extension interval must be finite, increasing, and extend its domain")]
    InvalidCurveExtensionInterval,

    #[error("a curve must be open before it can be extended")]
    CurveExtensionMustBeOpen,

    #[error("curve extension length must be finite and strictly positive")]
    InvalidCurveExtensionLength,

    #[error("the requested natural curve extension length could not be reached")]
    CurveExtensionLengthDidNotConverge,

    #[error("curve extension requires at least one boundary object")]
    EmptyCurveExtensionBoundaries,

    #[error("the requested curve end does not reach any boundary object")]
    CurveExtensionBoundaryNotFound,

    #[error("curve intersection refinement did not converge")]
    CurveIntersectionDidNotConverge,

    #[error("surface/surface intersection does not yet support {context}")]
    UnsupportedSurfaceSurfaceIntersection { context: &'static str },

    #[error("surface/B-rep intersection does not yet support {context}")]
    UnsupportedSurfaceBrepIntersection { context: &'static str },

    #[error("B-rep/B-rep intersection does not yet support {context}")]
    UnsupportedBrepBrepIntersection { context: &'static str },

    #[error("a NURBS surface extension interval must be finite, increasing, and extend its domain")]
    InvalidSurfaceExtensionInterval,

    #[error("surface extension length must be finite and strictly positive")]
    InvalidSurfaceExtensionLength,

    #[error(
        "surface shrink length {length} must be smaller than the available path length {available}"
    )]
    SurfaceShrinkLengthExceedsPath { length: Real, available: Real },

    #[error("a surface must be open in {direction} before it can be extended in that direction")]
    SurfaceExtensionDirectionMustBeOpen { direction: &'static str },

    #[error("a curve must be closed before its seam can be changed")]
    CurveSeamMustBeClosed,

    #[error(
        "surface seam relocation requires a closed homogeneous {direction} control-net direction"
    )]
    SurfaceSeamDirectionMustBeClosed { direction: &'static str },

    #[error("periodic NURBS conversion requires degree two or higher")]
    PeriodicNurbsDegreeTooLow,

    #[error("a curve must be closed before it can be made periodic")]
    PeriodicCurveMustBeClosed,

    #[error("a surface must be closed in the {direction} direction before it can be made periodic")]
    PeriodicSurfaceDirectionMustBeClosed { direction: &'static str },

    #[error(
        "smooth periodic degree-{degree} conversion requires at least {required} controls, got {actual}"
    )]
    InsufficientSmoothPeriodicControlPoints {
        degree: usize,
        required: usize,
        actual: usize,
    },

    #[error("the smooth periodic interpolation system could not be solved reliably")]
    PeriodicInterpolationSolveFailed,

    #[error("the NURBS degree-change interpolation system could not be solved reliably")]
    DegreeChangeSolveFailed,

    #[error("the NURBS knot-removal interpolation system could not be solved reliably")]
    KnotRemovalSolveFailed,

    #[error("maximum NURBS knot-removal kink angle must be finite and lie in [0, pi]")]
    InvalidKnotRemovalAngle,

    #[error("knot removal is not supported for a periodic NURBS {direction}")]
    PeriodicKnotRemovalUnsupported { direction: &'static str },

    #[error("there is no removable NURBS knot near parameter {parameter}")]
    NoRemovableKnot { parameter: f64 },

    #[error(
        "NURBS {direction} control-point index {index} is outside the control-point count {control_point_count}"
    )]
    ControlPointIndexOutOfRange {
        direction: &'static str,
        index: usize,
        control_point_count: usize,
    },

    #[error("no valid control-point insertion interval contains parameter {parameter}")]
    NoControlPointInsertionInterval { parameter: f64 },

    #[error("B-rep trim/isocurve intersection did not converge")]
    TrimIntersectionDidNotConverge,

    #[error("invalid B-rep topology: {context}")]
    InvalidBrepTopology { context: &'static str },

    #[error("a B-rep face subset must contain at least one face")]
    EmptyBrepFaceSubset,

    #[error("B-rep face index {face} is outside the face count {face_count}")]
    BrepFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("B-rep face index {face} appears more than once")]
    DuplicateBrepFaceIndex { face: usize },

    #[error("NURBS weight {index} must be finite and nonzero")]
    InvalidWeight { index: usize },

    #[error("NURBS parameter {parameter} is outside [{domain_start}, {domain_end}]")]
    ParameterOutOfDomain {
        parameter: f64,
        domain_start: f64,
        domain_end: f64,
    },

    #[error("the rational NURBS denominator vanished during evaluation")]
    ZeroWeightAtParameter,

    #[error("a polyline requires at least two vertices")]
    InsufficientPolylineVertices,

    #[error("polyline segment {segment} is degenerate at the requested tolerance")]
    DegeneratePolylineSegment { segment: usize },

    #[error("curve closure tolerance must be finite and non-negative")]
    InvalidCurveClosureTolerance,

    #[error("a regular polygon requires from 3 through {maximum} sides, got {actual}")]
    InvalidRegularPolygonSides { actual: usize, maximum: usize },

    #[error(
        "cannot join linear curves unambiguously where {endpoint_count} endpoints meet within tolerance"
    )]
    AmbiguousPolylineJoin { endpoint_count: usize },

    #[error("adaptive numerical integration did not converge at the requested tolerance")]
    NumericalIntegrationDidNotConverge,

    #[error("surface pullback did not converge at absolute model tolerance {tolerance}")]
    SurfacePullbackDidNotConverge { tolerance: Real },

    #[error("surface pullback would create more than {maximum} control points")]
    TooManySurfacePullbackControlPoints { maximum: usize },

    #[error("a curve division count must be from 1 through {maximum}, got {actual}")]
    InvalidCurveDivisionCount { actual: usize, maximum: usize },

    #[error("a curve division length must be finite and strictly positive")]
    InvalidCurveDivisionLength,

    #[error("curve division would create more than {maximum} points")]
    TooManyCurveDivisionPoints { maximum: usize },

    #[error("adaptive curve morph supports at most {maximum} control points")]
    TooManyMorphCurveControlPoints { maximum: usize },

    #[error("an interpolated curve requires at least two points, got {actual}")]
    InsufficientCurveInterpolationPoints { actual: usize },

    #[error("curve interpolation supports degree 1 or 3, got {actual}")]
    UnsupportedCurveInterpolationDegree { actual: usize },

    #[error("curve interpolation supports at most {maximum} points")]
    TooManyCurveInterpolationPoints { maximum: usize },

    #[error("curve interpolation point {second_index} coincides with its predecessor")]
    CoincidentCurveInterpolationPoints { second_index: usize },

    #[error("curve interpolation tangents require an open degree-three curve")]
    CurveInterpolationTangentsRequireOpenCubic,

    #[error("curve tween count must be from 1 through {maximum}, got {actual}")]
    InvalidCurveTweenCount { actual: usize, maximum: usize },

    #[error("curve tween sample count must be from {minimum} through {maximum}, got {actual}")]
    InvalidCurveTweenSampleCount {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },

    #[error("curve tween would create more than {maximum} output control points")]
    TooManyCurveTweenControlPoints { maximum: usize },

    #[error(
        "curve tween refit did not reach tolerance {tolerance} before the {maximum}-control-point limit; sampled deviation is {deviation}"
    )]
    CurveTweenRefitDidNotConverge {
        tolerance: Real,
        deviation: Real,
        maximum: usize,
    },

    #[error("curve fit degree must be from 1 through {maximum}, got {actual}")]
    InvalidCurveFitDegree { actual: usize, maximum: usize },

    #[error("curve fit tolerance must be finite and strictly positive")]
    InvalidCurveFitTolerance,

    #[error("curve fit angle tolerance must be finite and lie in [0, pi]")]
    InvalidCurveFitAngleTolerance,

    #[error("curve fit requires more than the maximum of {maximum} control points")]
    TooManyCurveFitControlPoints { maximum: usize },

    #[error(
        "curve fit did not reach tolerance {tolerance} before the {maximum}-control-point limit; sampled deviation is {deviation}"
    )]
    CurveFitDidNotConverge {
        tolerance: Real,
        deviation: Real,
        maximum: usize,
    },

    #[error("curve rebuild degree must be from 1 through {maximum}, got {actual}")]
    InvalidCurveRebuildDegree { actual: usize, maximum: usize },

    #[error("curve rebuild point count must be from {minimum} through {maximum}, got {actual}")]
    InvalidCurveRebuildPointCount {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },

    #[error("curve rebuild interpolation system could not be solved reliably")]
    CurveRebuildSolveFailed,

    #[error("curve-through degree must be from 1 through {maximum}, got {actual}")]
    InvalidCurveThroughDegree { actual: usize, maximum: usize },

    #[error(
        "a spiral requires a non-zero finite turn count and at least one non-zero finite radius"
    )]
    InvalidSpiralDimensions,

    #[error("a spiral supports at most {maximum} NURBS control points")]
    TooManySpiralControlPoints { maximum: usize },

    #[error("a swept spiral requires at least 5 interpolation points per turn, got {actual}")]
    InvalidSweptSpiralPointsPerTurn { actual: usize },

    #[error(
        "catenary point count must be from {minimum} through {maximum} for this output, got {actual}"
    )]
    InvalidCatenaryPointCount {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },

    #[error("a catenary parameter must be finite and strictly positive")]
    InvalidCatenaryParameter,

    #[error("a catenary length must be finite and longer than its endpoint chord")]
    InvalidCatenaryLength,

    #[error("the requested catenary apex is not beyond both endpoints along the axis")]
    InvalidCatenaryApex,

    #[error("the catenary through-point must lie strictly beyond the interior endpoint chord")]
    InvalidCatenaryThroughPoint,

    #[error("the catenary constraint solver did not converge to a finite curve")]
    CatenarySolveDidNotConverge,

    #[error("arc-length distance {distance} is outside [0, {length}]")]
    ArcLengthOutOfDomain { distance: f64, length: f64 },

    #[error("planar area requires a closed polyline")]
    OpenPolylineArea,

    #[error("the polyline does not define a non-degenerate planar region")]
    DegeneratePlanarRegion,

    #[error("the polyline is not planar at the requested tolerance")]
    NonPlanarPolyline,

    #[error("surface tessellation requires at least one sample per knot span")]
    InvalidTessellationResolution,

    #[error("mesh density must be finite and lie in [0, 1], got {0}")]
    InvalidMeshDensity(Real),

    #[error(
        "surface wire density must be from {MIN_SURFACE_WIRE_DENSITY} through {MAX_SURFACE_WIRE_DENSITY}, got {0}"
    )]
    InvalidSurfaceWireDensity(i32),

    #[error("surface wireframe would contain more than {MAX_SURFACE_WIRES} curves")]
    TooManySurfaceWires,

    #[error("B-rep face {face} could not be robustly tessellated inside its trim domain")]
    UnsupportedBrepTrimTessellation { face: usize },

    #[error("oriented volume requires a closed, consistently oriented B-rep")]
    OpenBrepVolume,

    #[error("a capped curve extrusion requires a closed, nondegenerate planar profile")]
    InvalidCappedExtrusionProfile,

    #[error("a planar face requires a closed, nondegenerate planar boundary")]
    InvalidPlanarFaceBoundary,

    #[error("a capped curve-along-curve extrusion requires an open path")]
    InvalidCappedExtrusionPath,

    #[error("a capped curve extrusion path must leave the profile plane")]
    CoplanarCappedExtrusion,

    #[error(
        "closed B-rep faces produced {boundary_edges} open and {orientation_conflicts} conflicting mesh edges"
    )]
    UnstitchedBrepTessellation {
        boundary_edges: usize,
        orientation_conflicts: usize,
    },

    #[error("non-affine B-rep morphing is not yet supported")]
    UnsupportedBrepMorph,

    #[error("a revolution sweep must be finite, non-zero, and no greater than one turn")]
    InvalidRevolutionSweep,

    #[error("a polygon mesh must contain at least one face")]
    EmptyMesh,

    #[error("a polygon mesh has too many vertices for 32-bit indices")]
    TooManyMeshVertices,

    #[error("a polygon mesh contains too many faces")]
    TooManyMeshFaces,

    #[error("mesh-plane face counts must be positive, got {x_count} by {y_count}")]
    InvalidMeshPlaneFaceCount { x_count: usize, y_count: usize },

    #[error("mesh-plane intervals must be strictly increasing")]
    InvalidMeshPlaneInterval,

    #[error("mesh-box face counts must be positive, got {x_count} by {y_count} by {z_count}")]
    InvalidMeshBoxFaceCount {
        x_count: usize,
        y_count: usize,
        z_count: usize,
    },

    #[error("mesh-box intervals must be strictly increasing")]
    InvalidMeshBoxInterval,

    #[error(
        "mesh-cylinder face counts require at least one vertical face and three around faces, got {vertical_count} by {around_count}"
    )]
    InvalidMeshCylinderFaceCount {
        vertical_count: usize,
        around_count: usize,
    },

    #[error("mesh-cylinder radius must be positive and its height interval strictly increasing")]
    InvalidMeshCylinderDimensions,

    #[error(
        "mesh-cone face counts require at least one vertical face and three around faces, got {vertical_count} by {around_count}"
    )]
    InvalidMeshConeFaceCount {
        vertical_count: usize,
        around_count: usize,
    },

    #[error("mesh-cone radius must be positive and its height nonzero")]
    InvalidMeshConeDimensions,

    #[error(
        "mesh truncated-cone face counts require at least one vertical face and three around faces, got {vertical_count} by {around_count}"
    )]
    InvalidMeshTruncatedConeFaceCount {
        vertical_count: usize,
        around_count: usize,
    },

    #[error("mesh truncated-cone radii and height must be positive")]
    InvalidMeshTruncatedConeDimensions,

    #[error(
        "UV mesh-sphere face counts require at least two vertical faces and three around faces, got {vertical_count} by {around_count}"
    )]
    InvalidMeshSphereFaceCount {
        vertical_count: usize,
        around_count: usize,
    },

    #[error("mesh-sphere radius must be positive")]
    InvalidMeshSphereRadius,

    #[error(
        "mesh-ellipsoid face counts require at least two vertical faces and three around faces, got {vertical_count} by {around_count}"
    )]
    InvalidMeshEllipsoidFaceCount {
        vertical_count: usize,
        around_count: usize,
    },

    #[error("mesh-ellipsoid radii must be positive")]
    InvalidMeshEllipsoidRadii,

    #[error("mesh-sphere subdivision count {subdivisions} exceeds the style maximum {maximum}")]
    InvalidMeshSphereSubdivisionCount { subdivisions: usize, maximum: usize },

    #[error(
        "mesh-torus face counts require at least three vertical and three around faces, got {vertical_count} by {around_count}"
    )]
    InvalidMeshTorusFaceCount {
        vertical_count: usize,
        around_count: usize,
    },

    #[error("mesh-torus minor radius must be positive and smaller than its major radius")]
    InvalidMeshTorusRadii,

    #[error("a mesh face subset must contain at least one face")]
    EmptyMeshFaceSubset,

    #[error("mesh face index {face} is outside the face count {face_count}")]
    MeshFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("mesh face index {face} appears more than once")]
    DuplicateMeshFaceIndex { face: usize },

    #[error(
        "a mesh face-angle interval must be finite and satisfy 0 <= greater than < less than <= pi"
    )]
    InvalidMeshFaceAngleInterval,

    #[error("a mesh break angle must be finite and lie in [0, pi]")]
    InvalidMeshBreakAngle,

    #[error("a mesh weld angle tolerance must be finite and lie in [0, pi]")]
    InvalidMeshWeldAngle,

    #[error("a mesh unweld angle tolerance must be finite and lie in [0, pi]")]
    InvalidMeshUnweldAngle,

    #[error("mesh topology edge index {edge} is outside the edge count {edge_count}")]
    MeshTopologyEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("mesh topology vertex index {vertex} is outside the vertex count {vertex_count}")]
    MeshTopologyVertexIndexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("triangle {triangle} references missing vertex index {vertex}")]
    InvalidTriangleIndex { triangle: usize, vertex: u32 },

    #[error("triangle index {triangle} is outside the mesh")]
    TriangleIndexOutOfRange { triangle: usize },

    #[error("triangle {triangle} is degenerate at the requested tolerance")]
    DegenerateTriangle { triangle: usize },

    #[error("quad {face} references missing vertex index {vertex}")]
    InvalidQuadIndex { face: usize, vertex: u32 },

    #[error("quad {face} is degenerate at the requested tolerance")]
    DegenerateQuad { face: usize },

    #[error("mesh face winding constraints describe a non-orientable surface")]
    NonOrientableMesh,

    #[error("a non-manifold edge must be shared by at least three faces, got {0}")]
    InvalidNonManifoldMinimumFaceCount(usize),
}
