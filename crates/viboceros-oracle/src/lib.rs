//! Versioned compatibility-probe protocol used to compare Viboceros with Rhino.

use std::collections::BTreeSet;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, ObjectAttributes, ObjectId, SelectionMode,
};
use viboceros_geometry::{
    Circle3, CircularArc3, CurveRef, Ellipse3, GeometryError, LineSegment, NurbsCurve,
    NurbsSurface, Point3, Polyline3, Tolerance, TriangleMesh, UnitVector3, WeightedPoint3,
    join_polylines,
};

pub const PROTOCOL_VERSION: u32 = 1;
const MAX_ITERATIONS: u32 = 1_000_000;
const MAX_STATE_CYCLE_OBJECTS: usize = 100_000;

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
    DocumentObjectStateCycle {
        id: String,
        object_count: usize,
        hide_indices: Vec<usize>,
        lock_indices: Vec<usize>,
    },
    DocumentObjectSwapCycle {
        id: String,
    },
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
    EllipseThreePoint {
        id: String,
        center: [f64; 3],
        first_axis_point: [f64; 3],
        second_axis_point: [f64; 3],
        angle_radians: f64,
    },
    PolylineLength {
        id: String,
        vertices: Vec<[f64; 3]>,
    },
    PolylineArea {
        id: String,
        vertices: Vec<[f64; 3]>,
    },
    PolylineJoin {
        id: String,
        polylines: Vec<Vec<[f64; 3]>>,
    },
    NurbsCurveEvaluate {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        parameter: f64,
    },
    NurbsCurveLength {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    NurbsCurveDivide {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        segment_count: usize,
        include_start: bool,
    },
    NurbsCurveReverse {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        normalized_parameter: f64,
    },
    NurbsCurveTopology {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    NurbsCurveExtractPoints {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    MeshUnifyNormals {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshDisjointPieces {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshCombineIdenticalVertices {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshCullUnusedVertices {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshVolume {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    MeshExtractNonManifold {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        selective: bool,
    },
    MeshExtractDuplicateFaces {
        id: String,
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    NurbsSurfaceExtractPoints {
        id: String,
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
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
            Self::DocumentObjectStateCycle { id, .. }
            | Self::DocumentObjectSwapCycle { id }
            | Self::PointDistance { id, .. }
            | Self::LinePoint { id, .. }
            | Self::CirclePoint { id, .. }
            | Self::ArcThreePoint { id, .. }
            | Self::EllipseThreePoint { id, .. }
            | Self::PolylineLength { id, .. }
            | Self::PolylineArea { id, .. }
            | Self::PolylineJoin { id, .. }
            | Self::NurbsCurveEvaluate { id, .. }
            | Self::NurbsCurveLength { id, .. }
            | Self::NurbsCurveDivide { id, .. }
            | Self::NurbsCurveReverse { id, .. }
            | Self::NurbsCurveTopology { id, .. }
            | Self::NurbsCurveExtractPoints { id, .. }
            | Self::MeshUnifyNormals { id, .. }
            | Self::MeshDisjointPieces { id, .. }
            | Self::MeshCombineIdenticalVertices { id, .. }
            | Self::MeshCullUnusedVertices { id, .. }
            | Self::MeshVolume { id, .. }
            | Self::MeshExtractNonManifold { id, .. }
            | Self::MeshExtractDuplicateFaces { id, .. }
            | Self::NurbsSurfaceExtractPoints { id, .. }
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

    #[error(transparent)]
    Document(#[from] DocumentError),

    #[error("unsupported oracle protocol version {actual}; expected {expected}")]
    ProtocolVersion { actual: u32, expected: u32 },

    #[error("oracle iterations must be from 1 through {MAX_ITERATIONS}, got {0}")]
    InvalidIterations(u32),

    #[error("oracle operation id '{0}' is empty or duplicated")]
    InvalidOperationId(String),

    #[error(
        "document state-cycle object count must be from 1 through {MAX_STATE_CYCLE_OBJECTS}, got {0}"
    )]
    InvalidStateCycleObjectCount(usize),

    #[error("document state-cycle object index {index} is outside object count {object_count}")]
    InvalidStateCycleObjectIndex { index: usize, object_count: usize },

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
        Operation::DocumentObjectStateCycle {
            object_count,
            hide_indices,
            lock_indices,
            ..
        } => document_object_state_cycle(iterations, *object_count, hide_indices, lock_indices)?,
        Operation::DocumentObjectSwapCycle { .. } => document_object_swap_cycle(iterations)?,
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
        Operation::EllipseThreePoint {
            center,
            first_axis_point,
            second_axis_point,
            angle_radians,
            ..
        } => {
            let ellipse = Ellipse3::try_from_three_points(
                point(*center)?,
                point(*first_axis_point)?,
                point(*second_axis_point)?,
                tolerance,
            )?;
            let (point, elapsed) = measure(iterations, || {
                ellipse.point_at_angle(black_box(*angle_radians))
            })?;
            (
                json!({
                    "center": ellipse.center().to_array(),
                    "point": point.to_array(),
                    "radius_x": ellipse.radius_x(),
                    "radius_y": ellipse.radius_y(),
                    "x_axis": ellipse.x_axis().as_vector().to_array(),
                    "y_axis": ellipse.y_axis().as_vector().to_array(),
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
        Operation::PolylineArea { vertices, .. } => {
            let polyline = Polyline3::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                tolerance,
            )?;
            let (area, elapsed) =
                measure(iterations, || black_box(&polyline).planar_area(tolerance))?;
            (json!(area), elapsed)
        }
        Operation::PolylineJoin { polylines, .. } => {
            let polylines = polylines
                .iter()
                .map(|vertices| {
                    Polyline3::try_new(
                        vertices
                            .iter()
                            .map(|coordinates| point(*coordinates))
                            .collect::<Result<Vec<_>, _>>()?,
                        tolerance,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (joined, elapsed) = measure(iterations, || {
                join_polylines(black_box(&polylines), tolerance)
            })?;
            (json!(canonical_join_segments(&joined)), elapsed)
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
        Operation::NurbsCurveLength {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (length, elapsed) = measure(iterations, || black_box(&curve).length(tolerance))?;
            (json!(length), elapsed)
        }
        Operation::NurbsCurveDivide {
            degree,
            control_points,
            knots,
            segment_count,
            include_start,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (points, elapsed) = measure(iterations, || {
                CurveRef::NurbsCurve(black_box(&curve)).divide_by_count(
                    black_box(*segment_count),
                    black_box(*include_start),
                    tolerance,
                )
            })?;
            (
                json!(points.into_iter().map(Point3::to_array).collect::<Vec<_>>()),
                elapsed,
            )
        }
        Operation::NurbsCurveReverse {
            degree,
            control_points,
            knots,
            normalized_parameter,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let ((point, derivative), elapsed) = measure(iterations, || {
                let reversed = black_box(&curve).reversed()?;
                let parameter = reversed.parameter_at(black_box(*normalized_parameter))?;
                reversed.evaluate_with_derivative(parameter)
            })?;
            (
                json!({
                    "derivative": derivative.to_array(),
                    "point": point.to_array(),
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveTopology {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let ((is_closed, is_periodic), elapsed) = measure(iterations, || {
                Ok((black_box(&curve).is_closed()?, curve.is_periodic()))
            })?;
            (
                json!({
                    "is_closed": is_closed,
                    "is_periodic": is_periodic,
                }),
                elapsed,
            )
        }
        Operation::NurbsCurveExtractPoints {
            degree,
            control_points,
            knots,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (points, elapsed) =
                measure(iterations, || black_box(&curve).extract_point_locations())?;
            (
                json!(points.into_iter().map(Point3::to_array).collect::<Vec<_>>()),
                elapsed,
            )
        }
        Operation::MeshUnifyNormals {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((unified, flipped_faces), elapsed) =
                measure(iterations, || black_box(&mesh).unified_face_orientations())?;
            (
                json!({
                    "flipped_faces": flipped_faces,
                    "triangles": unified.triangles(),
                }),
                elapsed,
            )
        }
        Operation::MeshDisjointPieces {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (pieces, elapsed) = measure(iterations, || Ok(black_box(&mesh).disjoint_pieces()))?;
            (
                json!({
                    "disjoint_mesh_count": pieces.len(),
                    "pieces": pieces.iter().map(mesh_value).collect::<Vec<_>>(),
                }),
                elapsed,
            )
        }
        Operation::MeshCombineIdenticalVertices {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((combined, removed_vertices), elapsed) = measure(iterations, || {
                Ok(black_box(&mesh).combined_identical_vertices())
            })?;
            (
                json!({
                    "changed": removed_vertices > 0,
                    "removed_vertices": removed_vertices,
                    "mesh": mesh_value(&combined),
                }),
                elapsed,
            )
        }
        Operation::MeshCullUnusedVertices {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let ((culled, removed_vertices), elapsed) =
                measure(iterations, || Ok(black_box(&mesh).culled_unused_vertices()))?;
            (
                json!({
                    "changed": removed_vertices > 0,
                    "removed_vertices": removed_vertices,
                    "mesh": mesh_value(&culled),
                }),
                elapsed,
            )
        }
        Operation::MeshVolume {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (volume, elapsed) = measure(iterations, || black_box(&mesh).signed_volume())?;
            (json!(volume), elapsed)
        }
        Operation::MeshExtractNonManifold {
            vertices,
            triangles,
            selective,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (extraction, elapsed) = measure(iterations, || {
                black_box(&mesh).extract_non_manifold_faces(3, black_box(*selective))
            })?;
            let value = if let Some(extraction) = extraction {
                let (remainder, extracted) = extraction.into_parts();
                json!({
                    "extracted": mesh_value(&extracted),
                    "remainder": remainder.as_ref().map(mesh_value),
                })
            } else {
                json!({
                    "extracted": null,
                    "remainder": mesh_value(&mesh),
                })
            };
            (value, elapsed)
        }
        Operation::MeshExtractDuplicateFaces {
            vertices,
            triangles,
            ..
        } => {
            let mesh = TriangleMesh::try_new(
                vertices
                    .iter()
                    .map(|coordinates| point(*coordinates))
                    .collect::<Result<Vec<_>, _>>()?,
                triangles.clone(),
                tolerance,
            )?;
            let (extraction, elapsed) =
                measure(
                    iterations,
                    || Ok(black_box(&mesh).extract_duplicate_faces()),
                )?;
            let value = if let Some(extraction) = extraction {
                let (remainder, extracted) = extraction.into_parts();
                json!({
                    "extracted": mesh_value(&extracted),
                    "remainder": remainder.as_ref().map(mesh_value),
                })
            } else {
                json!({
                    "extracted": null,
                    "remainder": mesh_value(&mesh),
                })
            };
            (value, elapsed)
        }
        Operation::NurbsSurfaceExtractPoints {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
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
            let (points, elapsed) = measure(iterations, || {
                Ok(black_box(&surface).extract_point_locations())
            })?;
            (
                json!(points.into_iter().map(Point3::to_array).collect::<Vec<_>>()),
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

fn document_object_state_cycle(
    iterations: u32,
    object_count: usize,
    hide_indices: &[usize],
    lock_indices: &[usize],
) -> Result<(Value, u64), ProbeError> {
    if !(1..=MAX_STATE_CYCLE_OBJECTS).contains(&object_count) {
        return Err(ProbeError::InvalidStateCycleObjectCount(object_count));
    }
    let mut document = Document::default();
    let mut object_ids = Vec::with_capacity(object_count);
    for index in 0..object_count {
        object_ids.push(document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            0.0,
            0.0,
        )?))?);
    }
    let hide_ids = state_cycle_ids(&object_ids, hide_indices)?;
    let lock_ids = state_cycle_ids(&object_ids, lock_indices)?;

    measure_document(iterations, || {
        document.clear_selection();
        for id in &hide_ids {
            document.select_object(*id, SelectionMode::Add)?;
        }
        let hide_count = document.set_objects_visibility(hide_ids.iter().copied(), false)?;
        let modes_after_hide = document_object_modes(&document, &object_ids)?;
        let selected_after_hide = document.selected_object_count();

        let show_count = document.set_objects_visibility(hide_ids.iter().copied(), true)?;
        let modes_after_show = document_object_modes(&document, &object_ids)?;
        for id in &lock_ids {
            document.select_object(*id, SelectionMode::Add)?;
        }
        let lock_count = document.set_objects_locked(lock_ids.iter().copied(), true)?;
        let modes_after_lock = document_object_modes(&document, &object_ids)?;
        let selected_after_lock = document.selected_object_count();

        let unlock_count = document.set_objects_locked(lock_ids.iter().copied(), false)?;
        let modes_after_unlock = document_object_modes(&document, &object_ids)?;
        Ok(json!({
            "hide_count": hide_count,
            "lock_count": lock_count,
            "modes_after_hide": modes_after_hide,
            "modes_after_lock": modes_after_lock,
            "modes_after_show": modes_after_show,
            "modes_after_unlock": modes_after_unlock,
            "selected_after_hide": selected_after_hide,
            "selected_after_lock": selected_after_lock,
            "show_count": show_count,
            "unlock_count": unlock_count,
        }))
    })
}

fn document_object_swap_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default = document.current_layer_id();
    let hidden_layer = document.add_layer("Oracle Hidden", ColorRgb::new(1, 2, 3))?;
    let locked_layer = document.add_layer("Oracle Locked", ColorRgb::new(4, 5, 6))?;
    let mut object_ids = Vec::with_capacity(9);
    for (layer, x) in [(default, 0.0), (hidden_layer, 10.0), (locked_layer, 20.0)] {
        for (offset, attributes) in [
            (0.0, ObjectAttributes::on_layer(layer)),
            (
                1.0,
                ObjectAttributes::on_layer(layer).with_visibility(false),
            ),
            (2.0, ObjectAttributes::on_layer(layer).with_locked(true)),
        ] {
            object_ids.push(document.add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(x + offset, 0.0, 0.0)?),
                attributes,
            )?);
        }
    }
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;
    let labels = [
        "default-normal",
        "default-hidden",
        "default-locked",
        "hidden-layer-normal",
        "hidden-layer-hidden",
        "hidden-layer-locked",
        "locked-layer-normal",
        "locked-layer-hidden",
        "locked-layer-locked",
    ];

    measure_document(iterations, || {
        document.clear_selection();
        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let hide_count_once = document.swap_object_visibility_modes()?;
        let hide_once = document_object_modes(&document, &object_ids)?;
        let selected_after_hide = document.selected_object_count();
        let hide_count_twice = document.swap_object_visibility_modes()?;
        let hide_twice = document_object_modes(&document, &object_ids)?;

        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let lock_count_once = document.swap_object_lock_modes()?;
        let lock_once = document_object_modes(&document, &object_ids)?;
        let selected_after_lock = document.selected_object_count();
        let lock_count_twice = document.swap_object_lock_modes()?;
        let lock_twice = document_object_modes(&document, &object_ids)?;
        Ok(json!({
            "hide_count_once": hide_count_once,
            "hide_count_twice": hide_count_twice,
            "hide_once": hide_once,
            "hide_twice": hide_twice,
            "labels": labels,
            "lock_count_once": lock_count_once,
            "lock_count_twice": lock_count_twice,
            "lock_once": lock_once,
            "lock_twice": lock_twice,
            "selected_after_hide": selected_after_hide,
            "selected_after_lock": selected_after_lock,
        }))
    })
}

fn state_cycle_ids(
    object_ids: &[ObjectId],
    indices: &[usize],
) -> Result<Vec<ObjectId>, ProbeError> {
    let indices = indices.iter().copied().collect::<BTreeSet<_>>();
    if let Some(index) = indices
        .iter()
        .find(|index| **index >= object_ids.len())
        .copied()
    {
        return Err(ProbeError::InvalidStateCycleObjectIndex {
            index,
            object_count: object_ids.len(),
        });
    }
    Ok(indices.into_iter().map(|index| object_ids[index]).collect())
}

fn document_object_modes(
    document: &Document,
    object_ids: &[ObjectId],
) -> Result<Vec<&'static str>, DocumentError> {
    object_ids
        .iter()
        .map(|id| {
            let attributes = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?
                .attributes();
            Ok(if !attributes.is_visible() {
                "hidden"
            } else if attributes.is_locked() {
                "locked"
            } else {
                "normal"
            })
        })
        .collect()
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

fn mesh_value(mesh: &TriangleMesh) -> Value {
    json!({
        "triangles": mesh.triangles(),
        "vertices": mesh.vertices().iter().map(|point| point.to_array()).collect::<Vec<_>>(),
    })
}

fn canonical_join_segments(
    joined: &[viboceros_geometry::JoinedPolyline3],
) -> Vec<Vec<[[f64; 3]; 2]>> {
    let mut polylines = joined
        .iter()
        .map(|component| {
            let mut segments = component
                .polyline()
                .segments()
                .map(|segment| [segment.start().to_array(), segment.end().to_array()])
                .collect::<Vec<_>>();
            if compare_point(&segments.last().unwrap()[1], &segments.first().unwrap()[0]).is_lt() {
                segments.reverse();
                for segment in &mut segments {
                    segment.swap(0, 1);
                }
            }
            segments
        })
        .collect::<Vec<_>>();
    polylines.sort_by(|left, right| compare_segments(left, right));
    polylines
}

fn compare_segments(left: &[[[f64; 3]; 2]], right: &[[[f64; 3]; 2]]) -> std::cmp::Ordering {
    left.iter()
        .flatten()
        .zip(right.iter().flatten())
        .map(|(left, right)| compare_point(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn compare_point(left: &[f64; 3], right: &[f64; 3]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.total_cmp(right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
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

fn measure_document<T>(
    iterations: u32,
    mut operation: impl FnMut() -> Result<T, DocumentError>,
) -> Result<(T, u64), ProbeError> {
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

    fn control(point: [f64; 3], weight: f64) -> ControlPoint {
        ControlPoint { point, weight }
    }

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
            Operation::EllipseThreePoint {
                id: "ellipse".to_owned(),
                center: [1.0, 2.0, 3.0],
                first_axis_point: [5.0, 2.0, 3.0],
                second_axis_point: [3.0, -4.0, 3.0],
                angle_radians: 0.75,
            },
        ]))
        .unwrap();
        assert_eq!(response.engine, "viboceros");
        assert_eq!(response.results[0].id, "distance");
        assert_eq!(response.results[0].value, json!(5.0));
        assert_eq!(response.results[1].value, json!([1.0, 0.5, 0.0]));
        assert_eq!(response.results[2].id, "ellipse");
        assert_eq!(response.results[2].value["radius_x"], json!(4.0));
        assert_eq!(
            response.results[2].value["radius_y"],
            json!(40.0_f64.sqrt())
        );
    }

    #[test]
    fn cycles_document_object_modes_and_prunes_selection() {
        let response = run_request(&request(vec![Operation::DocumentObjectStateCycle {
            id: "object-state".to_owned(),
            object_count: 4,
            hide_indices: vec![2, 0, 2],
            lock_indices: vec![1, 2],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "hide_count": 2,
                "lock_count": 2,
                "modes_after_hide": ["hidden", "normal", "hidden", "normal"],
                "modes_after_lock": ["normal", "locked", "locked", "normal"],
                "modes_after_show": ["normal", "normal", "normal", "normal"],
                "modes_after_unlock": ["normal", "normal", "normal", "normal"],
                "selected_after_hide": 0,
                "selected_after_lock": 0,
                "show_count": 2,
                "unlock_count": 2,
            })
        );

        let error = run_request(&request(vec![Operation::DocumentObjectStateCycle {
            id: "invalid-object-state".to_owned(),
            object_count: 2,
            hide_indices: vec![2],
            lock_indices: Vec::new(),
        }]))
        .unwrap_err();
        assert!(matches!(
            error,
            ProbeError::InvalidStateCycleObjectIndex {
                index: 2,
                object_count: 2
            }
        ));
    }

    #[test]
    fn swaps_document_object_modes_with_rhino_layer_filtering() {
        let response = run_request(&request(vec![Operation::DocumentObjectSwapCycle {
            id: "object-swap".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "hide_count_once": 2,
                "hide_count_twice": 2,
                "hide_once": [
                    "hidden", "normal", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "hide_twice": [
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "labels": [
                    "default-normal", "default-hidden", "default-locked",
                    "hidden-layer-normal", "hidden-layer-hidden", "hidden-layer-locked",
                    "locked-layer-normal", "locked-layer-hidden", "locked-layer-locked",
                ],
                "lock_count_once": 2,
                "lock_count_twice": 2,
                "lock_once": [
                    "locked", "hidden", "normal",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "lock_twice": [
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "selected_after_hide": 0,
                "selected_after_lock": 0,
            })
        );
    }

    #[test]
    fn reports_closed_and_periodic_nurbs_topology() {
        let controls = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
        ]
        .map(|point| ControlPoint { point, weight: 1.0 })
        .to_vec();
        let response = run_request(&request(vec![Operation::NurbsCurveTopology {
            id: "topology".to_owned(),
            degree: 2,
            control_points: controls,
            knots: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({"is_closed": true, "is_periodic": true})
        );
    }

    #[test]
    fn extracts_unique_nurbs_controls_in_rhino_grip_order() {
        let response = run_request(&request(vec![
            Operation::NurbsCurveExtractPoints {
                id: "closed".to_owned(),
                degree: 2,
                control_points: vec![
                    control([0.0, 0.0, 0.0], 1.0),
                    control([3.0, 0.0, 0.0], 1.0),
                    control([3.0, 2.0, 0.0], 1.0),
                    control([0.0, 0.0, 0.0], 1.0),
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
            },
            Operation::NurbsCurveExtractPoints {
                id: "periodic".to_owned(),
                degree: 2,
                control_points: vec![
                    control([0.0, 0.0, 0.0], 1.0),
                    control([2.0, 0.0, 0.0], 1.0),
                    control([1.0, 2.0, 0.0], 1.0),
                    control([0.0, 0.0, 0.0], 1.0),
                    control([2.0, 0.0, 0.0], 1.0),
                ],
                knots: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            },
            Operation::NurbsCurveExtractPoints {
                id: "weighted-periodic".to_owned(),
                degree: 2,
                control_points: vec![
                    control([0.0, 0.0, 0.0], 1.0),
                    control([2.0, 0.0, 0.0], 1.0),
                    control([1.0, 2.0, 0.0], 1.0),
                    control([0.0, 0.0, 0.0], 2.0),
                    control([2.0, 0.0, 0.0], 3.0),
                ],
                knots: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!([[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 2.0, 0.0]])
        );
        assert_eq!(
            response.results[1].value,
            json!([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 2.0, 0.0]])
        );
        assert_eq!(
            response.results[2].value,
            json!([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 2.0, 0.0]])
        );
    }

    #[test]
    fn extracts_unique_periodic_surface_controls_in_grip_order() {
        let response = run_request(&request(vec![Operation::NurbsSurfaceExtractPoints {
            id: "periodic-surface".to_owned(),
            degree_u: 2,
            degree_v: 1,
            control_point_count_u: 5,
            control_point_count_v: 2,
            control_points: vec![
                control([0.0, 0.0, 0.0], 1.0),
                control([2.0, 0.0, 0.0], 1.0),
                control([1.0, 2.0, 0.0], 1.0),
                control([0.0, 0.0, 0.0], 1.0),
                control([2.0, 0.0, 0.0], 1.0),
                control([0.0, 0.0, 3.0], 1.0),
                control([2.0, 0.0, 3.0], 1.0),
                control([1.0, 2.0, 3.0], 1.0),
                control([0.0, 0.0, 3.0], 1.0),
                control([2.0, 0.0, 3.0], 1.0),
            ],
            knots_u: vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!([
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 3.0],
                [2.0, 0.0, 0.0],
                [2.0, 0.0, 3.0],
                [1.0, 2.0, 0.0],
                [1.0, 2.0, 3.0]
            ])
        );
    }

    #[test]
    fn unifies_mesh_face_winding_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshUnifyNormals {
            id: "mesh".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            triangles: vec![[0, 2, 1], [0, 3, 1], [0, 3, 2], [1, 2, 3]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "flipped_faces": 1,
                "triangles": [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            })
        );
    }

    #[test]
    fn splits_mesh_components_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshDisjointPieces {
            id: "mesh".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 2.0, 0.0],
                [1.0, 2.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "disjoint_mesh_count": 2,
                "pieces": [
                    {
                        "triangles": [[0, 1, 2], [3, 4, 5]],
                        "vertices": [
                            [0.0, 0.0, 0.0],
                            [1.0, 0.0, 0.0],
                            [0.0, 1.0, 0.0],
                            [1.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                            [0.0, -1.0, 0.0],
                        ],
                    },
                    {
                        "triangles": [[0, 1, 2]],
                        "vertices": [
                            [0.0, 1.0, 0.0],
                            [0.0, 2.0, 0.0],
                            [1.0, 2.0, 0.0],
                        ],
                    },
                ],
            })
        );
    }

    #[test]
    fn combines_identical_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshCombineIdenticalVertices {
            id: "combined".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, -2.0, 0.0],
                [0.0, 2.0, 0.0],
                [99.0, 99.0, 99.0],
            ],
            triangles: vec![[0, 1, 2], [3, 4, 5]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "changed": true,
                "removed_vertices": 3,
                "mesh": {
                    "triangles": [[3, 1, 2], [1, 3, 4]],
                    "vertices": [
                        [99.0, 99.0, 99.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 0.0],
                        [0.0, -2.0, 0.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn culls_unused_mesh_vertices_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshCullUnusedVertices {
            id: "culled".to_owned(),
            vertices: vec![
                [99.0, 99.0, 99.0],
                [0.0, 0.0, 0.0],
                [98.0, 98.0, 98.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 0.0],
                [97.0, 97.0, 97.0],
            ],
            triangles: vec![[1, 3, 4], [3, 5, 4]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "changed": true,
                "removed_vertices": 3,
                "mesh": {
                    "triangles": [[0, 1, 2], [1, 3, 2]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 0.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn measures_translation_stable_signed_mesh_volume_for_oracle_comparison() {
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let response = run_request(&request(vec![
            Operation::MeshVolume {
                id: "translated".to_owned(),
                vertices: vec![
                    [1.0e9, -2.0e9, 3.0e9],
                    [1.0e9 + 2.0, -2.0e9, 3.0e9],
                    [1.0e9, -2.0e9 + 3.0, 3.0e9],
                    [1.0e9, -2.0e9, 3.0e9 + 4.0],
                ],
                triangles: faces.clone(),
            },
            Operation::MeshVolume {
                id: "reversed".to_owned(),
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [0.0, 0.0, 4.0],
                ],
                triangles: faces
                    .into_iter()
                    .map(|mut face| {
                        face.swap(1, 2);
                        face
                    })
                    .collect(),
            },
        ]))
        .unwrap();
        assert_eq!(response.results[0].value, json!(4.0));
        assert_eq!(response.results[1].value, json!(-4.0));
    }

    #[test]
    fn extracts_non_manifold_mesh_faces_for_oracle_comparison() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 1.0],
        ];
        let triangles = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]];
        let response = run_request(&request(vec![
            Operation::MeshExtractNonManifold {
                id: "all".to_owned(),
                vertices: vertices.clone(),
                triangles: triangles.clone(),
                selective: false,
            },
            Operation::MeshExtractNonManifold {
                id: "hanging".to_owned(),
                vertices,
                triangles,
                selective: true,
            },
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "extracted": {
                    "triangles": [[0, 2, 1], [0, 1, 3], [0, 1, 4]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                        [0.0, -1.0, 1.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 3, 2], [1, 2, 3]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                },
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "extracted": {
                    "triangles": [[0, 1, 2]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, -1.0, 1.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn extracts_duplicate_mesh_faces_for_oracle_comparison() {
        let response = run_request(&request(vec![Operation::MeshExtractDuplicateFaces {
            id: "duplicates".to_owned(),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            triangles: vec![[0, 1, 2], [0, 1, 6], [3, 5, 4]],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "extracted": {
                    "triangles": [[0, 2, 1]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                    ],
                },
                "remainder": {
                    "triangles": [[0, 1, 2], [0, 1, 3]],
                    "vertices": [
                        [0.0, 0.0, 0.0],
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                },
            })
        );
    }

    #[test]
    fn joins_shuffled_polylines_into_canonical_oracle_output() {
        let response = run_request(&request(vec![Operation::PolylineJoin {
            id: "join".to_owned(),
            polylines: vec![
                vec![[4.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
                vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                vec![[3.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            ],
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!([[
                [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                [[2.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
                [[3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]
            ]])
        );
    }

    #[test]
    fn measures_rotated_area_and_rational_curve_length() {
        let response = run_request(&request(vec![
            Operation::PolylineArea {
                id: "area".to_owned(),
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [3.0, 0.0, 3.0],
                    [3.0, 4.0, 3.0],
                    [0.0, 4.0, 0.0],
                    [0.0, 0.0, 0.0],
                ],
            },
            Operation::NurbsCurveLength {
                id: "length".to_owned(),
                degree: 2,
                control_points: vec![
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
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            },
            Operation::NurbsCurveDivide {
                id: "division".to_owned(),
                degree: 2,
                control_points: vec![
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
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                segment_count: 4,
                include_start: true,
            },
            Operation::NurbsCurveReverse {
                id: "reverse".to_owned(),
                degree: 2,
                control_points: vec![
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
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                normalized_parameter: 0.25,
            },
        ]))
        .unwrap();
        let tolerance = Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-12).unwrap();
        assert!(tolerance.approx_eq(
            response.results[0].value.as_f64().unwrap(),
            12.0 * 2.0_f64.sqrt()
        ));
        assert!(tolerance.approx_eq(
            response.results[1].value.as_f64().unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
        let divided = response.results[2].value.as_array().unwrap();
        assert_eq!(divided.len(), 5);
        for (index, actual) in divided.iter().enumerate() {
            let angle = std::f64::consts::FRAC_PI_2 * index as f64 / 4.0;
            let actual = actual.as_array().unwrap();
            assert!(tolerance.approx_eq(actual[0].as_f64().unwrap(), angle.cos()));
            assert!(tolerance.approx_eq(actual[1].as_f64().unwrap(), angle.sin()));
            assert_eq!(actual[2], json!(0.0));
        }
        let reversed = response.results[3].value.as_object().unwrap();
        let point = reversed["point"].as_array().unwrap();
        let derivative = reversed["derivative"].as_array().unwrap();
        let source = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(super::point([1.0, 0.0, 0.0]).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(
                    super::point([1.0, 1.0, 0.0]).unwrap(),
                    std::f64::consts::FRAC_1_SQRT_2,
                )
                .unwrap(),
                WeightedPoint3::try_new(super::point([0.0, 1.0, 0.0]).unwrap(), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (expected_point, expected_derivative) = source.evaluate_with_derivative(0.75).unwrap();
        for coordinate in 0..3 {
            assert!(tolerance.approx_eq(
                point[coordinate].as_f64().unwrap(),
                expected_point.to_array()[coordinate]
            ));
            assert!(tolerance.approx_eq(
                derivative[coordinate].as_f64().unwrap(),
                -expected_derivative.to_array()[coordinate]
            ));
        }
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
