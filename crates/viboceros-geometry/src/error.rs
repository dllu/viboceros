use thiserror::Error;

use crate::{MAX_SURFACE_WIRE_DENSITY, MAX_SURFACE_WIRES, MIN_SURFACE_WIRE_DENSITY};

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

    #[error("NURBS knot multiplicity must be from 1 through {maximum}, got {actual}")]
    InvalidKnotMultiplicity { actual: usize, maximum: usize },

    #[error("a NURBS curve split parameter must lie strictly inside its domain")]
    InvalidCurveSplitParameter,

    #[error("a NURBS curve trim interval must be finite, increasing, and inside its domain")]
    InvalidCurveTrimInterval,

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

    #[error(
        "surface wire density must be from {MIN_SURFACE_WIRE_DENSITY} through {MAX_SURFACE_WIRE_DENSITY}, got {0}"
    )]
    InvalidSurfaceWireDensity(i32),

    #[error("surface wireframe would contain more than {MAX_SURFACE_WIRES} curves")]
    TooManySurfaceWires,

    #[error("B-rep face {face} requires trimmed-face clipping before it can be tessellated")]
    UnsupportedBrepTrimTessellation { face: usize },

    #[error(
        "B-rep face {face} requires trimmed-domain integration before its area can be measured"
    )]
    UnsupportedBrepTrimArea { face: usize },

    #[error("oriented volume requires a closed, consistently oriented B-rep")]
    OpenBrepVolume,

    #[error("B-rep face {face} requires trimmed-domain integration for mass properties")]
    UnsupportedBrepTrimMassProperties { face: usize },

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
