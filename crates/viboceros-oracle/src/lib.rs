//! Versioned compatibility-probe protocol used to compare Viboceros with Rhino.

use std::collections::BTreeSet;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use viboceros_command::{CommandError, CommandRegistry};
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, LayerId, ObjectAttributes, ObjectId, SelectionMode,
};
use viboceros_geometry::{
    AffineTransform3, Circle3, CircularArc3, CurveRef, Ellipse3, GeometryError, LineSegment,
    NurbsCurve, NurbsSurface, Point3, PointCloud3, Polyline3, Tolerance, TriangleMesh, UnitVector3,
    Vector3, WeightedPoint3, join_polylines,
};
use viboceros_io::{
    ThreeDmColorSource, ThreeDmError, ThreeDmGeometry, ThreeDmGroup, ThreeDmLayer, ThreeDmModel,
    ThreeDmObject, read_3dm_file, write_3dm_file,
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
    DocumentObjectIsolationCycle {
        id: String,
    },
    DocumentActionSelectionCycle {
        id: String,
    },
    DocumentAttributeSelectionCycle {
        id: String,
    },
    DocumentObjectNamingCycle {
        id: String,
    },
    DocumentLayerAssignmentCycle {
        id: String,
    },
    DocumentOrientCycle {
        id: String,
    },
    DocumentLinearArrayCycle {
        id: String,
    },
    DocumentRectangularArrayCycle {
        id: String,
    },
    DocumentCurveArrayCycle {
        id: String,
    },
    DocumentPolarArrayCycle {
        id: String,
    },
    DocumentDuplicateSelectionCycle {
        id: String,
    },
    DocumentPointCloudCycle {
        id: String,
    },
    ThreeDmGroupRoundTrip {
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
    NurbsCurveShortFilter {
        id: String,
        degree: usize,
        control_points: Vec<ControlPoint>,
        knots: Vec<f64>,
        maximum_length: f64,
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
    NurbsCurveClassification {
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
            | Self::DocumentObjectIsolationCycle { id }
            | Self::DocumentActionSelectionCycle { id }
            | Self::DocumentAttributeSelectionCycle { id }
            | Self::DocumentObjectNamingCycle { id }
            | Self::DocumentLayerAssignmentCycle { id }
            | Self::DocumentOrientCycle { id }
            | Self::DocumentLinearArrayCycle { id }
            | Self::DocumentRectangularArrayCycle { id }
            | Self::DocumentCurveArrayCycle { id }
            | Self::DocumentPolarArrayCycle { id }
            | Self::DocumentDuplicateSelectionCycle { id }
            | Self::DocumentPointCloudCycle { id }
            | Self::ThreeDmGroupRoundTrip { id }
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
            | Self::NurbsCurveShortFilter { id, .. }
            | Self::NurbsCurveDivide { id, .. }
            | Self::NurbsCurveReverse { id, .. }
            | Self::NurbsCurveTopology { id, .. }
            | Self::NurbsCurveClassification { id, .. }
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

    #[error(transparent)]
    Command(#[from] CommandError),

    #[error(transparent)]
    ThreeDm(#[from] ThreeDmError),

    #[error("unsupported oracle protocol version {actual}; expected {expected}")]
    ProtocolVersion { actual: u32, expected: u32 },

    #[error("oracle iterations must be from 1 through {MAX_ITERATIONS}, got {0}")]
    InvalidIterations(u32),

    #[error("oracle operation id '{0}' is empty or duplicated")]
    InvalidOperationId(String),

    #[error("oracle maximum curve length must be finite and strictly positive, got {0}")]
    InvalidMaximumCurveLength(f64),

    #[error(
        "document state-cycle object count must be from 1 through {MAX_STATE_CYCLE_OBJECTS}, got {0}"
    )]
    InvalidStateCycleObjectCount(usize),

    #[error("document state-cycle object index {index} is outside object count {object_count}")]
    InvalidStateCycleObjectIndex { index: usize, object_count: usize },

    #[error("oracle timing exceeded the 64-bit nanosecond range")]
    TimingOverflow,

    #[error("oracle fixture invariant failed: {0}")]
    FixtureInvariant(&'static str),
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
        if let Operation::NurbsCurveShortFilter { maximum_length, .. } = operation
            && (!maximum_length.is_finite() || *maximum_length <= 0.0)
        {
            return Err(ProbeError::InvalidMaximumCurveLength(*maximum_length));
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
        Operation::DocumentObjectIsolationCycle { .. } => {
            document_object_isolation_cycle(iterations)?
        }
        Operation::DocumentActionSelectionCycle { .. } => {
            document_action_selection_cycle(iterations)?
        }
        Operation::DocumentAttributeSelectionCycle { .. } => {
            document_attribute_selection_cycle(iterations)?
        }
        Operation::DocumentObjectNamingCycle { .. } => document_object_naming_cycle(iterations)?,
        Operation::DocumentLayerAssignmentCycle { .. } => {
            document_layer_assignment_cycle(iterations)?
        }
        Operation::DocumentOrientCycle { .. } => document_orient_cycle(iterations, tolerance)?,
        Operation::DocumentLinearArrayCycle { .. } => {
            document_linear_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentRectangularArrayCycle { .. } => {
            document_rectangular_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentCurveArrayCycle { .. } => {
            document_curve_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentPolarArrayCycle { .. } => {
            document_polar_array_cycle(iterations, tolerance)?
        }
        Operation::DocumentDuplicateSelectionCycle { .. } => {
            document_duplicate_selection_cycle(iterations, tolerance)?
        }
        Operation::DocumentPointCloudCycle { .. } => {
            document_point_cloud_cycle(iterations, tolerance)?
        }
        Operation::ThreeDmGroupRoundTrip { .. } => {
            three_dm_group_round_trip(iterations, tolerance)?
        }
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
        Operation::NurbsCurveShortFilter {
            degree,
            control_points,
            knots,
            maximum_length,
            ..
        } => {
            let curve = NurbsCurve::try_new_rational(
                *degree,
                weighted_points(control_points)?,
                knots.clone(),
            )?;
            let (is_short, elapsed) = measure(iterations, || {
                Ok(CurveRef::NurbsCurve(black_box(&curve)).length(tolerance)?
                    <= black_box(*maximum_length))
            })?;
            (json!(is_short), elapsed)
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
        Operation::NurbsCurveClassification {
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
            let (
                (
                    is_linear_model,
                    is_linear_zero,
                    is_planar_model,
                    sel_line_match,
                    sel_polyline_match,
                ),
                elapsed,
            ) = measure(iterations, || {
                let is_linear_zero = black_box(&curve).is_linear_at_zero_tolerance()?;
                Ok((
                    curve.is_linear(tolerance)?,
                    is_linear_zero,
                    curve.is_planar(tolerance)?,
                    curve.spans().count() == 1 && is_linear_zero,
                    curve.degree() == 1 && curve.control_points().len() > 2,
                ))
            })?;
            (
                json!({
                    "is_linear_model": is_linear_model,
                    "is_linear_zero": is_linear_zero,
                    "is_planar_model": is_planar_model,
                    "sel_line_match": sel_line_match,
                    "sel_polyline_match": sel_polyline_match,
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

fn document_object_isolation_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default = document.current_layer_id();
    let hidden_layer = document.add_layer("Isolation Hidden", ColorRgb::new(1, 2, 3))?;
    let locked_layer = document.add_layer("Isolation Locked", ColorRgb::new(4, 5, 6))?;
    let mut object_ids = Vec::with_capacity(10);
    for (x, attributes) in [
        (0.0, ObjectAttributes::on_layer(default)),
        (1.0, ObjectAttributes::on_layer(default)),
        (
            2.0,
            ObjectAttributes::on_layer(default).with_visibility(false),
        ),
        (3.0, ObjectAttributes::on_layer(default).with_locked(true)),
        (10.0, ObjectAttributes::on_layer(hidden_layer)),
        (
            11.0,
            ObjectAttributes::on_layer(hidden_layer).with_visibility(false),
        ),
        (
            12.0,
            ObjectAttributes::on_layer(hidden_layer).with_locked(true),
        ),
        (20.0, ObjectAttributes::on_layer(locked_layer)),
        (
            21.0,
            ObjectAttributes::on_layer(locked_layer).with_visibility(false),
        ),
        (
            22.0,
            ObjectAttributes::on_layer(locked_layer).with_locked(true),
        ),
    ] {
        object_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(x, 0.0, 0.0)?),
            attributes,
        )?);
    }
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;
    let labels = [
        "default-selected",
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
        let isolate_count = document.isolate_selected_objects()?;
        let isolate_repeat_count = document.isolate_selected_objects()?;
        let after_isolate = document_object_modes(&document, &object_ids)?;
        let selected_after_isolate = document.selected_object_count();
        let unisolate_count = document.unisolate_objects()?;
        let unisolate_repeat_count = document.unisolate_objects()?;
        let after_unisolate = document_object_modes(&document, &object_ids)?;
        let selected_after_unisolate = document.selected_object_count();

        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let isolate_lock_count = document.isolate_lock_selected_objects()?;
        let isolate_lock_repeat_count = document.isolate_lock_selected_objects()?;
        let after_isolate_lock = document_object_modes(&document, &object_ids)?;
        let selected_after_isolate_lock = document.selected_object_count();
        let unisolate_lock_count = document.unisolate_locked_objects()?;
        let unisolate_lock_repeat_count = document.unisolate_locked_objects()?;
        let after_unisolate_lock = document_object_modes(&document, &object_ids)?;
        let selected_after_unisolate_lock = document.selected_object_count();
        Ok(json!({
            "after_isolate": after_isolate,
            "after_isolate_lock": after_isolate_lock,
            "after_unisolate": after_unisolate,
            "after_unisolate_lock": after_unisolate_lock,
            "isolate_count": isolate_count,
            "isolate_lock_count": isolate_lock_count,
            "isolate_lock_repeat_count": isolate_lock_repeat_count,
            "isolate_repeat_count": isolate_repeat_count,
            "labels": labels,
            "selected_after_isolate": selected_after_isolate,
            "selected_after_isolate_lock": selected_after_isolate_lock,
            "selected_after_unisolate": selected_after_unisolate,
            "selected_after_unisolate_lock": selected_after_unisolate_lock,
            "unisolate_count": unisolate_count,
            "unisolate_lock_count": unisolate_lock_count,
            "unisolate_lock_repeat_count": unisolate_lock_repeat_count,
            "unisolate_repeat_count": unisolate_repeat_count,
        }))
    })
}

fn document_action_selection_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let mut object_ids = Vec::with_capacity(4);
    for index in 0..4 {
        object_ids.push(document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            0.0,
            0.0,
        )?))?);
    }

    let mut batch_document = Document::default();
    batch_document.begin_transaction("Oracle batch")?;
    let mut batch_ids = Vec::with_capacity(2);
    for index in 0..2 {
        batch_ids.push(batch_document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            1.0,
            0.0,
        )?))?);
    }
    batch_document.commit_transaction()?;

    measure_document(iterations, || {
        document.clear_selection();
        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let last_default_count = document.select_last_changed(true);
        let last_default = document_selected_indices(&document, &object_ids);
        let previous_once_count = document.select_previous(true);
        let previous_once = document_selected_indices(&document, &object_ids);
        let previous_twice_count = document.select_previous(true);
        let previous_twice = document_selected_indices(&document, &object_ids);

        document.select_object(object_ids[0], SelectionMode::Replace)?;
        let last_add_count = document.select_last_changed(false);
        let last_add = document_selected_indices(&document, &object_ids);

        establish_previous_selection(&mut document, &object_ids)?;
        let previous_default_count = document.select_previous(true);
        let previous_default = document_selected_indices(&document, &object_ids);
        let previous_default_twice_count = document.select_previous(true);
        let previous_default_twice = document_selected_indices(&document, &object_ids);

        establish_previous_selection(&mut document, &object_ids)?;
        let previous_add_count = document.select_previous(false);
        let previous_add = document_selected_indices(&document, &object_ids);

        batch_document.clear_selection();
        let batch_last_count = batch_document.select_last_changed(true);
        let batch_last = document_selected_indices(&batch_document, &batch_ids);
        Ok(json!({
            "batch_last": batch_last,
            "batch_last_count": batch_last_count,
            "last_add": last_add,
            "last_add_count": last_add_count,
            "last_default": last_default,
            "last_default_count": last_default_count,
            "previous_add": previous_add,
            "previous_add_count": previous_add_count,
            "previous_default": previous_default,
            "previous_default_count": previous_default_count,
            "previous_default_twice": previous_default_twice,
            "previous_default_twice_count": previous_default_twice_count,
            "previous_once": previous_once,
            "previous_once_count": previous_once_count,
            "previous_twice": previous_twice,
            "previous_twice_count": previous_twice_count,
        }))
    })
}

fn document_attribute_selection_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default_layer = document.current_layer_id();
    let hidden_layer = document.add_layer("Hidden Parts", ColorRgb::new(10, 20, 30))?;
    let locked_layer = document.add_layer("Locked Parts", ColorRgb::new(40, 50, 60))?;
    let selected_color = ColorRgb::new(10, 20, 30);
    let specifications = [
        (default_layer, None, true, false),
        (default_layer, Some("BoltA"), true, false),
        (default_layer, Some("bolta"), true, false),
        (default_layer, Some("BoltLong"), true, false),
        (default_layer, Some("Peer"), true, false),
        (default_layer, Some("BoltA"), false, false),
        (default_layer, Some("BoltA"), true, true),
        (hidden_layer, Some("BoltA"), true, false),
        (hidden_layer, Some("BoltA"), false, false),
        (locked_layer, Some("BoltA"), true, false),
        (locked_layer, Some("BoltA"), true, true),
    ];
    let mut object_ids = Vec::with_capacity(specifications.len());
    for (index, (layer, name, visible, locked)) in specifications.into_iter().enumerate() {
        let mut attributes = ObjectAttributes::on_layer(layer)
            .with_visibility(visible)
            .with_locked(locked);
        if let Some(name) = name {
            attributes = attributes.with_name(name);
        }
        if [0, 1, 5, 6].contains(&index) {
            attributes = attributes.with_object_color(selected_color);
        }
        object_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(index as f64, 0.0, 0.0)?),
            attributes,
        )?);
    }
    document.add_group(
        Some("Team".to_owned()),
        [object_ids[1], object_ids[4], object_ids[6]],
    )?;
    document.add_group(Some("team".to_owned()), [object_ids[2]])?;
    document.add_group(Some("Overlap".to_owned()), [object_ids[1], object_ids[3]])?;
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;

    measure_document(iterations, || {
        document.set_layer_visibility(hidden_layer, false)?;
        document.set_layer_locked(locked_layer, true)?;

        document.clear_selection();
        document.select_objects_direct([object_ids[0]], SelectionMode::Replace)?;
        let name_count = document.select_objects_by_name_pattern("BOLT?");
        let name = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([object_ids[0]], SelectionMode::Replace)?;
        let group_upper_count = document.select_group_objects_by_name("Team");
        let group_upper = document_selected_indices(&document, &object_ids);
        let group_lower_count = document.select_group_objects_by_name("team");
        let group_lower = document_selected_indices(&document, &object_ids);
        let group_wrong_case_count = document.select_group_objects_by_name("TEAM");
        let group_wrong_case = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([object_ids[0]], SelectionMode::Replace)?;
        let hidden_layer_count = document.select_layer_objects_by_name_pattern("hidden parts")?;
        let hidden_layer_selection = document_selected_indices(&document, &object_ids);
        let hidden_layer_visible = document
            .layer(hidden_layer)
            .is_some_and(|layer| layer.is_visible());
        let locked_layer_count = document.select_layer_objects_by_name_pattern("LOCKED*")?;
        let locked_layer_selection = document_selected_indices(&document, &object_ids);
        let locked_layer_locked = document
            .layer(locked_layer)
            .is_some_and(|layer| layer.is_locked());
        let all_layers_count = document.select_layer_objects_by_name_pattern("*")?;
        let all_layers = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([object_ids[9]], SelectionMode::Replace)?;
        let color_count = document.select_objects_by_display_color(selected_color)?;
        let color = document_selected_indices(&document, &object_ids);
        Ok(json!({
            "all_layers": all_layers,
            "all_layers_count": all_layers_count,
            "color": color,
            "color_count": color_count,
            "group_lower": group_lower,
            "group_lower_count": group_lower_count,
            "group_upper": group_upper,
            "group_upper_count": group_upper_count,
            "group_wrong_case": group_wrong_case,
            "group_wrong_case_count": group_wrong_case_count,
            "hidden_layer": hidden_layer_selection,
            "hidden_layer_count": hidden_layer_count,
            "hidden_layer_visible": hidden_layer_visible,
            "locked_layer": locked_layer_selection,
            "locked_layer_count": locked_layer_count,
            "locked_layer_locked": locked_layer_locked,
            "name": name,
            "name_count": name_count,
        }))
    })
}

