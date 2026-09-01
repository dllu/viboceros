//! Versioned geometry-probe protocol used to compare Viboceros with Rhino.

use std::collections::BTreeSet;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use viboceros_geometry::{
    Circle3, CircularArc3, GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3, Polyline3,
    Tolerance, UnitVector3, WeightedPoint3,
};

pub const PROTOCOL_VERSION: u32 = 1;
const MAX_ITERATIONS: u32 = 1_000_000;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProbeRequest {
    pub protocol_version: u32,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    #[serde(default)]
    pub tolerance: ToleranceSpec,
    pub operations: Vec<Operation>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct ToleranceSpec {
    pub absolute: f64,
    pub relative: f64,
    pub angular: f64,
}

impl Default for ToleranceSpec {
    fn default() -> Self {
        Self {
            absolute: Tolerance::DEFAULT.absolute(),
            relative: Tolerance::DEFAULT.relative(),
            angular: Tolerance::DEFAULT.angular(),
        }
    }
}

impl ToleranceSpec {
    fn geometry(self) -> Result<Tolerance, GeometryError> {
        Tolerance::try_new(self.absolute, self.relative, self.angular)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    PointDistance {
        id: String,
        a: [f64; 3],
        b: [f64; 3],
    },
    LinePoint {
        id: String,
        start: [f64; 3],
        end: [f64; 3],
        parameter: f64,
    },
    CirclePoint {
        id: String,
        center: [f64; 3],
        radius: f64,
        x_axis: [f64; 3],
        normal: [f64; 3],
        angle_radians: f64,
    },
    ArcThreePoint {
        id: String,
        start: [f64; 3],
        through: [f64; 3],
        end: [f64; 3],
        normalized_parameter: f64,
    },
    PolylineLength {
        id: String,
        vertices: Vec<[f64; 3]>,
    },
    NurbsCurveEvaluate {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        parameter: f64,
    },
    NurbsSurfaceEvaluate {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        u: f64,
        v: f64,
    },
}

impl Operation {
    pub fn id(&self) -> &str {
        match self {
            Self::PointDistance { id, .. }
            | Self::LinePoint { id, .. }
            | Self::CirclePoint { id, .. }
            | Self::ArcThreePoint { id, .. }
            | Self::PolylineLength { id, .. }
            | Self::NurbsCurveEvaluate { id, .. }
            | Self::NurbsSurfaceEvaluate { id, .. } => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct ControlPoint {
    pub point: [f64; 3],
    #[serde(default = "unit_weight")]
    pub weight: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ProbeResponse {
    pub protocol_version: u32,
    pub engine: String,
    pub engine_version: String,
    pub iterations: u32,
    pub results: Vec<OperationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct OperationResult {
    pub id: String,
    pub value: Value,
    pub elapsed_ns: u64,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error("unsupported oracle protocol version {actual}; expected {expected}")]
    ProtocolVersion { actual: u32, expected: u32 },

    #[error("oracle iterations must be from 1 through {MAX_ITERATIONS}, got {0}")]
    InvalidIterations(u32),

    #[error("oracle operation id '{0}' is empty or duplicated")]
    InvalidOperationId(String),

    #[error("oracle timing exceeded the 64-bit nanosecond range")]
    TimingOverflow,
}

pub fn run_request(request: &ProbeRequest) -> Result<ProbeResponse, ProbeError> {
    validate_request(request)?;
    let tolerance = request.tolerance.geometry()?;
    let results = request
        .operations
        .iter()
        .map(|operation| execute(operation, request.iterations, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProbeResponse {
        protocol_version: PROTOCOL_VERSION,
        engine: "viboceros".to_owned(),
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        iterations: request.iterations,
        results,
        error: None,
    })
}

pub fn run_files(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<(), ProbeError> {
    let request: ProbeRequest = serde_json::from_slice(&fs::read(input)?)?;
    let response = match run_request(&request) {
        Ok(response) => response,
        Err(error) => ProbeResponse {
            protocol_version: PROTOCOL_VERSION,
            engine: "viboceros".to_owned(),
            engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            iterations: request.iterations,
            results: Vec::new(),
            error: Some(error.to_string()),
        },
    };
    let bytes = serde_json::to_vec_pretty(&response)?;
    fs::write(output, bytes)?;
    Ok(())
}

fn validate_request(request: &ProbeRequest) -> Result<(), ProbeError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(ProbeError::ProtocolVersion {
            actual: request.protocol_version,
            expected: PROTOCOL_VERSION,
        });
    }
    if !(1..=MAX_ITERATIONS).contains(&request.iterations) {
        return Err(ProbeError::InvalidIterations(request.iterations));
    }
    let mut ids = BTreeSet::new();
    for operation in &request.operations {
        let id = operation.id();
        if id.trim().is_empty() || !ids.insert(id) {
            return Err(ProbeError::InvalidOperationId(id.to_owned()));
        }
    }
    request.tolerance.geometry()?;
    Ok(())
}

fn execute(
    operation: &Operation,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<OperationResult, ProbeError> {
    let (value, elapsed_ns) = match operation {
        Operation::PointDistance { a, b, .. } => {
            let a = point(*a)?;
            let b = point(*b)?;
            let (distance, elapsed) = measure(iterations, || a.distance_to(black_box(b)))?;
            (json!(distance), elapsed)
        }
        Operation::LinePoint {
            start,
            end,
            parameter,
            ..
        } => {
            let line = LineSegment::try_new(point(*start)?, point(*end)?, tolerance)?;
            let (point, elapsed) = measure(iterations, || line.point_at(black_box(*parameter)))?;
            (json!(point.to_array()), elapsed)
        }
        Operation::CirclePoint {
            center,
            radius,
            x_axis,
            normal,
            angle_radians,
            ..
        } => {
            let center = point(*center)?;
            let x_axis = unit(*x_axis, tolerance)?;
            let normal = unit(*normal, tolerance)?;
            let point_on_circle = center.translated(x_axis.as_vector().scaled(*radius)?)?;
            let circle =
                Circle3::try_from_center_point(center, point_on_circle, normal, tolerance)?;
            let (point, elapsed) = measure(iterations, || {
                circle.point_at_angle(black_box(*angle_radians))
            })?;
            (json!(point.to_array()), elapsed)
        }
        Operation::ArcThreePoint {
            start,
            through,
            end,
            normalized_parameter,
            ..
        } => {
            let arc = CircularArc3::try_from_three_points(
                point(*start)?,
                point(*through)?,
                point(*end)?,
                tolerance,
            )?;
            let (point, elapsed) = measure(iterations, || {
                arc.point_at(black_box(*normalized_parameter))
            })?;
            (
                json!({
                    "center": arc.center().to_array(),
                    "point": point.to_array(),
                    "radius": arc.radius(),
                    "sweep_radians": arc.sweep_radians(),
                }),
                elapsed,
            )
        }
        Operation::PolylineLength { vertices, .. } => {
            let polyline = Polyline3::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                tolerance,
            )?;
            let (length, elapsed) = measure(iterations, || black_box(&polyline).length())?;
            (json!(length), elapsed)
        }
        Operation::NurbsCurveEvaluate {
            degree,
            control_points,
            knots,
            parameter,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let ((point, derivative), elapsed) = measure(iterations, || {
                curve.evaluate_with_derivative(black_box(*parameter))
            })?;
            (
                json!({
                    "derivative": derivative.to_array(),
                    "point": point.to_array(),
                }),
                elapsed,
            )
        }
        Operation::NurbsSurfaceEvaluate {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            u,
            v,
            ..
        } => {
            let surface = NurbsSurface::try_new_rational(
                *degree_u,
                *degree_v,
                *control_point_count_u,
                *control_point_count_v,
                weighted_points(control_points)?,
                knots_u.clone(),
                knots_v.clone(),
            )?;
            let ((point, derivative_u, derivative_v), elapsed) = measure(iterations, || {
                surface.evaluate_with_derivatives(black_box(*u), black_box(*v))
            })?;
            let normal = surface.normal_at(*u, *v, tolerance)?;
            (
                json!({
                    "derivative_u": derivative_u.to_array(),
                    "derivative_v": derivative_v.to_array(),
                    "normal": normal.as_vector().to_array(),
                    "point": point.to_array(),
                }),
                elapsed,
            )
        }
    };
    Ok(OperationResult {
        id: operation.id().to_owned(),
        value,
        elapsed_ns,
    })
}

fn point(coordinates: [f64; 3]) -> Result<Point3, GeometryError> {
    Point3::try_from(coordinates)
}

fn unit(coordinates: [f64; 3], tolerance: Tolerance) -> Result<UnitVector3, GeometryError> {
    UnitVector3::try_new(coordinates[0], coordinates[1], coordinates[2], tolerance)
}

fn weighted_points(points: &[ControlPoint]) -> Result<Vec<WeightedPoint3>, GeometryError> {
    points
        .iter()
        .map(|control| WeightedPoint3::try_new(point(control.point)?, control.weight))
        .collect()
}

fn measure<T>(
    iterations: u32,
    mut operation: impl FnMut() -> Result<T, GeometryError>,
) -> Result<(T, u64), ProbeError> {
    // Keep one-time dispatch and lazy initialization out of the timed loop.
    let mut value = black_box(operation()?);
    let started = Instant::now();
    for _ in 0..iterations {
        value = black_box(operation()?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

const fn default_iterations() -> u32 {
    1
}

const fn unit_weight() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operations: Vec<Operation>) -> ProbeRequest {
        ProbeRequest {
            protocol_version: PROTOCOL_VERSION,
            iterations: 2,
            tolerance: ToleranceSpec::default(),
            operations,
        }
    }

    #[test]
    fn runs_a_versioned_batch_and_preserves_operation_ids() {
        let response = run_request(&request(vec![
            Operation::PointDistance {
                id: "distance".to_owned(),
                a: [0.0, 0.0, 0.0],
                b: [3.0, 4.0, 0.0],
            },
            Operation::LinePoint {
                id: "line".to_owned(),
                start: [0.0, 0.0, 0.0],
                end: [4.0, 2.0, 0.0],
                parameter: 0.25,
            },
        ]))
        .unwrap();
        assert_eq!(response.engine, "viboceros");
        assert_eq!(response.results[0].id, "distance");
        assert_eq!(response.results[0].value, json!(5.0));
        assert_eq!(response.results[1].value, json!([1.0, 0.5, 0.0]));
    }

    #[test]
    fn rejects_protocol_iteration_and_id_errors() {
        let mut invalid = request(Vec::new());
        invalid.protocol_version = 2;
        assert!(matches!(
            run_request(&invalid),
            Err(ProbeError::ProtocolVersion { .. })
        ));
        invalid.protocol_version = PROTOCOL_VERSION;
        invalid.iterations = 0;
        assert!(matches!(
            run_request(&invalid),
            Err(ProbeError::InvalidIterations(0))
        ));
        invalid.iterations = 1;
        invalid.operations = vec![
            Operation::PointDistance {
                id: "same".to_owned(),
                a: [0.0; 3],
                b: [1.0; 3],
            },
            Operation::PointDistance {
                id: "same".to_owned(),
                a: [0.0; 3],
                b: [1.0; 3],
            },
        ];
        assert!(matches!(
            run_request(&invalid),
            Err(ProbeError::InvalidOperationId(id)) if id == "same"
        ));
    }

    #[test]
    fn evaluates_rational_curve_and_surface_derivatives() {
        let curve_controls = vec![
            ControlPoint {
                point: [1.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [1.0, 1.0, 0.0],
                weight: std::f64::consts::FRAC_1_SQRT_2,
            },
            ControlPoint {
                point: [0.0, 1.0, 0.0],
                weight: 1.0,
            },
        ];
        let surface_controls = vec![
            ControlPoint {
                point: [0.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [2.0, 0.0, 0.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [0.0, 3.0, 1.0],
                weight: 1.0,
            },
            ControlPoint {
                point: [2.0, 3.0, 1.0],
                weight: 1.0,
            },
        ];
        let response = run_request(&request(vec![
            Operation::NurbsCurveEvaluate {
                id: "curve".to_owned(),
                degree: 2,
                control_points: curve_controls,
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                parameter: 0.5,
            },
            Operation::NurbsSurfaceEvaluate {
                id: "surface".to_owned(),
                degree_u: 1,
                degree_v: 1,
                control_point_count_u: 2,
                control_point_count_v: 2,
                control_points: surface_controls,
                knots_u: vec![0.0, 0.0, 1.0, 1.0],
                knots_v: vec![0.0, 0.0, 1.0, 1.0],
                u: 0.25,
                v: 0.75,
            },
        ]))
        .unwrap();
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].id, "curve");
        assert_eq!(response.results[1].id, "surface");
    }
}
