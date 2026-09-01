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

    #[error("a regular polygon requires from 3 through {maximum} sides, got {actual}")]
    InvalidRegularPolygonSides { actual: usize, maximum: usize },

    #[error(
        "cannot join linear curves unambiguously where {endpoint_count} endpoints meet within tolerance"
    )]
    AmbiguousPolylineJoin { endpoint_count: usize },

    #[error("surface tessellation requires at least one sample per knot span")]
    InvalidTessellationResolution,

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
}