fn document_object_naming_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let mut object_ids = Vec::with_capacity(3);
    for index in 0..3 {
        object_ids.push(document.add_geometry(Geometry::Point(Point3::try_new(
            index as f64,
            0.0,
            0.0,
        )?))?);
    }

    measure_document(iterations, || {
        let shared_count = document
            .set_object_names(object_ids.iter().map(|id| (*id, Some("Sample".to_owned()))))?;
        let shared = document_object_names(&document, &object_ids)?;
        let counter_count = document.set_object_names(
            object_ids
                .iter()
                .enumerate()
                .map(|(index, id)| (*id, Some(format!("Sample {index}")))),
        )?;
        let counter = document_object_names(&document, &object_ids)?;
        let clear_count = document.set_object_names(object_ids.iter().map(|id| (*id, None)))?;
        Ok(json!({
            "clear_count": clear_count,
            "cleared": document_object_names(&document, &object_ids)?,
            "counter": counter,
            "counter_count": counter_count,
            "shared": shared,
            "shared_count": shared_count,
        }))
    })
}

fn document_layer_assignment_cycle(iterations: u32) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::default();
    let default_layer = document.current_layer_id();
    let normal_layer = document.add_layer("Normal", ColorRgb::new(10, 20, 30))?;
    let hidden_layer = document.add_layer("Hidden", ColorRgb::new(40, 50, 60))?;
    let locked_layer = document.add_layer("Locked", ColorRgb::new(70, 80, 90))?;
    let mut object_ids = Vec::with_capacity(5);
    for index in 0..5 {
        object_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(index as f64, 0.0, 0.0)?),
            ObjectAttributes::on_layer(default_layer).with_name(format!("Part{index}")),
        )?);
    }
    document.add_group(Some("Assembly".to_owned()), [object_ids[0], object_ids[1]])?;
    document.set_layer_visibility(hidden_layer, false)?;
    document.set_layer_locked(locked_layer, true)?;

    let value = (|| -> Result<Value, DocumentError> {
        document.select_objects_direct([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
        let change_count =
            document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let change_layers = document_layer_names(&document, &object_ids[..2])?;
        let change_selected = document_selected_indices(&document, &object_ids);
        let change_group_sizes = document_group_sizes_for(&document, &object_ids[..2]);
        let current_after_change = document
            .layer(document.current_layer_id())
            .ok_or(DocumentError::LayerNotFound(document.current_layer_id()))?
            .name()
            .to_owned();
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
        let copies =
            document.copy_objects_to_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let copy_layers = document_layer_names(&document, &copies)?;
        let copy_names = document_object_names(&document, &copies)?;
        let copy_group_sizes = document_group_sizes_for(&document, &copies);
        let copy_selected = document_selected_indices(&document, &copies);
        let original_selected_after_copy = document_selected_indices(&document, &object_ids);
        let copy_count = copies.len();
        undo_required(&mut document)?;

        document.set_objects_layer([object_ids[0]], normal_layer)?;
        document.select_objects_direct([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
        let mixed_copies =
            document.copy_objects_to_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let mixed_copy_layers = document_layer_names(&document, &mixed_copies)?;
        let mixed_copy_group_sizes = document_group_sizes_for(&document, &mixed_copies);
        let mixed_copy_count = mixed_copies.len();
        undo_required(&mut document)?;
        undo_required(&mut document)?;

        document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?;
        let same_layer_copy_count = document
            .copy_objects_to_layer([object_ids[0], object_ids[1]], normal_layer)?
            .len();
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[2]], SelectionMode::Replace)?;
        let hidden_change_count = document.set_objects_layer([object_ids[2]], hidden_layer)?;
        let hidden_change_selected = document_selected_indices(&document, &object_ids);
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[3]], SelectionMode::Replace)?;
        let locked_change_count = document.set_objects_layer([object_ids[3]], locked_layer)?;
        let locked_change_selected = document_selected_indices(&document, &object_ids);
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[2]], SelectionMode::Replace)?;
        let hidden_copies = document.copy_objects_to_layer([object_ids[2]], hidden_layer)?;
        let hidden_copy_layers = document_layer_names(&document, &hidden_copies)?;
        let hidden_copy_selected = document_selected_indices(&document, &hidden_copies);
        let hidden_copy_count = hidden_copies.len();
        undo_required(&mut document)?;

        document.select_objects_direct([object_ids[3]], SelectionMode::Replace)?;
        let locked_copies = document.copy_objects_to_layer([object_ids[3]], locked_layer)?;
        let locked_copy_layers = document_layer_names(&document, &locked_copies)?;
        let locked_copy_selected = document_selected_indices(&document, &locked_copies);
        let original_selected_after_destination_copies =
            document_selected_indices(&document, &object_ids);
        let locked_copy_count = locked_copies.len();
        undo_required(&mut document)?;

        Ok(json!({
            "change_count": change_count,
            "change_group_sizes": change_group_sizes,
            "change_layers": change_layers,
            "change_selected": change_selected,
            "copy_count": copy_count,
            "copy_group_sizes": copy_group_sizes,
            "copy_layers": copy_layers,
            "copy_names": copy_names,
            "copy_selected": copy_selected,
            "current_after_change": current_after_change,
            "current_unchanged": document.current_layer_id() == default_layer,
            "hidden_change_count": hidden_change_count,
            "hidden_change_selected": hidden_change_selected,
            "hidden_copy_count": hidden_copy_count,
            "hidden_copy_layers": hidden_copy_layers,
            "hidden_copy_selected": hidden_copy_selected,
            "locked_change_count": locked_change_count,
            "locked_change_selected": locked_change_selected,
            "locked_copy_count": locked_copy_count,
            "locked_copy_layers": locked_copy_layers,
            "locked_copy_selected": locked_copy_selected,
            "mixed_copy_count": mixed_copy_count,
            "mixed_copy_group_sizes": mixed_copy_group_sizes,
            "mixed_copy_layers": mixed_copy_layers,
            "original_selected_after_copy": original_selected_after_copy,
            "original_selected_after_destination_copies":
                original_selected_after_destination_copies,
            "same_layer_copy_count": same_layer_copy_count,
        }))
    })()?;

    document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?;
    document.set_objects_layer([object_ids[0], object_ids[1]], default_layer)?;
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(document.set_objects_layer([object_ids[0], object_ids[1]], normal_layer)?);
        black_box(document.set_objects_layer([object_ids[0], object_ids[1]], default_layer)?);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed_ns))
}

