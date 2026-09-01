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

    #[error("a NURBS curve degree must be at least one")]
    InvalidDegree,

    #[error(
        "a degree {degree} NURBS curve requires at least {required} control points, got {actual}"
    )]
    InsufficientControlPoints {
        degree: usize,
        required: usize,
        actual: usize,
    },

    #[error(
        "a NURBS curve with this degree and control-point count requires {expected} knots, got {actual}"
    )]
    InvalidKnotCount { expected: usize, actual: usize },

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
}
