use thiserror::Error;

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

    #[error("invalid B-rep topology: {context}")]
    InvalidBrepTopology { context: &'static str },

    #[error("NURBS weight {index} must be finite and strictly positive")]
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

    #[error("B-rep face {face} requires trimmed-face clipping before it can be tessellated")]
    UnsupportedBrepTrimTessellation { face: usize },

    #[error("oriented volume requires a closed, consistently oriented B-rep")]
    OpenBrepVolume,

    #[error("B-rep face {face} requires trimmed-domain integration for mass properties")]
    UnsupportedBrepTrimMassProperties { face: usize },

    #[error("a capped curve extrusion requires a closed, nondegenerate planar profile")]
    InvalidCappedExtrusionProfile,

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

    #[error("a triangle mesh must contain at least one triangle")]
    EmptyMesh,

    #[error("a triangle mesh has too many vertices for 32-bit indices")]
    TooManyMeshVertices,

    #[error("triangle {triangle} references missing vertex index {vertex}")]
    InvalidTriangleIndex { triangle: usize, vertex: u32 },

    #[error("triangle index {triangle} is outside the mesh")]
    TriangleIndexOutOfRange { triangle: usize },

    #[error("triangle {triangle} is degenerate at the requested tolerance")]
    DegenerateTriangle { triangle: usize },

    #[error("mesh face winding constraints describe a non-orientable surface")]
    NonOrientableMesh,

    #[error("a non-manifold edge must be shared by at least three faces, got {0}")]
    InvalidNonManifoldMinimumFaceCount(usize),
}