fn document_linear_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let source_points = [
        Point3::try_new(1.0, 2.0, 3.0)?,
        Point3::try_new(4.0, 2.0, 3.0)?,
    ];
    let mut original_ids = Vec::with_capacity(source_points.len());
    for (index, point) in source_points.iter().enumerate() {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(*point),
            ObjectAttributes::on_layer(layer).with_name(index.to_string()),
        )?);
    }
    document.add_group(
        Some("Viboceros Linear Array Group".to_owned()),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, "ArrayLinear 4 0,0,0 2,-1,3")?;

    let records = sorted_document_point_records(&document)?;
    let locations_after_array = records
        .iter()
        .map(|(_, coordinates, _)| *coordinates)
        .collect::<Vec<_>>();
    let names_after_array = records
        .iter()
        .map(|(_, _, name)| name.clone())
        .collect::<Vec<_>>();
    let selected_after_array = records
        .iter()
        .filter(|(id, _, _)| document.is_selected(*id))
        .map(|(_, coordinates, _)| *coordinates)
        .collect::<Vec<_>>();
    let originals_selected_after_array = original_ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
        .collect::<Vec<_>>();
    let groups_after_array = document_group_point_locations(&document)?;
    let value = json!({
        "command_succeeded": true,
        "groups_after_array": groups_after_array,
        "locations_after_array": locations_after_array,
        "names_after_array": names_after_array,
        "originals_selected_after_array": originals_selected_after_array,
        "selected_after_array": selected_after_array,
    });

    let spacing = Point3::try_new(0.0, 0.0, 0.0)?.vector_to(Point3::try_new(2.0, -1.0, 3.0)?)?;
    let (_, elapsed_ns) = measure(iterations, || {
        (1..4)
            .flat_map(|copy_index| {
                source_points
                    .into_iter()
                    .map(move |source| source.translated(spacing.scaled(copy_index as f64)?))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok((value, elapsed_ns))
}

fn document_rectangular_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let fill = document_rectangular_array_scenario(
        tolerance,
        &registry,
        "fill",
        "Array 3 2 1 10 -6 0 Mode=Fill",
    )?;
    let unit_cell = document_rectangular_array_scenario(
        tolerance,
        &registry,
        "unit-cell",
        "Array 3 2 2 2 -1 4 Mode=UnitCell",
    )?;
    let value = json!({
        "fill": fill,
        "unit_cell": unit_cell,
    });

    let source_points = [
        Point3::try_new(1.0, 2.0, 3.0)?,
        Point3::try_new(4.0, 2.0, 3.0)?,
    ];
    let translations = (0..2)
        .flat_map(|z_index| {
            (0..2).flat_map(move |y_index| {
                (0..3)
                    .filter(move |x_index| *x_index != 0 || y_index != 0 || z_index != 0)
                    .map(move |x_index| {
                        Vector3::try_new(
                            2.0 * x_index as f64,
                            -y_index as f64,
                            4.0 * z_index as f64,
                        )
                    })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (_, elapsed_ns) = measure(iterations, || {
        translations
            .iter()
            .flat_map(|translation| {
                source_points
                    .iter()
                    .map(|source| source.translated(*translation))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok((value, elapsed_ns))
}

fn document_rectangular_array_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let source_points = [
        Point3::try_new(1.0, 2.0, 3.0)?,
        Point3::try_new(4.0, 2.0, 3.0)?,
    ];
    let mut original_ids = Vec::with_capacity(source_points.len());
    for (index, point) in source_points.iter().enumerate() {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Point(*point),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {index}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Rectangular Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let records = sorted_document_point_records(&document)?;
    Ok(json!({
        "command_succeeded": true,
        "groups_after_array": document_group_point_locations(&document)?,
        "locations_after_array": records
            .iter()
            .map(|(_, coordinates, _)| *coordinates)
            .collect::<Vec<_>>(),
        "names_after_array": records
            .iter()
            .map(|(_, _, name)| name.clone())
            .collect::<Vec<_>>(),
        "originals_selected_after_array": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
        "selected_after_array": records
            .iter()
            .filter(|(id, _, _)| document.is_selected(*id))
            .map(|(_, coordinates, _)| *coordinates)
            .collect::<Vec<_>>(),
    }))
}

fn document_orient_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let value = json!({
        "orient_default": document_orient_scenario(
            tolerance,
            &registry,
            "orient-default",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4",
        )?,
        "orient_copy_no": document_orient_scenario(
            tolerance,
            &registry,
            "orient-copy-no",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale=No",
        )?,
        "orient_copy_1d": document_orient_scenario(
            tolerance,
            &registry,
            "orient-copy-1d",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale=1D",
        )?,
        "orient_copy_3d": document_orient_scenario(
            tolerance,
            &registry,
            "orient-copy-3d",
            "Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale=3D",
        )?,
        "orient_spatial": document_orient_scenario(
            tolerance,
            &registry,
            "orient-spatial",
            "Orient 1,2,3 2,4,6 -5,4,2 -7,8,3 Copy=Yes Scale=No",
        )?,
        "orient3_default": document_orient_scenario(
            tolerance,
            &registry,
            "orient3-default",
            "Orient3Pt 1,2,3 3,2,3 1,3,4 10,-1,4 10,5,4 8,-1,8",
        )?,
        "orient3_copy_scale": document_orient_scenario(
            tolerance,
            &registry,
            "orient3-copy-scale",
            "Orient3Pt 1,2,3 3,2,3 1,3,4 10,-1,4 10,5,4 8,-1,8 Copy=Yes Scale=Yes",
        )?,
    });

    let source_origin = Point3::try_new(1.0, 2.0, 3.0)?;
    let target_origin = Point3::try_new(-5.0, 4.0, 2.0)?;
    let source_direction = Vector3::try_new(1.0, 2.0, 3.0)?.normalized(tolerance)?;
    let target_direction = Vector3::try_new(-2.0, 4.0, 1.0)?.normalized(tolerance)?;
    let (_, elapsed_ns) = measure(iterations, || {
        AffineTransform3::try_direction_mapping(
            black_box(source_origin),
            source_direction,
            black_box(target_origin),
            target_direction,
            1.0,
            1.0,
            tolerance,
        )
    })?;
    Ok((value, elapsed_ns))
}

fn document_orient_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let origin = Point3::try_new(1.0, 2.0, 3.0)?;
    let mut original_ids = Vec::with_capacity(3);
    for (axis, offset) in [
        ("x", [1.0, 0.0, 0.0]),
        ("y", [0.0, 1.0, 0.0]),
        ("z", [0.0, 0.0, 1.0]),
    ] {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                origin,
                origin.translated(Vector3::try_from(offset)?)?,
                tolerance,
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {axis}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Orient Group {label}")),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(&document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);
    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(&document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
    }))
}

fn document_curve_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let no_rotation_items = document_curve_array_scenario(
        tolerance,
        &registry,
        "no-rotation-items",
        CurveArrayFixturePath::Line,
        [0.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=NoRotation PathName=Rail",
    )?;
    let no_rotation_distance = document_curve_array_scenario(
        tolerance,
        &registry,
        "no-rotation-distance",
        CurveArrayFixturePath::Line,
        [0.0, 0.0, 0.0],
        "ArrayCrv Distance=3 Orientation=NoRotation PathName=Rail",
    )?;
    let base_point = document_curve_array_scenario(
        tolerance,
        &registry,
        "base-point",
        CurveArrayFixturePath::Line,
        [20.0, 0.0, 0.0],
        "ArrayCrv 4 BasePoint=20,0,0 Orientation=NoRotation PathName=Rail",
    )?;
    let freeform = document_curve_array_scenario(
        tolerance,
        &registry,
        "freeform",
        CurveArrayFixturePath::TiltedArc,
        [5.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=Freeform PathName=Rail",
    )?;
    let freeform_nurbs = document_curve_array_scenario(
        tolerance,
        &registry,
        "freeform-nurbs",
        CurveArrayFixturePath::SpatialNurbs,
        [0.0, 0.0, 0.0],
        "ArrayCrv 5 Orientation=Freeform PathName=Rail",
    )?;
    let roadlike = document_curve_array_scenario(
        tolerance,
        &registry,
        "roadlike",
        CurveArrayFixturePath::TiltedArc,
        [5.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=Roadlike PathName=Rail",
    )?;
    let stairlike = document_curve_array_scenario(
        tolerance,
        &registry,
        "stairlike",
        CurveArrayFixturePath::TiltedArc,
        [5.0, 0.0, 0.0],
        "ArrayCrv 4 Orientation=Stairlike PathName=Rail",
    )?;
    let value = json!({
        "base_point": base_point,
        "freeform": freeform,
        "freeform_nurbs": freeform_nurbs,
        "no_rotation_distance": no_rotation_distance,
        "no_rotation_items": no_rotation_items,
        "roadlike": roadlike,
        "stairlike": stairlike,
    });

    let line = LineSegment::try_new(
        Point3::try_new(0.0, 0.0, 0.0)?,
        Point3::try_new(10.0, 0.0, 0.0)?,
        tolerance,
    )?;
    let (_, elapsed_ns) = measure(iterations, || {
        CurveRef::Line(&line).divide_by_count_samples(3, true, tolerance)
    })?;
    Ok((value, elapsed_ns))
}

#[derive(Clone, Copy)]
enum CurveArrayFixturePath {
    Line,
    TiltedArc,
    SpatialNurbs,
}

impl CurveArrayFixturePath {
    fn geometry(self, tolerance: Tolerance) -> Result<Geometry, GeometryError> {
        match self {
            Self::Line => Ok(Geometry::Line(LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0)?,
                Point3::try_new(10.0, 0.0, 0.0)?,
                tolerance,
            )?)),
            Self::TiltedArc => Ok(Geometry::Arc(CircularArc3::try_from_three_points(
                Point3::try_new(5.0, 0.0, 0.0)?,
                Point3::try_new(0.0, 3.0, 4.0)?,
                Point3::try_new(-5.0, 0.0, 0.0)?,
                tolerance,
            )?)),
            Self::SpatialNurbs => Ok(Geometry::NurbsCurve(NurbsCurve::try_new(
                3,
                [
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 3.0],
                    [4.0, 3.0, -1.0],
                    [7.0, 5.0, 4.0],
                    [10.0, 8.0, 6.0],
                ]
                .into_iter()
                .map(Point3::try_from)
                .collect::<Result<Vec<_>, _>>()?,
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            )?)),
        }
    }
}

fn document_curve_array_scenario(
    tolerance: Tolerance,
    registry: &CommandRegistry,
    label: &str,
    path: CurveArrayFixturePath,
    source_anchor: [f64; 3],
    command: &str,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let anchor = Point3::try_from(source_anchor)?;
    // Keep the spatial-frame endpoints away from half-micro rounding
    // boundaries while retaining a longer-than-unit orientation witness.
    let source_axis_length = match path {
        CurveArrayFixturePath::SpatialNurbs => 1.25,
        CurveArrayFixturePath::Line | CurveArrayFixturePath::TiltedArc => 1.0,
    };
    let mut original_ids = Vec::with_capacity(3);
    for (axis, offset) in [
        ("x", [source_axis_length, 0.0, 0.0]),
        ("y", [0.0, source_axis_length, 0.0]),
        ("z", [0.0, 0.0, source_axis_length]),
    ] {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                anchor,
                anchor.translated(Vector3::try_from(offset)?)?,
                tolerance,
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {axis}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Curve Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    let path_id = document.add_geometry_with_attributes(
        path.geometry(tolerance)?,
        ObjectAttributes::on_layer(layer).with_name("Rail"),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(&mut document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(&document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);
    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(&document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| curve_array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": curve_array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
        "path_selected": document.is_selected(path_id),
    }))
}

fn curve_array_line_records_value(records: &[ArrayLineRecord]) -> Value {
    fn rounded_point(point: [f64; 3]) -> [f64; 3] {
        point.map(|coordinate| {
            let rounded = (coordinate * 1.0e6).round() / 1.0e6;
            if rounded == 0.0 { 0.0 } else { rounded }
        })
    }

    Value::Array(
        records
            .iter()
            .map(|record| {
                json!({
                    "end": rounded_point(record.end),
                    "name": record.name,
                    "selected": record.selected,
                    "start": rounded_point(record.start),
                })
            })
            .collect(),
    )
}

fn document_polar_array_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::new(tolerance);
    let full_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "full",
        "ArrayPolar 4 0,0,0 360 Rotate=Yes",
    )?;
    let negative_full_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "negative-full",
        "ArrayPolar 4 0,0,0 -360 Rotate=Yes",
    )?;
    let multi_turn_z_offset_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "multi-turn",
        "ArrayPolar 4 0,0,0 720 Rotate=Yes ZOffset=2",
    )?;
    let partial_rotate_no = document_polar_array_scenario(
        &mut document,
        &registry,
        "partial-no",
        "ArrayPolar 4 0,0,0 180 Rotate=No",
    )?;
    let partial_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "partial-yes",
        "ArrayPolar 4 0,0,0 180 Rotate=Yes",
    )?;
    let z_offset_rotate_yes = document_polar_array_scenario(
        &mut document,
        &registry,
        "z-offset",
        "ArrayPolar 4 0,0,0 180 Rotate=Yes ZOffset=2",
    )?;
    let value = json!({
        "full_rotate_yes": full_rotate_yes,
        "multi_turn_z_offset_rotate_yes": multi_turn_z_offset_rotate_yes,
        "negative_full_rotate_yes": negative_full_rotate_yes,
        "partial_rotate_no": partial_rotate_no,
        "partial_rotate_yes": partial_rotate_yes,
        "z_offset_rotate_yes": z_offset_rotate_yes,
    });

    let axis = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance)?;
    let center = Point3::try_new(0.0, 0.0, 0.0)?;
    let source_points = [
        Point3::try_new(2.0, 0.0, 0.0)?,
        Point3::try_new(4.0, 1.0, 0.0)?,
    ];
    let transforms = (1..4)
        .map(|copy_index| {
            AffineTransform3::try_rotation(center, axis, (90.0 * copy_index as f64).to_radians())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (_, elapsed_ns) = measure(iterations, || {
        transforms
            .iter()
            .flat_map(|transform| {
                source_points
                    .iter()
                    .map(|source| transform.transform_point(*source))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok((value, elapsed_ns))
}

fn document_polar_array_scenario(
    document: &mut Document,
    registry: &CommandRegistry,
    label: &str,
    command: &str,
) -> Result<Value, ProbeError> {
    let layer = document.current_layer_id();
    let source_lines = [
        ([2.0, 0.0, 0.0], [4.0, 1.0, 0.0]),
        ([1.0, -1.0, 2.0], [2.0, -0.5, 3.0]),
    ];
    let mut original_ids = Vec::with_capacity(source_lines.len());
    for (index, (start, end)) in source_lines.into_iter().enumerate() {
        original_ids.push(document.add_geometry_with_attributes(
            Geometry::Line(LineSegment::try_new(
                Point3::try_from(start)?,
                Point3::try_from(end)?,
                document.tolerance(),
            )?),
            ObjectAttributes::on_layer(layer).with_name(format!("{label} {index}")),
        )?);
    }
    document.add_group(
        Some(format!("Viboceros Polar Array Group {label}")),
        original_ids.iter().copied(),
    )?;
    document.select_object(original_ids[0], SelectionMode::Replace)?;
    registry.execute(document, command)?;

    let name_prefix = format!("{label} ");
    let object_ids = document
        .objects()
        .filter(|object| {
            object
                .attributes()
                .name()
                .is_some_and(|name| name.starts_with(&name_prefix))
        })
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let mut objects = object_ids
        .iter()
        .map(|id| array_line_record(document, *id))
        .collect::<Result<Vec<_>, _>>()?;
    objects.sort_by(compare_array_line_records);

    let mut groups = document
        .groups()
        .filter_map(|group| {
            let members = group
                .members()
                .filter(|id| object_ids.contains(id))
                .collect::<Vec<_>>();
            (!members.is_empty()).then_some(members)
        })
        .map(|members| {
            let mut records = members
                .into_iter()
                .map(|id| array_line_record(document, id))
                .collect::<Result<Vec<_>, _>>()?;
            records.sort_by(compare_array_line_records);
            Ok::<_, ProbeError>(records)
        })
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by(|left, right| compare_array_line_record_lists(left, right));

    Ok(json!({
        "command_succeeded": true,
        "groups": groups
            .iter()
            .map(|records| array_line_records_value(records))
            .collect::<Vec<_>>(),
        "objects": array_line_records_value(&objects),
        "originals_selected": original_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>(),
    }))
}

#[derive(Clone)]
struct ArrayLineRecord {
    start: [f64; 3],
    end: [f64; 3],
    name: String,
    selected: bool,
}

fn array_line_record(document: &Document, id: ObjectId) -> Result<ArrayLineRecord, ProbeError> {
    let object = document
        .object(id)
        .ok_or(DocumentError::ObjectNotFound(id))?;
    let Geometry::Line(line) = object.geometry() else {
        return Err(ProbeError::FixtureInvariant(
            "array fixture contains a non-line object",
        ));
    };
    Ok(ArrayLineRecord {
        start: line.start().to_array(),
        end: line.end().to_array(),
        name: object.attributes().name().unwrap_or_default().to_owned(),
        selected: document.is_selected(id),
    })
}

fn compare_array_line_records(
    left: &ArrayLineRecord,
    right: &ArrayLineRecord,
) -> std::cmp::Ordering {
    compare_array_sort_point(&left.start, &right.start)
        .then_with(|| compare_array_sort_point(&left.end, &right.end))
        .then_with(|| left.name.cmp(&right.name))
}

fn compare_array_sort_point(left: &[f64; 3], right: &[f64; 3]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let left = (left * 1.0e12).round();
            let left = if left == 0.0 { 0.0 } else { left };
            let right = (right * 1.0e12).round();
            let right = if right == 0.0 { 0.0 } else { right };
            left.total_cmp(&right)
        })
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn compare_array_line_record_lists(
    left: &[ArrayLineRecord],
    right: &[ArrayLineRecord],
) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_array_line_records(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn array_line_records_value(records: &[ArrayLineRecord]) -> Value {
    Value::Array(
        records
            .iter()
            .map(|record| {
                json!({
                    "end": record.end,
                    "name": record.name,
                    "selected": record.selected,
                    "start": record.start,
                })
            })
            .collect(),
    )
}

fn sorted_document_point_records(
    document: &Document,
) -> Result<Vec<(ObjectId, [f64; 3], String)>, ProbeError> {
    let mut records = Vec::with_capacity(document.objects().len());
    for object in document.objects() {
        let Geometry::Point(point) = object.geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "array fixture contains a non-point object",
            ));
        };
        records.push((
            object.id(),
            point.to_array(),
            object.attributes().name().unwrap_or_default().to_owned(),
        ));
    }
    records
        .sort_by(|left, right| compare_point(&left.1, &right.1).then_with(|| left.2.cmp(&right.2)));
    Ok(records)
}

fn document_group_point_locations(document: &Document) -> Result<Vec<Vec<[f64; 3]>>, ProbeError> {
    let mut groups = Vec::with_capacity(document.groups().len());
    for group in document.groups() {
        let mut points = Vec::with_capacity(group.members().len());
        for id in group.members() {
            let object = document
                .object(id)
                .ok_or(DocumentError::ObjectNotFound(id))?;
            let Geometry::Point(point) = object.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "array group contains a non-point object",
                ));
            };
            points.push(point.to_array());
        }
        points.sort_by(compare_point);
        groups.push(points);
    }
    groups.sort_by(|left, right| compare_point_lists(left, right));
    Ok(groups)
}

struct OracleTemporaryFile {
    path: PathBuf,
}

impl OracleTemporaryFile {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            path: std::env::temp_dir().join(format!(
                "viboceros-oracle-{}-{nonce}-{label}.3dm",
                std::process::id()
            )),
        }
    }
}

impl Drop for OracleTemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn three_dm_group_round_trip(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let path = OracleTemporaryFile::new("groups");
    let objects = [
        (
            "P0",
            [0.0, 0.0, 0.0],
            vec![0],
            [12, 34, 56],
            ThreeDmColorSource::Object,
        ),
        (
            "P1",
            [1.0, 0.0, 0.0],
            vec![0, 1],
            [23, 45, 67],
            ThreeDmColorSource::Layer,
        ),
        (
            "P2",
            [2.0, 0.0, 0.0],
            vec![1],
            [34, 56, 78],
            ThreeDmColorSource::Material,
        ),
        (
            "P3",
            [3.0, 0.0, 0.0],
            Vec::new(),
            [45, 67, 89],
            ThreeDmColorSource::Parent,
        ),
    ]
    .into_iter()
    .map(
        |(name, location, group_indices, object_color, color_source)| {
            Ok::<_, GeometryError>(ThreeDmObject {
                geometry: ThreeDmGeometry::Point(point(location)?),
                layer_index: 0,
                name: Some(name.to_owned()),
                visible: true,
                locked: false,
                object_color,
                color_source,
                group_indices,
            })
        },
    )
    .collect::<Result<Vec<_>, _>>()?;
    let source = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".to_owned(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![
            ThreeDmGroup {
                name: "Assembly α".to_owned(),
            },
            ThreeDmGroup {
                name: "Inspection".to_owned(),
            },
            ThreeDmGroup {
                name: "Empty Group".to_owned(),
            },
        ],
        objects,
    );
    write_3dm_file(&path.path, &source)?;
    let decoded = read_3dm_file(&path.path, tolerance)?;

    let group_names = decoded
        .groups
        .iter()
        .map(|group| group.name.clone())
        .collect::<Vec<_>>();
    let group_members = (0..decoded.groups.len())
        .map(|group_index| {
            decoded
                .objects
                .iter()
                .enumerate()
                .filter_map(|(object_index, object)| {
                    object
                        .group_indices
                        .contains(&group_index)
                        .then_some(object_index)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let object_groups = decoded
        .objects
        .iter()
        .map(|object| {
            object
                .group_indices
                .iter()
                .map(|index| decoded.groups[*index].name.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let object_colors = decoded
        .objects
        .iter()
        .map(|object| object.object_color)
        .collect::<Vec<_>>();
    let color_sources = decoded
        .objects
        .iter()
        .map(|object| match object.color_source {
            ThreeDmColorSource::Layer => "layer",
            ThreeDmColorSource::Object => "object",
            ThreeDmColorSource::Material => "material",
            ThreeDmColorSource::Parent => "parent",
        })
        .collect::<Vec<_>>();
    let value = json!({
        "color_sources": color_sources,
        "group_members": group_members,
        "group_names": group_names,
        "object_colors": object_colors,
        "object_groups": object_groups,
        "unsupported_object_count": decoded.unsupported_object_count(),
    });

    let (_, elapsed_ns) = measure(iterations, || {
        Ok(decoded
            .objects
            .iter()
            .map(|object| object.group_indices.len())
            .sum::<usize>())
    })?;
    Ok((value, elapsed_ns))
}

#[derive(Clone, Copy)]
struct PointCloudCycleIds {
    line: ObjectId,
    mesh: ObjectId,
    cloud: ObjectId,
    point: ObjectId,
    default_layer: LayerId,
}

fn document_point_cloud_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let (mut document, ids) = point_cloud_cycle_document(tolerance)?;

    let cloud_to_cloud_input =
        run_point_cloud_extract(&registry, &mut document, ids, [ids.cloud], "Input")?;
    let line_mesh_cloud_current = run_point_cloud_extract(
        &registry,
        &mut document,
        ids,
        [ids.line, ids.mesh, ids.cloud],
        "Current",
    )?;
    let mesh_line_cloud_input =
        run_point_cloud_extract(&registry, &mut document, ids, [ids.mesh, ids.line], "Input")?;

    document.clear_selection();
    registry.execute(&mut document, "SelPt")?;
    let sel_pt = point_cloud_source_selection(&document, ids);
    document.clear_selection();
    registry.execute(&mut document, "SelPtCloud")?;
    let sel_pt_cloud = point_cloud_source_selection(&document, ids);

    document.clear_selection();
    document.select_objects_direct([ids.cloud], SelectionMode::Replace)?;
    let before_explode = document
        .objects()
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    registry.execute(&mut document, "Explode")?;
    let explode = describe_point_cloud_cycle_objects(
        &document,
        document
            .objects()
            .filter(|object| !before_explode.contains(&object.id()))
            .map(|object| object.id()),
        ids.default_layer,
    );
    let equality_base = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
    let geometry_equals_delta = [
        1.0e-16, 1.0e-15, 1.0e-14, 1.0e-13, 1.0e-12, 1.0e-11, 1.0e-10, 1.0e-9, 1.0e-8, 1.0e-7,
    ]
    .into_iter()
    .map(|delta| {
        point_clouds_geometrically_equal(
            &equality_base,
            &[[1.0 + delta, 2.0, 3.0], [4.0, 5.0, 6.0]],
        )
    })
    .collect::<Result<Vec<_>, _>>()?;
    let geometry_equals_reversed =
        point_clouds_geometrically_equal(&equality_base, &[[4.0, 5.0, 6.0], [1.0, 2.0, 3.0]])?;
    let geometry_equals_relative_delta = [1.0, 1.0e3, 1.0e6, 1.0e9]
        .into_iter()
        .map(|scale| {
            point_clouds_geometrically_equal(
                &[[scale, 0.0, 0.0], [0.0, scale, 0.0]],
                &[[scale * (1.0 + 1.0e-10), 0.0, 0.0], [0.0, scale, 0.0]],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = json!({
        "cloud_to_cloud_input": cloud_to_cloud_input,
        "explode": explode,
        "explode_source_exists": document.object(ids.cloud).is_some(),
        "explode_succeeded": true,
        "geometry_equals_delta": geometry_equals_delta,
        "geometry_equals_relative_delta": geometry_equals_relative_delta,
        "geometry_equals_reversed": geometry_equals_reversed,
        "line_mesh_cloud_current": line_mesh_cloud_current,
        "mesh_line_cloud_input": mesh_line_cloud_input,
        "sel_pt": sel_pt,
        "sel_pt_cloud": sel_pt_cloud,
        "sel_pt_cloud_succeeded": true,
        "sel_pt_succeeded": true,
    });

    let query_cloud = PointCloud3::try_new(
        (0..4096)
            .map(|index| Point3::try_new((index % 64) as f64, (index / 64) as f64, 0.0))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let query = Point3::try_new(31.25, 27.75, 0.0)?;
    let (_nearest, elapsed_ns) = measure(iterations, || query_cloud.nearest_xy(query, 100.0))?;
    Ok((value, elapsed_ns))
}

fn point_clouds_geometrically_equal(
    left: &[[f64; 3]],
    right: &[[f64; 3]],
) -> Result<bool, ProbeError> {
    let cloud = |points: &[[f64; 3]]| {
        Ok::<_, GeometryError>(Geometry::PointCloud(PointCloud3::try_new(
            points
                .iter()
                .copied()
                .map(point)
                .collect::<Result<Vec<_>, _>>()?,
        )?))
    };
    Ok(cloud(left)?.geometrically_equals(&cloud(right)?)?)
}

fn point_cloud_cycle_document(
    tolerance: Tolerance,
) -> Result<(Document, PointCloudCycleIds), ProbeError> {
    let mut document = Document::new(tolerance);
    let default_layer = document.current_layer_id();
    let layer_a = document.add_layer("A", ColorRgb::new(10, 20, 30))?;
    let layer_b = document.add_layer("B", ColorRgb::new(40, 50, 60))?;
    let line = document.add_geometry_with_attributes(
        Geometry::Line(LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0)?,
            Point3::try_new(2.0, 0.0, 0.0)?,
            tolerance,
        )?),
        ObjectAttributes::on_layer(layer_a).with_name("LineSource"),
    )?;
    let mesh = document.add_geometry_with_attributes(
        Geometry::Mesh(TriangleMesh::try_new(
            vec![
                Point3::try_new(10.0, 0.0, 0.0)?,
                Point3::try_new(12.0, 0.0, 0.0)?,
                Point3::try_new(10.0, 2.0, 0.0)?,
            ],
            vec![[0, 1, 2]],
            tolerance,
        )?),
        ObjectAttributes::on_layer(layer_b).with_name("MeshSource"),
    )?;
    let cloud = document.add_geometry_with_attributes(
        Geometry::PointCloud(PointCloud3::try_new(vec![
            Point3::try_new(20.0, 0.0, 0.0)?,
            Point3::try_new(21.0, 1.0, 0.0)?,
            Point3::try_new(22.0, 0.0, 0.0)?,
        ])?),
        ObjectAttributes::on_layer(layer_a).with_name("CloudSource"),
    )?;
    let point = document.add_geometry_with_attributes(
        Geometry::Point(Point3::try_new(30.0, 0.0, 0.0)?),
        ObjectAttributes::on_layer(layer_b).with_name("PointSource"),
    )?;
    Ok((
        document,
        PointCloudCycleIds {
            line,
            mesh,
            cloud,
            point,
            default_layer,
        },
    ))
}

fn run_point_cloud_extract<const N: usize>(
    registry: &CommandRegistry,
    document: &mut Document,
    source_ids: PointCloudCycleIds,
    selection: [ObjectId; N],
    output_layer: &str,
) -> Result<Value, ProbeError> {
    document.clear_selection();
    for (index, id) in selection.into_iter().enumerate() {
        document.select_objects_direct(
            [id],
            if index == 0 {
                SelectionMode::Replace
            } else {
                SelectionMode::Add
            },
        )?;
    }
    let before = document
        .objects()
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    registry.execute(
        document,
        &format!("ExtractPt Output=PointCloud OutputLayer={output_layer}"),
    )?;
    let objects = describe_point_cloud_cycle_objects(
        document,
        document
            .objects()
            .filter(|object| !before.contains(&object.id()))
            .map(|object| object.id()),
        source_ids.default_layer,
    );
    let result = json!({
        "objects": objects,
        "source_selection": point_cloud_source_selection(document, source_ids),
        "succeeded": true,
    });
    document.undo()?.ok_or(DocumentError::HistoryInvariant(
        "point-cloud extraction did not create an undo entry",
    ))?;
    Ok(result)
}

fn describe_point_cloud_cycle_objects(
    document: &Document,
    ids: impl IntoIterator<Item = ObjectId>,
    default_layer: LayerId,
) -> Vec<Value> {
    ids.into_iter()
        .filter_map(|id| document.object(id))
        .map(|object| {
            let (geometry_type, points) = match object.geometry() {
                Geometry::Point(point) => ("point", vec![point.to_array()]),
                Geometry::PointCloud(cloud) => (
                    "point_cloud",
                    cloud
                        .points()
                        .iter()
                        .map(|point| point.to_array())
                        .collect(),
                ),
                _ => ("unexpected", Vec::new()),
            };
            let layer = if object.attributes().layer_id() == default_layer {
                "Current"
            } else {
                document
                    .layer(object.attributes().layer_id())
                    .map_or("Unexpected", |layer| layer.name())
            };
            json!({
                "layer": layer,
                "name": object.attributes().name(),
                "points": points,
                "selected": document.is_selected(object.id()),
                "type": geometry_type,
            })
        })
        .collect()
}

fn point_cloud_source_selection(document: &Document, ids: PointCloudCycleIds) -> Vec<&'static str> {
    [
        ("line", ids.line),
        ("mesh", ids.mesh),
        ("cloud", ids.cloud),
        ("point", ids.point),
    ]
    .into_iter()
    .filter_map(|(label, id)| document.is_selected(id).then_some(label))
    .collect()
}

fn document_duplicate_selection_cycle(
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let mut document = Document::new(tolerance);
    let layer = document.current_layer_id();
    let mut object_ids = Vec::new();
    let mut add = |document: &mut Document,
                   geometry: Geometry,
                   attributes: ObjectAttributes|
     -> Result<ObjectId, DocumentError> {
        let id = document.add_geometry_with_attributes(geometry, attributes)?;
        object_ids.push(id);
        Ok(id)
    };
    let ordinary = || ObjectAttributes::on_layer(layer);

    let unrelated = add(
        &mut document,
        Geometry::Point(Point3::try_new(30.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    let point_original = add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    let point_duplicate = add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary().with_name("Different attributes"),
    )?;
    add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary().with_visibility(false),
    )?;
    add(
        &mut document,
        Geometry::Point(Point3::try_new(0.0, 0.0, 0.0)?),
        ordinary().with_locked(true),
    )?;
    let point_near = add(
        &mut document,
        Geometry::Point(Point3::try_new(tolerance.absolute() * 0.5, 0.0, 0.0)?),
        ordinary(),
    )?;
    let group_peer = add(
        &mut document,
        Geometry::Point(Point3::try_new(20.0, 0.0, 0.0)?),
        ordinary(),
    )?;
    document.add_group(
        Some("Duplicate probe group".to_owned()),
        [point_duplicate, group_peer],
    )?;

    let line_start = Point3::try_new(0.0, 10.0, 0.0)?;
    let line_end = Point3::try_new(5.0, 10.0, 0.0)?;
    let line_original = add(
        &mut document,
        Geometry::Line(LineSegment::try_new(line_start, line_end, tolerance)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Line(LineSegment::try_new(line_start, line_end, tolerance)?),
        ordinary(),
    )?;
    let line_reversed = add(
        &mut document,
        Geometry::Line(LineSegment::try_new(line_end, line_start, tolerance)?),
        ordinary(),
    )?;
    let line_nurbs = add(
        &mut document,
        Geometry::NurbsCurve(NurbsCurve::try_new(
            1,
            vec![line_start, line_end],
            vec![0.0, 0.0, 1.0, 1.0],
        )?),
        ordinary(),
    )?;
    let line_near = add(
        &mut document,
        Geometry::Line(LineSegment::try_new(
            line_start,
            Point3::try_new(5.0 + tolerance.absolute() * 0.5, 10.0, 0.0)?,
            tolerance,
        )?),
        ordinary(),
    )?;

    let open_vertices = vec![
        Point3::try_new(0.0, 20.0, 0.0)?,
        Point3::try_new(2.0, 20.0, 0.0)?,
        Point3::try_new(2.0, 22.0, 0.0)?,
    ];
    let open_polyline = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(open_vertices.clone(), tolerance)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(open_vertices.clone(), tolerance)?),
        ordinary(),
    )?;
    let mut reversed_open_vertices = open_vertices.clone();
    reversed_open_vertices.reverse();
    let open_polyline_reversed = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(reversed_open_vertices, tolerance)?),
        ordinary(),
    )?;

    let closed_vertices = vec![
        Point3::try_new(10.0, 20.0, 0.0)?,
        Point3::try_new(12.0, 20.0, 0.0)?,
        Point3::try_new(12.0, 22.0, 0.0)?,
        Point3::try_new(10.0, 20.0, 0.0)?,
    ];
    let closed_polyline = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(closed_vertices.clone(), tolerance)?),
        ordinary(),
    )?;
    let shifted_closed_polyline = add(
        &mut document,
        Geometry::Polyline(Polyline3::try_new(
            vec![
                closed_vertices[1],
                closed_vertices[2],
                closed_vertices[0],
                closed_vertices[1],
            ],
            tolerance,
        )?),
        ordinary(),
    )?;

    let up = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance)?;
    let circle_center = Point3::try_new(0.0, 30.0, 0.0)?;
    let circle_original = add(
        &mut document,
        Geometry::Circle(Circle3::try_new(circle_center, 3.0, up, tolerance)?),
        ordinary(),
    )?;
    add(
        &mut document,
        Geometry::Circle(Circle3::try_new(circle_center, 3.0, up, tolerance)?),
        ordinary(),
    )?;
    let circle_opposite = add(
        &mut document,
        Geometry::Circle(Circle3::try_new(
            circle_center,
            3.0,
            up.opposite(),
            tolerance,
        )?),
        ordinary(),
    )?;

    let mesh_vertices = vec![
        Point3::try_new(0.0, 40.0, 0.0)?,
        Point3::try_new(2.0, 40.0, 0.0)?,
        Point3::try_new(0.0, 42.0, 0.0)?,
    ];
    let mesh = TriangleMesh::try_new(mesh_vertices.clone(), vec![[0, 1, 2]], tolerance)?;
    let mesh_original = add(&mut document, Geometry::Mesh(mesh.clone()), ordinary())?;
    add(&mut document, Geometry::Mesh(mesh.clone()), ordinary())?;
    let mesh_reversed = add(&mut document, Geometry::Mesh(mesh.reversed()), ordinary())?;
    let mesh_reindexed = add(
        &mut document,
        Geometry::Mesh(TriangleMesh::try_new(
            vec![mesh_vertices[1], mesh_vertices[2], mesh_vertices[0]],
            vec![[2, 0, 1]],
            tolerance,
        )?),
        ordinary(),
    )?;

    measure_document(iterations, || {
        document.clear_selection();
        document.select_objects_direct([unrelated], SelectionMode::Replace)?;
        let all_count = document.select_duplicate_objects(true)?;
        let all = document_selected_indices(&document, &object_ids);

        document.clear_selection();
        document.select_objects_direct([unrelated], SelectionMode::Replace)?;
        let without_original_count = document.select_duplicate_objects(false)?;

        let equal = |left: ObjectId, right: ObjectId| -> Result<bool, DocumentError> {
            let left = document
                .object(left)
                .ok_or(DocumentError::ObjectNotFound(left))?;
            let right = document
                .object(right)
                .ok_or(DocumentError::ObjectNotFound(right))?;
            Ok(left.geometry().geometrically_equals(right.geometry())?)
        };
        Ok(json!({
            "all": all,
            "all_count": all_count,
            "circle_opposite_equal": equal(circle_original, circle_opposite)?,
            "closed_shifted_equal": equal(closed_polyline, shifted_closed_polyline)?,
            "line_nurbs_equal": equal(line_original, line_nurbs)?,
            "line_near_equal": equal(line_original, line_near)?,
            "line_reversed_equal": equal(line_original, line_reversed)?,
            "mesh_reindexed_equal": equal(mesh_original, mesh_reindexed)?,
            "mesh_reversed_equal": equal(mesh_original, mesh_reversed)?,
            "point_near_equal": equal(point_original, point_near)?,
            "polyline_reversed_equal": equal(open_polyline, open_polyline_reversed)?,
            "without_original_count": without_original_count,
        }))
    })
}

fn document_object_names(
    document: &Document,
    object_ids: &[ObjectId],
) -> Result<Vec<Option<String>>, DocumentError> {
    object_ids
        .iter()
        .map(|id| {
            document
                .object(*id)
                .map(|object| object.attributes().name().map(str::to_owned))
                .ok_or(DocumentError::ObjectNotFound(*id))
        })
        .collect()
}

fn document_layer_names(
    document: &Document,
    object_ids: &[ObjectId],
) -> Result<Vec<String>, DocumentError> {
    object_ids
        .iter()
        .map(|id| {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            document
                .layer(object.attributes().layer_id())
                .map(|layer| layer.name().to_owned())
                .ok_or(DocumentError::LayerNotFound(object.attributes().layer_id()))
        })
        .collect()
}

fn document_group_sizes_for(document: &Document, object_ids: &[ObjectId]) -> Vec<usize> {
    let object_ids = object_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut sizes = document
        .groups()
        .filter_map(|group| {
            let size = group
                .members()
                .filter(|member| object_ids.contains(member))
                .count();
            (size > 0).then_some(size)
        })
        .collect::<Vec<_>>();
    sizes.sort_unstable();
    sizes
}

fn undo_required(document: &mut Document) -> Result<(), DocumentError> {
    if document.undo()?.is_some() {
        Ok(())
    } else {
        Err(DocumentError::HistoryInvariant(
            "oracle layer-assignment edit was missing",
        ))
    }
}

fn establish_previous_selection(
    document: &mut Document,
    object_ids: &[ObjectId],
) -> Result<(), DocumentError> {
    document.select_objects([object_ids[0], object_ids[1]], SelectionMode::Replace)?;
    document.clear_selection();
    document.select_object(object_ids[2], SelectionMode::Add)?;
    Ok(())
}

fn document_selected_indices(document: &Document, object_ids: &[ObjectId]) -> Vec<usize> {
    object_ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
        .collect()
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

fn compare_point_lists(left: &[[f64; 3]], right: &[[f64; 3]]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_point(left, right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
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
    fn isolates_only_ordinary_unselected_objects_in_rhino_scope() {
        let response = run_request(&request(vec![Operation::DocumentObjectIsolationCycle {
            id: "object-isolation".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "after_isolate": [
                    "normal", "hidden", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "after_isolate_lock": [
                    "normal", "locked", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "after_unisolate": [
                    "normal", "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "after_unisolate_lock": [
                    "normal", "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                    "normal", "hidden", "locked",
                ],
                "isolate_count": 1,
                "isolate_lock_count": 1,
                "isolate_lock_repeat_count": 0,
                "isolate_repeat_count": 0,
                "labels": [
                    "default-selected", "default-normal", "default-hidden", "default-locked",
                    "hidden-layer-normal", "hidden-layer-hidden", "hidden-layer-locked",
                    "locked-layer-normal", "locked-layer-hidden", "locked-layer-locked",
                ],
                "selected_after_isolate": 1,
                "selected_after_isolate_lock": 1,
                "selected_after_unisolate": 1,
                "selected_after_unisolate_lock": 1,
                "unisolate_count": 1,
                "unisolate_lock_count": 1,
                "unisolate_lock_repeat_count": 0,
                "unisolate_repeat_count": 0,
            })
        );
    }

    #[test]
    fn selects_last_and_previous_objects_with_rhino_defaults() {
        let response = run_request(&request(vec![Operation::DocumentActionSelectionCycle {
            id: "action-selection".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "batch_last": [0, 1],
                "batch_last_count": 2,
                "last_add": [0, 3],
                "last_add_count": 2,
                "last_default": [3],
                "last_default_count": 1,
                "previous_add": [0, 1, 2],
                "previous_add_count": 3,
                "previous_default": [0, 1],
                "previous_default_count": 2,
                "previous_default_twice": [2],
                "previous_default_twice_count": 1,
                "previous_once": [0],
                "previous_once_count": 1,
                "previous_twice": [3],
                "previous_twice_count": 1,
            })
        );
    }

    #[test]
    fn selects_attributes_without_group_expansion_and_activates_layers() {
        let response = run_request(&request(vec![Operation::DocumentAttributeSelectionCycle {
            id: "attribute-selection".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "all_layers": [0, 1, 2, 3, 4, 7, 9],
                "all_layers_count": 7,
                "color": [0, 7, 9],
                "color_count": 3,
                "group_lower": [0, 1, 2, 4],
                "group_lower_count": 4,
                "group_upper": [0, 1, 4],
                "group_upper_count": 3,
                "group_wrong_case": [0, 1, 2, 4],
                "group_wrong_case_count": 4,
                "hidden_layer": [0, 7],
                "hidden_layer_count": 2,
                "hidden_layer_visible": true,
                "locked_layer": [0, 7, 9],
                "locked_layer_count": 3,
                "locked_layer_locked": false,
                "name": [0, 1, 2],
                "name_count": 3,
            })
        );
    }

    #[test]
    fn assigns_shared_countered_and_cleared_object_names() {
        let response = run_request(&request(vec![Operation::DocumentObjectNamingCycle {
            id: "object-naming".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "clear_count": 3,
                "cleared": [null, null, null],
                "counter": ["Sample 0", "Sample 1", "Sample 2"],
                "counter_count": 3,
                "shared": ["Sample", "Sample", "Sample"],
                "shared_count": 3,
            })
        );
    }

    #[test]
    fn moves_and_copies_objects_between_layers_with_rhino_scope() {
        let response = run_request(&request(vec![Operation::DocumentLayerAssignmentCycle {
            id: "layer-assignment".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "change_count": 2,
                "change_group_sizes": [2],
                "change_layers": ["Normal", "Normal"],
                "change_selected": [0, 1],
                "copy_count": 2,
                "copy_group_sizes": [2],
                "copy_layers": ["Normal", "Normal"],
                "copy_names": ["Part0", "Part1"],
                "copy_selected": [],
                "current_after_change": "Default",
                "current_unchanged": true,
                "hidden_change_count": 1,
                "hidden_change_selected": [],
                "hidden_copy_count": 1,
                "hidden_copy_layers": ["Hidden"],
                "hidden_copy_selected": [],
                "locked_change_count": 1,
                "locked_change_selected": [],
                "locked_copy_count": 1,
                "locked_copy_layers": ["Locked"],
                "locked_copy_selected": [],
                "mixed_copy_count": 1,
                "mixed_copy_group_sizes": [1],
                "mixed_copy_layers": ["Normal"],
                "original_selected_after_copy": [0, 1],
                "original_selected_after_destination_copies": [3],
                "same_layer_copy_count": 0,
            })
        );
    }

    #[test]
    fn linearly_arrays_grouped_objects_with_rhino_scope() {
        let response = run_request(&request(vec![Operation::DocumentLinearArrayCycle {
            id: "linear-array".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "command_succeeded": true,
                "groups_after_array": [
                    [[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]],
                    [[3.0, 1.0, 6.0], [6.0, 1.0, 6.0]],
                    [[5.0, 0.0, 9.0], [8.0, 0.0, 9.0]],
                    [[7.0, -1.0, 12.0], [10.0, -1.0, 12.0]],
                ],
                "locations_after_array": [
                    [1.0, 2.0, 3.0],
                    [3.0, 1.0, 6.0],
                    [4.0, 2.0, 3.0],
                    [5.0, 0.0, 9.0],
                    [6.0, 1.0, 6.0],
                    [7.0, -1.0, 12.0],
                    [8.0, 0.0, 9.0],
                    [10.0, -1.0, 12.0],
                ],
                "names_after_array": ["0", "0", "1", "0", "1", "0", "1", "1"],
                "originals_selected_after_array": [0, 1],
                "selected_after_array": [[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]],
            })
        );
    }

    #[test]
    fn rectangularly_arrays_grouped_objects_with_rhino_unit_cell_and_fill_scope() {
        let response = run_request(&request(vec![Operation::DocumentRectangularArrayCycle {
            id: "rectangular-array".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for (scenario, object_count, group_count) in [("fill", 12, 6), ("unit_cell", 24, 12)] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(
                value[scenario]["locations_after_array"]
                    .as_array()
                    .unwrap()
                    .len(),
                object_count
            );
            assert_eq!(
                value[scenario]["groups_after_array"]
                    .as_array()
                    .unwrap()
                    .len(),
                group_count
            );
            assert_eq!(
                value[scenario]["originals_selected_after_array"],
                json!([0, 1])
            );
            assert_eq!(
                value[scenario]["selected_after_array"],
                json!([[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]])
            );
        }
        assert!(
            value["fill"]["locations_after_array"]
                .as_array()
                .unwrap()
                .contains(&json!([11.0, -4.0, 3.0]))
        );
        assert!(
            value["unit_cell"]["locations_after_array"]
                .as_array()
                .unwrap()
                .contains(&json!([8.0, 1.0, 7.0]))
        );
    }

    #[test]
    fn arrays_grouped_triads_along_curves_with_rhino_orientation_scope() {
        let response = run_request(&request(vec![Operation::DocumentCurveArrayCycle {
            id: "curve-array".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for (scenario, object_count, group_count) in [
            ("base_point", 15, 5),
            ("freeform", 12, 4),
            ("freeform_nurbs", 15, 5),
            ("no_rotation_distance", 12, 4),
            ("no_rotation_items", 12, 4),
            ("roadlike", 12, 4),
            ("stairlike", 12, 4),
        ] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(
                value[scenario]["objects"].as_array().unwrap().len(),
                object_count
            );
            assert_eq!(
                value[scenario]["groups"].as_array().unwrap().len(),
                group_count
            );
            assert_eq!(value[scenario]["originals_selected"], json!([0, 1, 2]));
            assert_eq!(value[scenario]["path_selected"], json!(false));
            assert_eq!(
                value[scenario]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|record| record["selected"] == json!(true))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn orients_grouped_triads_with_rhino_scale_copy_and_frame_scope() {
        let response = run_request(&request(vec![Operation::DocumentOrientCycle {
            id: "orient".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for (scenario, object_count, group_count) in [
            ("orient_default", 3, 1),
            ("orient_copy_no", 6, 2),
            ("orient_copy_1d", 6, 2),
            ("orient_copy_3d", 6, 2),
            ("orient_spatial", 6, 2),
            ("orient3_default", 3, 1),
            ("orient3_copy_scale", 6, 2),
        ] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(
                value[scenario]["objects"].as_array().unwrap().len(),
                object_count
            );
            assert_eq!(
                value[scenario]["groups"].as_array().unwrap().len(),
                group_count
            );
            assert_eq!(value[scenario]["originals_selected"], json!([0, 1, 2]));
            assert_eq!(
                value[scenario]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|record| record["selected"] == json!(true))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn polar_arrays_grouped_lines_with_rhino_sweep_and_option_scope() {
        let response = run_request(&request(vec![Operation::DocumentPolarArrayCycle {
            id: "polar-array".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        for scenario in [
            "full_rotate_yes",
            "negative_full_rotate_yes",
            "multi_turn_z_offset_rotate_yes",
            "partial_rotate_no",
            "partial_rotate_yes",
            "z_offset_rotate_yes",
        ] {
            assert_eq!(value[scenario]["command_succeeded"], json!(true));
            assert_eq!(value[scenario]["groups"].as_array().unwrap().len(), 4);
            assert_eq!(value[scenario]["objects"].as_array().unwrap().len(), 8);
            assert_eq!(value[scenario]["originals_selected"], json!([0, 1]));
            assert_eq!(
                value[scenario]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|record| record["selected"] == json!(true))
                    .count(),
                2
            );
        }

        let multi_turn_objects = value["multi_turn_z_offset_rotate_yes"]["objects"]
            .as_array()
            .unwrap();
        assert!(multi_turn_objects.iter().any(|record| {
            let start = record["start"].as_array().unwrap();
            let end = record["end"].as_array().unwrap();
            [([2.0, 0.0, 6.0], start), ([4.0, 1.0, 6.0], end)]
                .into_iter()
                .all(|(expected, actual)| {
                    expected.into_iter().zip(actual).all(|(expected, actual)| {
                        Tolerance::DEFAULT.approx_eq(expected, actual.as_f64().unwrap())
                    })
                })
                && record["selected"] == json!(false)
        }));
        let no_rotate_objects = value["partial_rotate_no"]["objects"].as_array().unwrap();
        assert!(no_rotate_objects.iter().all(|record| {
            let start = record["start"].as_array().unwrap();
            let end = record["end"].as_array().unwrap();
            let name = record["name"].as_str().unwrap();
            let expected = if name.ends_with(" 0") {
                [2.0, 1.0, 0.0]
            } else {
                [1.0, 0.5, 1.0]
            };
            start
                .iter()
                .zip(end)
                .zip(expected)
                .all(|((start, end), expected)| {
                    Tolerance::DEFAULT
                        .approx_eq(end.as_f64().unwrap() - start.as_f64().unwrap(), expected)
                })
        }));
    }

    #[test]
    fn extracts_selects_and_explodes_native_point_clouds() {
        let response = run_request(&request(vec![Operation::DocumentPointCloudCycle {
            id: "point-cloud-cycle".to_owned(),
        }]))
        .unwrap();
        let value = &response.results[0].value;
        assert_eq!(
            value["mesh_line_cloud_input"]["objects"][0],
            json!({
                "layer": "B",
                "name": "MeshSource",
                "points": [
                    [10.0, 0.0, 0.0],
                    [12.0, 0.0, 0.0],
                    [10.0, 2.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                ],
                "selected": true,
                "type": "point_cloud",
            })
        );
        assert_eq!(
            value["line_mesh_cloud_current"]["objects"][0]["layer"],
            json!("Current")
        );
        assert_eq!(
            value["line_mesh_cloud_current"]["objects"][0]["name"],
            Value::Null
        );
        assert_eq!(value["sel_pt"], json!(["point"]));
        assert_eq!(value["sel_pt_cloud"], json!(["cloud"]));
        assert_eq!(
            value["geometry_equals_delta"],
            json!([true, true, true, true, true, true, true, true, true, false])
        );
        assert_eq!(
            value["geometry_equals_relative_delta"],
            json!([true, true, true, true])
        );
        assert_eq!(value["geometry_equals_reversed"], json!(false));
        assert_eq!(value["explode_source_exists"], json!(false));
        assert_eq!(value["explode"].as_array().unwrap().len(), 3);
        assert!(
            value["explode"]
                .as_array()
                .unwrap()
                .iter()
                .all(|point| point["selected"] == json!(false))
        );
    }

    #[test]
    fn round_trips_overlapping_and_empty_three_dm_groups() {
        let response = run_request(&request(vec![Operation::ThreeDmGroupRoundTrip {
            id: "three-dm-groups".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "color_sources": ["object", "layer", "material", "parent"],
                "group_members": [[0, 1], [1, 2], []],
                "group_names": ["Assembly α", "Inspection", "Empty Group"],
                "object_colors": [
                    [12, 34, 56],
                    [23, 45, 67],
                    [34, 56, 78],
                    [45, 67, 89],
                ],
                "object_groups": [
                    ["Assembly α"],
                    ["Assembly α", "Inspection"],
                    ["Inspection"],
                    [],
                ],
                "unsupported_object_count": 0,
            })
        );
    }

    #[test]
    fn selects_duplicate_geometry_without_attributes_or_groups() {
        let response = run_request(&request(vec![Operation::DocumentDuplicateSelectionCycle {
            id: "duplicate-selection".to_owned(),
        }]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "all": [0, 1, 2, 3, 8, 9, 10, 11, 12, 13, 14, 15, 18, 19, 20, 21, 22],
                "all_count": 17,
                "circle_opposite_equal": true,
                "closed_shifted_equal": false,
                "line_near_equal": true,
                "line_nurbs_equal": true,
                "line_reversed_equal": true,
                "mesh_reindexed_equal": false,
                "mesh_reversed_equal": false,
                "point_near_equal": false,
                "polyline_reversed_equal": true,
                "without_original_count": 12,
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
    fn filters_nurbs_curves_at_an_inclusive_maximum_length() {
        let operation = |id: &str, end_x, maximum_length| Operation::NurbsCurveShortFilter {
            id: id.to_owned(),
            degree: 1,
            control_points: vec![
                control([0.0, 0.0, 0.0], 1.0),
                control([end_x, 0.0, 0.0], 1.0),
            ],
            knots: vec![0.0, 0.0, 1.0, 1.0],
            maximum_length,
        };
        let response = run_request(&request(vec![
            operation("boundary", 1.0, 1.0),
            operation("long", 1.5, 1.0),
        ]))
        .unwrap();
        assert_eq!(response.results[0].value, json!(true));
        assert_eq!(response.results[1].value, json!(false));

        let error = run_request(&request(vec![operation("invalid", 1.0, 0.0)])).unwrap_err();
        assert!(matches!(
            error,
            ProbeError::InvalidMaximumCurveLength(length) if length == 0.0
        ));
    }

    #[test]
    fn classifies_nurbs_curves_for_shape_selection() {
        let operation = |id: &str, degree, points: Vec<[f64; 3]>, knots: Vec<f64>| {
            Operation::NurbsCurveClassification {
                id: id.to_owned(),
                degree,
                control_points: points
                    .into_iter()
                    .map(|point| control(point, 1.0))
                    .collect(),
                knots,
            }
        };
        let single_knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let response = run_request(&request(vec![
            operation(
                "single",
                3,
                vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [3.0, 0.0, 0.0],
                ],
                single_knots.clone(),
            ),
            operation(
                "multi",
                3,
                vec![
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [2.0, 1.0, 0.0],
                    [3.0, 1.0, 0.0],
                    [4.0, 1.0, 0.0],
                ],
                vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            ),
            operation(
                "near",
                3,
                vec![
                    [0.0, 2.0, 0.0],
                    [1.0, 2.000_000_000_5, 0.0],
                    [2.0, 2.000_000_000_5, 0.0],
                    [3.0, 2.0, 0.0],
                ],
                single_knots.clone(),
            ),
            operation(
                "nonplanar",
                3,
                vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 2.0, 0.0],
                    [0.0, 0.0, 2.0],
                ],
                single_knots,
            ),
            operation(
                "degree-one-multi",
                1,
                vec![[0.0, 3.0, 0.0], [0.3, 3.0, 0.0], [0.8, 3.0, 0.0]],
                vec![0.0, 0.0, 0.5, 1.0, 1.0],
            ),
            operation(
                "degree-one-two-controls",
                1,
                vec![[0.0, 4.0, 0.0], [0.6, 4.0, 0.0]],
                vec![0.0, 0.0, 1.0, 1.0],
            ),
        ]))
        .unwrap();
        assert_eq!(
            response.results[0].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": true,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[1].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": false,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[2].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": false,
                "is_planar_model": true,
                "sel_line_match": false,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[3].value,
            json!({
                "is_linear_model": false,
                "is_linear_zero": false,
                "is_planar_model": false,
                "sel_line_match": false,
                "sel_polyline_match": false,
            })
        );
        assert_eq!(
            response.results[4].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": false,
                "sel_polyline_match": true,
            })
        );
        assert_eq!(
            response.results[5].value,
            json!({
                "is_linear_model": true,
                "is_linear_zero": true,
                "is_planar_model": true,
                "sel_line_match": true,
                "sel_polyline_match": false,
            })
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
