//! Extensible command registry and the first model-editing commands.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, GroupId, LayerId, ObjectAttributes,
    ObjectColorSource, ObjectId, SelectionMode, suggested_layer_color,
};
use viboceros_geometry::{
    AffineTransform3, BoundingBox3, Circle3, CircularArc3, ControlPointCurveClosure,
    CurveInterpolationOptions, CurveKnotSpacing, CurveRef, CurveSample, Ellipse3, Frame3,
    GeometryError, InterpolatedCurveClosure, LineSegment, MAX_CURVE_DIVISION_POINTS,
    MAX_REGULAR_POLYGON_SIDES, MeshFaceExtraction, NurbsCurve, NurbsSurface, Plane, Point3,
    PointCloud3, Polyline3, PolylineClosure, Real, SurfacePointMorph, Tolerance, TriangleMesh,
    UnitVector3, Vector3, join_polylines,
};
use viboceros_io::{
    StepError, StlError, StlFormat, ThreeDmColorSource, ThreeDmError, ThreeDmGeometry,
    ThreeDmGroup, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, read_step_file,
    read_stl_file, write_3dm_file, write_step_file, write_stl_file,
};

const SURFACE_EXPORT_SAMPLES_PER_SPAN: usize = 16;
const MAX_EXTRACTED_POINTS: usize = 1_000_000;
const MAX_ARRAY_OBJECTS: usize = 1_000_000;
pub const MAX_CURVE_COMMAND_DEGREE: usize = 11;

pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether successful mutations should be grouped into one undo step.
    fn records_history(&self) -> bool {
        true
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError>;
}

#[derive(Default)]
pub struct CommandRegistry {
    commands: Vec<Box<dyn Command>>,
    lookup: BTreeMap<String, usize>,
}

impl CommandRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        registry
            .register(PointCommand)
            .expect("unique built-in command");
        registry
            .register(LineCommand)
            .expect("unique built-in command");
        registry
            .register(CircleCommand)
            .expect("unique built-in command");
        registry
            .register(ArcCommand)
            .expect("unique built-in command");
        registry
            .register(EllipseCommand)
            .expect("unique built-in command");
        registry
            .register(PolylineCommand)
            .expect("unique built-in command");
        registry
            .register(RectangleCommand)
            .expect("unique built-in command");
        registry
            .register(PolygonCommand)
            .expect("unique built-in command");
        registry
            .register(CurveCommand)
            .expect("unique built-in command");
        registry
            .register(ControlPointCurveCommand)
            .expect("unique built-in command");
        registry
            .register(InterpCurveCommand)
            .expect("unique built-in command");
        registry
            .register(SrfPtCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeCurveCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeCurveAlongCurveCommand)
            .expect("unique built-in command");
        registry
            .register(ExtrudeCurveToPointCommand)
            .expect("unique built-in command");
        registry
            .register(RevolveCommand)
            .expect("unique built-in command");
        registry
            .register(LayerCommand)
            .expect("unique built-in command");
        registry
            .register(ChangeLayerCommand)
            .expect("unique built-in command");
        registry
            .register(CopyToLayerCommand)
            .expect("unique built-in command");
        registry
            .register(SelAllCommand)
            .expect("unique built-in command");
        registry
            .register(SelNoneCommand)
            .expect("unique built-in command");
        registry
            .register(InvertCommand)
            .expect("unique built-in command");
        registry
            .register(SelLastCommand)
            .expect("unique built-in command");
        registry
            .register(SelPrevCommand)
            .expect("unique built-in command");
        registry
            .register(SelNameCommand)
            .expect("unique built-in command");
        registry
            .register(SelColorCommand)
            .expect("unique built-in command");
        registry
            .register(SelLayerCommand)
            .expect("unique built-in command");
        registry
            .register(SelGroupCommand)
            .expect("unique built-in command");
        registry
            .register(SelectDuplicateCommand {
                name: "SelDup",
                include_originals: false,
            })
            .expect("unique built-in command");
        registry
            .register(SelectDuplicateCommand {
                name: "SelDupAll",
                include_originals: true,
            })
            .expect("unique built-in command");
        for (name, filter) in [
            ("SelCrv", GeometrySelectionFilter::Curve),
            ("SelOpenCrv", GeometrySelectionFilter::OpenCurve),
            ("SelClosedCrv", GeometrySelectionFilter::ClosedCurve),
            ("SelPlanarCrv", GeometrySelectionFilter::PlanarCurve),
            ("SelLine", GeometrySelectionFilter::Line),
            ("SelPolyline", GeometrySelectionFilter::Polyline),
            ("SelPt", GeometrySelectionFilter::Point),
            ("SelPtCloud", GeometrySelectionFilter::PointCloud),
            ("SelSrf", GeometrySelectionFilter::Surface),
            ("SelMesh", GeometrySelectionFilter::Mesh),
            ("SelOpenMesh", GeometrySelectionFilter::OpenMesh),
            ("SelClosedMesh", GeometrySelectionFilter::ClosedMesh),
        ] {
            registry
                .register(SelectGeometryCommand { name, filter })
                .expect("unique built-in command");
        }
        registry
            .register(SelShortCurveCommand)
            .expect("unique built-in command");
        registry
            .register(LengthCommand)
            .expect("unique built-in command");
        registry
            .register(AreaCommand)
            .expect("unique built-in command");
        registry
            .register(VolumeCommand)
            .expect("unique built-in command");
        registry
            .register(DivideCommand)
            .expect("unique built-in command");
        registry
            .register(CrvStartCommand)
            .expect("unique built-in command");
        registry
            .register(CrvEndCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractPtCommand)
            .expect("unique built-in command");
        registry
            .register(CloseCrvCommand)
            .expect("unique built-in command");
        registry
            .register(FlipCommand)
            .expect("unique built-in command");
        registry
            .register(UnifyMeshNormalsCommand)
            .expect("unique built-in command");
        registry
            .register(CombineIdenticalMeshVerticesCommand)
            .expect("unique built-in command");
        registry
            .register(CullUnusedMeshVerticesCommand)
            .expect("unique built-in command");
        registry
            .register(SplitDisjointMeshCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractNonManifoldMeshEdgesCommand)
            .expect("unique built-in command");
        registry
            .register(ExtractDuplicateMeshFacesCommand)
            .expect("unique built-in command");
        registry
            .register(GroupCommand)
            .expect("unique built-in command");
        registry
            .register(SetObjectNameCommand)
            .expect("unique built-in command");
        registry
            .register(SetObjectColorCommand)
            .expect("unique built-in command");
        registry
            .register(UngroupCommand)
            .expect("unique built-in command");
        registry
            .register(DeleteCommand)
            .expect("unique built-in command");
        registry
            .register(HideCommand)
            .expect("unique built-in command");
        registry
            .register(ShowCommand)
            .expect("unique built-in command");
        registry
            .register(LockCommand)
            .expect("unique built-in command");
        registry
            .register(UnlockCommand)
            .expect("unique built-in command");
        registry
            .register(HideSwapCommand)
            .expect("unique built-in command");
        registry
            .register(LockSwapCommand)
            .expect("unique built-in command");
        registry
            .register(IsolateCommand)
            .expect("unique built-in command");
        registry
            .register(UnisolateCommand)
            .expect("unique built-in command");
        registry
            .register(IsolateLockCommand)
            .expect("unique built-in command");
        registry
            .register(UnisolateLockCommand)
            .expect("unique built-in command");
        registry
            .register(JoinCommand)
            .expect("unique built-in command");
        registry
            .register(ExplodeCommand)
            .expect("unique built-in command");
        registry
            .register(MoveCommand)
            .expect("unique built-in command");
        registry
            .register(CopyCommand)
            .expect("unique built-in command");
        registry
            .register(OrientCommand)
            .expect("unique built-in command");
        registry
            .register(OrientThreePointCommand)
            .expect("unique built-in command");
        registry
            .register(OrientOnSurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(ArrayCommand)
            .expect("unique built-in command");
        registry
            .register(ArrayCurveCommand)
            .expect("unique built-in command");
        registry
            .register(ArraySurfaceCommand)
            .expect("unique built-in command");
        registry
            .register(ArrayLinearCommand)
            .expect("unique built-in command");
        registry
            .register(ArrayPolarCommand)
            .expect("unique built-in command");
        registry
            .register(ScaleCommand)
            .expect("unique built-in command");
        registry
            .register(ScaleOneDimensionalCommand)
            .expect("unique built-in command");
        registry
            .register(ScaleTwoDimensionalCommand)
            .expect("unique built-in command");
        registry
            .register(ScaleNonUniformCommand)
            .expect("unique built-in command");
        registry
            .register(RotateCommand)
            .expect("unique built-in command");
        registry
            .register(RotateThreeDimensionalCommand)
            .expect("unique built-in command");
        registry
            .register(MirrorCommand)
            .expect("unique built-in command");
        registry
            .register(ShearCommand)
            .expect("unique built-in command");
        registry
            .register(ProjectToConstructionPlaneCommand)
            .expect("unique built-in command");
        registry
            .register(ToNurbsCommand)
            .expect("unique built-in command");
        registry
            .register(ClearCommand)
            .expect("unique built-in command");
        registry
            .register(UndoCommand)
            .expect("unique built-in command");
        registry
            .register(RedoCommand)
            .expect("unique built-in command");
        registry
            .register(ImportStlCommand)
            .expect("unique built-in command");
        registry
            .register(ExportStlCommand)
            .expect("unique built-in command");
        registry
            .register(ImportStepCommand)
            .expect("unique built-in command");
        registry
            .register(ExportStepCommand)
            .expect("unique built-in command");
        registry
            .register(ImportThreeDmCommand)
            .expect("unique built-in command");
        registry
            .register(ExportThreeDmCommand)
            .expect("unique built-in command");
        registry
    }

    pub fn register(&mut self, command: impl Command + 'static) -> Result<(), CommandError> {
        let index = self.commands.len();
        let keys = std::iter::once(command.name())
            .chain(command.aliases().iter().copied())
            .map(normalize_command_name)
            .collect::<BTreeSet<_>>();
        if let Some(duplicate) = keys.iter().find(|key| self.lookup.contains_key(*key)) {
            return Err(CommandError::DuplicateCommand(duplicate.clone()));
        }

        for key in keys {
            self.lookup.insert(key, index);
        }
        self.commands.push(Box::new(command));
        Ok(())
    }

    pub fn execute(&self, document: &mut Document, input: &str) -> Result<String, CommandError> {
        let mut tokens = input.split_whitespace();
        let name = tokens.next().ok_or(CommandError::EmptyInput)?;
        let name = normalize_command_name(name);

        if name == "help" || name == "?" {
            return Ok(format!("Commands: {}", self.command_names().join(", ")));
        }

        let index = self
            .lookup
            .get(&name)
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(name.clone()))?;
        let arguments: Vec<_> = tokens.collect();
        let command = &self.commands[index];
        if !command.records_history() {
            return command.run(document, &arguments);
        }

        document.begin_transaction(command.name())?;
        match command.run(document, &arguments) {
            Ok(message) => {
                document.commit_transaction()?;
                Ok(message)
            }
            Err(error) => {
                document.rollback_transaction()?;
                Err(error)
            }
        }
    }

    pub fn command_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.commands.iter().map(|command| command.name()).collect();
        names.sort_unstable_by_key(|name| name.to_ascii_lowercase());
        names
    }
}

fn normalize_command_name(name: &str) -> String {
    name.trim_start_matches(['_', '-']).to_ascii_lowercase()
}

struct PointCommand;

impl Command for PointCommand {
    fn name(&self) -> &'static str {
        "Point"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Pt"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (point, consumed) = parse_point(arguments)?;
        require_consumed(arguments, consumed, "Point x,y,z")?;
        let id = document.add_geometry(Geometry::Point(point))?;
        Ok(format!("Added point {id} at {}", format_point(point)))
    }
}

struct LineCommand;

impl Command for LineCommand {
    fn name(&self) -> &'static str {
        "Line"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["L"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (start, consumed) = parse_point(arguments)?;
        let (end, end_consumed) = parse_point(&arguments[consumed..])?;
        require_consumed(arguments, consumed + end_consumed, "Line x1,y1,z1 x2,y2,z2")?;
        let line = LineSegment::try_new(start, end, document.tolerance())?;
        let length = line.length()?;
        let id = document.add_geometry(Geometry::Line(line))?;
        Ok(format!("Added line {id} (length {length:.6})"))
    }
}

struct CircleCommand;

impl Command for CircleCommand {
    fn name(&self) -> &'static str {
        "Circle"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["C"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (center, consumed) = parse_point(arguments)?;
        let remaining = &arguments[consumed..];
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance())?;
        let circle = if remaining.len() == 1 && !remaining[0].contains(',') {
            let radius = parse_finite_real(remaining[0])?;
            Circle3::try_new(center, radius, normal, document.tolerance())?
        } else {
            let (point_on_circle, point_consumed) = parse_point(remaining)?;
            require_consumed(
                remaining,
                point_consumed,
                "Circle center radius | center point-on-circle",
            )?;
            Circle3::try_from_center_point(center, point_on_circle, normal, document.tolerance())?
        };
        let radius = circle.radius();
        let id = document.add_geometry(Geometry::Circle(circle))?;
        Ok(format!("Added circle {id} (radius {radius:.6})"))
    }
}

struct ArcCommand;

impl Command for ArcCommand {
    fn name(&self) -> &'static str {
        "Arc"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["A"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (start, start_consumed) = parse_point(arguments)?;
        let (through, through_consumed) = parse_point(&arguments[start_consumed..])?;
        let (end, end_consumed) = parse_point(&arguments[start_consumed + through_consumed..])?;
        require_consumed(
            arguments,
            start_consumed + through_consumed + end_consumed,
            "Arc start point-on-arc end",
        )?;
        let arc = CircularArc3::try_from_three_points(start, through, end, document.tolerance())?;
        let sweep_degrees = arc.sweep_radians().to_degrees();
        let id = document.add_geometry(Geometry::Arc(arc))?;
        Ok(format!("Added arc {id} (sweep {sweep_degrees:.6}°)"))
    }
}

struct EllipseCommand;

impl Command for EllipseCommand {
    fn name(&self) -> &'static str {
        "Ellipse"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Ell"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (center, center_consumed) = parse_point(arguments)?;
        let (first_axis, first_consumed) = parse_point(&arguments[center_consumed..])?;
        let (second_axis, second_consumed) =
            parse_point(&arguments[center_consumed + first_consumed..])?;
        require_consumed(
            arguments,
            center_consumed + first_consumed + second_consumed,
            "Ellipse center first-axis-point second-axis-point",
        )?;
        let ellipse =
            Ellipse3::try_from_three_points(center, first_axis, second_axis, document.tolerance())?;
        let (radius_x, radius_y) = (ellipse.radius_x(), ellipse.radius_y());
        let id = document.add_geometry(Geometry::Ellipse(ellipse))?;
        Ok(format!(
            "Added ellipse {id} (radii {radius_x:.6} × {radius_y:.6})"
        ))
    }
}

struct PolylineCommand;

impl Command for PolylineCommand {
    fn name(&self) -> &'static str {
        "Polyline"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["PLine"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut vertices = Vec::new();
        let mut consumed = 0;
        while consumed < arguments.len() {
            let (vertex, vertex_consumed) = parse_point(&arguments[consumed..])?;
            vertices.push(vertex);
            consumed += vertex_consumed;
        }
        let polyline = Polyline3::try_new(vertices, document.tolerance())?;
        let vertex_count = polyline.vertices().len();
        let segment_count = polyline.segment_count();
        let closed = polyline.is_closed();
        let id = document.add_geometry(Geometry::Polyline(polyline))?;
        Ok(format!(
            "Added {}polyline {id} ({vertex_count} vertices, {segment_count} segments)",
            if closed { "closed " } else { "" }
        ))
    }
}

struct RectangleCommand;

impl Command for RectangleCommand {
    fn name(&self) -> &'static str {
        "Rectangle"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Rect"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (first, first_consumed) = parse_point(arguments)?;
        let (opposite, opposite_consumed) = parse_point(&arguments[first_consumed..])?;
        require_consumed(
            arguments,
            first_consumed + opposite_consumed,
            "Rectangle first-corner opposite-corner",
        )?;
        let polyline = top_view_rectangle(first, opposite, document.tolerance())?;
        let width = polyline.vertices()[0].distance_to(polyline.vertices()[1])?;
        let height = polyline.vertices()[1].distance_to(polyline.vertices()[2])?;
        let id = document.add_geometry(Geometry::Polyline(polyline))?;
        Ok(format!("Added rectangle {id} ({width:.6} × {height:.6})"))
    }
}

fn top_view_rectangle(
    first: Point3,
    opposite: Point3,
    tolerance: Tolerance,
) -> Result<Polyline3, GeometryError> {
    let second = Point3::try_new(opposite.x(), first.y(), first.z())?;
    let opposite = Point3::try_new(opposite.x(), opposite.y(), first.z())?;
    let fourth = Point3::try_new(first.x(), opposite.y(), first.z())?;
    Polyline3::try_new(vec![first, second, opposite, fourth, first], tolerance)
}

struct PolygonCommand;

impl Command for PolygonCommand {
    fn name(&self) -> &'static str {
        "Polygon"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Poly"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let side_text = arguments.first().ok_or(CommandError::Usage(
            "Polygon sides center radius | sides center first-vertex",
        ))?;
        let side_count = side_text
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidInteger((*side_text).to_owned()))?;
        if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
            return Err(GeometryError::InvalidRegularPolygonSides {
                actual: side_count,
                maximum: MAX_REGULAR_POLYGON_SIDES,
            }
            .into());
        }
        let (center, center_consumed) = parse_point(&arguments[1..])?;
        let remaining = &arguments[1 + center_consumed..];
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance())?;
        let first_vertex = if remaining.len() == 1 && !remaining[0].contains(',') {
            let radius = parse_finite_real(remaining[0])?;
            Circle3::try_new(center, radius, normal, document.tolerance())?.point_at_angle(0.0)?
        } else {
            let (first_vertex, consumed) = parse_point(remaining)?;
            require_consumed(
                remaining,
                consumed,
                "Polygon sides center radius | sides center first-vertex",
            )?;
            first_vertex
        };
        let polygon = Polyline3::try_regular_polygon(
            side_count,
            center,
            first_vertex,
            normal,
            document.tolerance(),
        )?;
        let perimeter = polygon.length()?;
        let id = document.add_geometry(Geometry::Polyline(polygon))?;
        Ok(format!(
            "Added {side_count}-sided polygon {id} (perimeter {perimeter:.6})"
        ))
    }
}

struct LayerCommand;

struct ChangeLayerCommand;

impl Command for ChangeLayerCommand {
    fn name(&self) -> &'static str {
        "ChangeLayer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let name = joined_argument(arguments, "ChangeLayer layer-name")?;
        let layer_id = named_layer_id(document, &name)?;
        let count = document.set_objects_layer(selected, layer_id)?;
        Ok(format!("Changed {count} object(s) to layer '{name}'"))
    }
}

struct CopyToLayerCommand;

impl Command for CopyToLayerCommand {
    fn name(&self) -> &'static str {
        "CopyToLayer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let name = joined_argument(arguments, "CopyToLayer layer-name")?;
        let layer_id = named_layer_id(document, &name)?;
        let copies = document.copy_objects_to_layer(selected, layer_id)?;
        Ok(format!(
            "Copied {} object(s) to layer '{name}'",
            copies.len()
        ))
    }
}

struct ControlPointCurveCommand;

const CURVE_USAGE: &str = "Curve point1 point2 ... [Degree=1..11] [Close=Open|Smooth|Sharp]";

struct CurveCommand;

impl Command for CurveCommand {
    fn name(&self) -> &'static str {
        "Curve"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut control_points = Vec::new();
        let mut requested_degree = 3;
        let mut closure = ControlPointCurveClosure::Open;
        let mut degree_seen = false;
        let mut close_seen = false;
        let mut index = 0;
        while index < arguments.len() {
            let argument = arguments[index];
            if let Some((name, value)) = argument.split_once('=') {
                let value = value.trim_start_matches('_');
                if option_name_eq(name, "Degree") && !degree_seen {
                    requested_degree = value
                        .parse::<usize>()
                        .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?
                        .clamp(1, MAX_CURVE_COMMAND_DEGREE);
                    degree_seen = true;
                } else if option_name_eq(name, "Close") && !close_seen {
                    closure =
                        if value.eq_ignore_ascii_case("Open") || value.eq_ignore_ascii_case("No") {
                            ControlPointCurveClosure::Open
                        } else if value.eq_ignore_ascii_case("Smooth")
                            || value.eq_ignore_ascii_case("Yes")
                        {
                            ControlPointCurveClosure::Smooth
                        } else if value.eq_ignore_ascii_case("Sharp") {
                            ControlPointCurveClosure::Sharp
                        } else {
                            return Err(CommandError::Usage(CURVE_USAGE));
                        };
                    close_seen = true;
                } else {
                    return Err(CommandError::Usage(CURVE_USAGE));
                }
                index += 1;
            } else {
                let (point, consumed) = parse_point(&arguments[index..])?;
                control_points.push(point);
                index += consumed;
            }
        }

        let control_point_count = control_points.len();
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            requested_degree,
            control_points,
            closure,
        )?;
        let degree = curve.degree();
        let topology = match closure {
            ControlPointCurveClosure::Open => "",
            ControlPointCurveClosure::Smooth if curve.is_periodic() => "periodic ",
            ControlPointCurveClosure::Smooth => "closed ",
            ControlPointCurveClosure::Sharp => "sharp closed ",
        };
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added {topology}degree {degree} curve {id} ({control_point_count} control points)"
        ))
    }
}

impl Command for ControlPointCurveCommand {
    fn name(&self) -> &'static str {
        "ControlPointCurve"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["CPCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let degree_text = arguments.first().ok_or(CommandError::Usage(
            "ControlPointCurve degree point1 point2 ...",
        ))?;
        let degree = degree_text
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidInteger((*degree_text).to_owned()))?;

        let mut control_points = Vec::new();
        let mut consumed = 1;
        while consumed < arguments.len() {
            let (point, point_tokens) = parse_point(&arguments[consumed..])?;
            control_points.push(point);
            consumed += point_tokens;
        }

        let control_point_count = control_points.len();
        let curve = NurbsCurve::try_clamped_uniform(degree, control_points)?;
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added degree {degree} control-point curve {id} ({control_point_count} control points)"
        ))
    }
}

const INTERP_CRV_USAGE: &str = "InterpCrv point1 point2 ... [Degree=1|3] [Knots=Uniform|Chord|SqrtChrd] [Close=Open|Smooth|Sharp] [StartTangent=x,y,z] [EndTangent=x,y,z]";

struct InterpCurveCommand;

impl Command for InterpCurveCommand {
    fn name(&self) -> &'static str {
        "InterpCrv"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["InterpCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (points, options) = parse_interp_curve_arguments(arguments)?;
        let point_count = points.len();
        let curve =
            NurbsCurve::try_interpolate_for_command(&points, options, document.tolerance())?;
        let degree = curve.degree();
        let periodic = curve.is_periodic();
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added {}degree {degree} interpolated curve {id} through {point_count} point(s)",
            if periodic { "periodic " } else { "" }
        ))
    }
}

fn parse_interp_curve_arguments(
    arguments: &[&str],
) -> Result<(Vec<Point3>, CurveInterpolationOptions), CommandError> {
    let mut points = Vec::new();
    let mut degree = 3;
    let mut knot_spacing = CurveKnotSpacing::Chord;
    let mut closure = InterpolatedCurveClosure::Open;
    let mut start_tangent = None;
    let mut end_tangent = None;
    let mut degree_seen = false;
    let mut knots_seen = false;
    let mut close_seen = false;
    let mut start_tangent_seen = false;
    let mut end_tangent_seen = false;
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index];
        if let Some((name, value)) = argument.split_once('=') {
            let value = value.trim_start_matches('_');
            if option_name_eq(name, "Degree") && !degree_seen {
                degree = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
                degree_seen = true;
            } else if option_name_eq(name, "Knots") && !knots_seen {
                knot_spacing = if value.eq_ignore_ascii_case("Uniform") {
                    CurveKnotSpacing::Uniform
                } else if value.eq_ignore_ascii_case("Chord") {
                    CurveKnotSpacing::Chord
                } else if value.eq_ignore_ascii_case("SqrtChrd")
                    || value.eq_ignore_ascii_case("SqrtChord")
                    || value.eq_ignore_ascii_case("ChordSquareRoot")
                    || value.eq_ignore_ascii_case("SquareRootChord")
                {
                    CurveKnotSpacing::SquareRootChord
                } else {
                    return Err(CommandError::Usage(INTERP_CRV_USAGE));
                };
                knots_seen = true;
            } else if option_name_eq(name, "Close") && !close_seen {
                closure = if value.eq_ignore_ascii_case("Open") || value.eq_ignore_ascii_case("No")
                {
                    InterpolatedCurveClosure::Open
                } else if value.eq_ignore_ascii_case("Smooth") || value.eq_ignore_ascii_case("Yes")
                {
                    InterpolatedCurveClosure::Smooth
                } else if value.eq_ignore_ascii_case("Sharp") {
                    InterpolatedCurveClosure::Sharp
                } else {
                    return Err(CommandError::Usage(INTERP_CRV_USAGE));
                };
                close_seen = true;
            } else if option_name_eq(name, "StartTangent") && !start_tangent_seen {
                start_tangent = Some(parse_interp_curve_tangent(value)?);
                start_tangent_seen = true;
            } else if option_name_eq(name, "EndTangent") && !end_tangent_seen {
                end_tangent = Some(parse_interp_curve_tangent(value)?);
                end_tangent_seen = true;
            } else {
                return Err(CommandError::Usage(INTERP_CRV_USAGE));
            }
            index += 1;
        } else {
            let (point, consumed) = parse_point(&arguments[index..])?;
            points.push(point);
            index += consumed;
        }
    }

    let mut options = CurveInterpolationOptions::new(degree, knot_spacing, closure);
    if let Some(tangent) = start_tangent {
        options = options.with_start_tangent(tangent);
    }
    if let Some(tangent) = end_tangent {
        options = options.with_end_tangent(tangent);
    }
    Ok((points, options))
}

fn parse_interp_curve_tangent(value: &str) -> Result<Vector3, CommandError> {
    if !value.contains(',') {
        return Err(CommandError::Usage(INTERP_CRV_USAGE));
    }
    let (point, consumed) = parse_point(&[value])?;
    debug_assert_eq!(consumed, 1);
    Ok(Vector3::try_from(point.to_array())?)
}

struct SrfPtCommand;

impl Command for SrfPtCommand {
    fn name(&self) -> &'static str {
        "SrfPt"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["SurfaceFromCorners"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut corners = Vec::with_capacity(4);
        let mut consumed = 0;
        for _ in 0..4 {
            let (corner, point_tokens) = parse_point(&arguments[consumed..])?;
            corners.push(corner);
            consumed += point_tokens;
        }
        require_consumed(arguments, consumed, "SrfPt corner1 corner2 corner3 corner4")?;
        let corners: [Point3; 4] = corners
            .try_into()
            .map_err(|_| CommandError::Usage("SrfPt corner1 corner2 corner3 corner4"))?;
        let surface = NurbsSurface::try_bilinear(corners)?;
        // A tensor-product surface may legitimately have singular boundaries,
        // but four-corner construction must span at least one valid face.
        surface.tessellate(1, document.tolerance())?;
        let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
        Ok(format!("Added four-corner NURBS surface {id}"))
    }
}

impl Command for LayerCommand {
    fn name(&self) -> &'static str {
        "Layer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage(
                "Layer name | New name | Current/Show/Hide/Lock/Unlock/Delete name | Color r,g,b name | Rename old => new | List",
            ));
        }
        match arguments[0].to_ascii_lowercase().as_str() {
            "new" => create_current_layer(document, &arguments[1..]),
            "current" => {
                let name = joined_argument(&arguments[1..], "Layer Current name")?;
                let id = named_layer_id(document, &name)?;
                document.set_current_layer(id)?;
                Ok(format!("Current layer is '{name}'"))
            }
            "show" | "hide" => {
                let visible = arguments[0].eq_ignore_ascii_case("show");
                let name = joined_argument(&arguments[1..], "Layer Show|Hide name")?;
                let id = named_layer_id(document, &name)?;
                document.set_layer_visibility(id, visible)?;
                Ok(format!(
                    "Layer '{name}' is {}",
                    if visible { "visible" } else { "hidden" }
                ))
            }
            "lock" | "unlock" => {
                let locked = arguments[0].eq_ignore_ascii_case("lock");
                let name = joined_argument(&arguments[1..], "Layer Lock|Unlock name")?;
                let id = named_layer_id(document, &name)?;
                document.set_layer_locked(id, locked)?;
                Ok(format!(
                    "Layer '{name}' is {}",
                    if locked { "locked" } else { "unlocked" }
                ))
            }
            "delete" => {
                let name = joined_argument(&arguments[1..], "Layer Delete name")?;
                let id = named_layer_id(document, &name)?;
                document.delete_layer(id)?;
                Ok(format!("Deleted layer '{name}'"))
            }
            "color" => {
                let color_text = arguments
                    .get(1)
                    .ok_or(CommandError::Usage("Layer Color r,g,b name"))?;
                let color = parse_color(color_text)?;
                let name = joined_argument(&arguments[2..], "Layer Color r,g,b name")?;
                let id = named_layer_id(document, &name)?;
                document.set_layer_color(id, color)?;
                Ok(format!(
                    "Layer '{name}' color is {},{},{}",
                    color.red, color.green, color.blue
                ))
            }
            "rename" => {
                let separator = arguments
                    .iter()
                    .position(|argument| *argument == "=>")
                    .ok_or(CommandError::Usage("Layer Rename old name => new name"))?;
                let old_name = joined_argument(
                    &arguments[1..separator],
                    "Layer Rename old name => new name",
                )?;
                let new_name = joined_argument(
                    &arguments[separator + 1..],
                    "Layer Rename old name => new name",
                )?;
                let id = named_layer_id(document, &old_name)?;
                document.rename_layer(id, &new_name)?;
                Ok(format!("Renamed layer '{old_name}' to '{new_name}'"))
            }
            "list" => {
                require_consumed(arguments, 1, "Layer List")?;
                let layers = document
                    .layers()
                    .map(|layer| {
                        let current = if layer.id() == document.current_layer_id() {
                            " current"
                        } else {
                            ""
                        };
                        let visibility = if layer.is_visible() {
                            "visible"
                        } else {
                            "hidden"
                        };
                        let lock = if layer.is_locked() {
                            "locked"
                        } else {
                            "unlocked"
                        };
                        format!("{} ({visibility}, {lock}{current})", layer.name())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("Layers: {layers}"))
            }
            _ => create_current_layer(document, arguments),
        }
    }
}

struct SelAllCommand;

impl Command for SelAllCommand {
    fn name(&self) -> &'static str {
        "SelAll"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "SelAll")?;
        let count = document.select_all();
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelNoneCommand;

impl Command for SelNoneCommand {
    fn name(&self) -> &'static str {
        "SelNone"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "SelNone")?;
        let count = document.clear_selection();
        Ok(format!("Deselected {count} object(s)"))
    }
}

struct InvertCommand;

impl Command for InvertCommand {
    fn name(&self) -> &'static str {
        "Invert"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Invert")?;
        let count = document.invert_selection();
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelLastCommand;

impl Command for SelLastCommand {
    fn name(&self) -> &'static str {
        "SelLast"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let deselect_others = parse_action_selection_arguments(
            arguments,
            "SelLast [DeselectOthersBeforeSelect=Yes|No]",
        )?;
        let count = document.select_last_changed(deselect_others);
        Ok(format!("Selection contains {count} object(s)"))
    }
}

struct SelPrevCommand;

impl Command for SelPrevCommand {
    fn name(&self) -> &'static str {
        "SelPrev"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let deselect_others = parse_action_selection_arguments(
            arguments,
            "SelPrev [DeselectOthersBeforeSelect=Yes|No]",
        )?;
        let count = document.select_previous(deselect_others);
        Ok(format!("Selection contains {count} object(s)"))
    }
}

fn parse_action_selection_arguments(
    arguments: &[&str],
    usage: &'static str,
) -> Result<bool, CommandError> {
    if arguments.is_empty() {
        return Ok(true);
    }
    let (name, value) = match arguments {
        [option] => option.split_once('=').ok_or(CommandError::Usage(usage))?,
        [name, value] => (*name, *value),
        _ => return Err(CommandError::Usage(usage)),
    };
    if !name
        .trim_start_matches('_')
        .eq_ignore_ascii_case("DeselectOthersBeforeSelect")
    {
        return Err(CommandError::Usage(usage));
    }
    parse_yes_no(value.trim_start_matches('_')).ok_or(CommandError::Usage(usage))
}

struct SelNameCommand;

impl Command for SelNameCommand {
    fn name(&self) -> &'static str {
        "SelName"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let pattern = parse_attribute_pattern(arguments, "SelName name-pattern")?;
        let count = document.select_objects_by_name_pattern(&pattern);
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelColorCommand;

impl Command for SelColorCommand {
    fn name(&self) -> &'static str {
        "SelColor"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [value] = arguments else {
            return Err(CommandError::Usage("SelColor r,g,b"));
        };
        let color = parse_color(value)?;
        let count = document.select_objects_by_display_color(color)?;
        Ok(format!(
            "Selected {count} object(s) with display color {},{},{}",
            color.red, color.green, color.blue
        ))
    }
}

struct SelLayerCommand;

impl Command for SelLayerCommand {
    fn name(&self) -> &'static str {
        "SelLayer"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let pattern = parse_attribute_pattern(arguments, "SelLayer layer-pattern")?;
        let count = document.select_layer_objects_by_name_pattern(&pattern)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelGroupCommand;

impl Command for SelGroupCommand {
    fn name(&self) -> &'static str {
        "SelGroup"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let name = parse_attribute_pattern(arguments, "SelGroup group-name")?;
        let count = document.select_group_objects_by_name(&name);
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelectDuplicateCommand {
    name: &'static str,
    include_originals: bool,
}

impl Command for SelectDuplicateCommand {
    fn name(&self) -> &'static str {
        self.name
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name)?;
        let count = document.select_duplicate_objects(self.include_originals)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

fn parse_attribute_pattern(
    arguments: &[&str],
    usage: &'static str,
) -> Result<String, CommandError> {
    if arguments.is_empty() {
        return Err(CommandError::Usage(usage));
    }
    let joined = arguments.join(" ");
    let pattern = joined.trim();
    let starts_quoted = pattern.starts_with('"');
    let ends_quoted = pattern.ends_with('"');
    if starts_quoted != ends_quoted {
        return Err(CommandError::Usage(usage));
    }
    Ok(if starts_quoted && pattern.len() >= 2 {
        pattern[1..pattern.len() - 1].to_owned()
    } else {
        pattern.to_owned()
    })
}

struct SelectGeometryCommand {
    name: &'static str,
    filter: GeometrySelectionFilter,
}

impl Command for SelectGeometryCommand {
    fn name(&self) -> &'static str {
        self.name
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name)?;
        let tolerance = document.tolerance();
        let matches = document
            .objects()
            .filter(|object| document.is_object_selectable(object.id()))
            .map(|object| {
                Ok(self
                    .filter
                    .matches(object.geometry(), tolerance)?
                    .then_some(object.id()))
            })
            .collect::<Result<Vec<Option<ObjectId>>, GeometryError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        document.select_objects(matches, SelectionMode::Add)?;
        Ok(format!(
            "Selected {} object(s)",
            document.selected_object_count()
        ))
    }
}

#[derive(Clone, Copy)]
enum GeometrySelectionFilter {
    Curve,
    OpenCurve,
    ClosedCurve,
    PlanarCurve,
    Line,
    Polyline,
    Point,
    PointCloud,
    Surface,
    Mesh,
    OpenMesh,
    ClosedMesh,
}

impl GeometrySelectionFilter {
    fn matches(self, geometry: &Geometry, tolerance: Tolerance) -> Result<bool, GeometryError> {
        let matches = match self {
            Self::Curve => geometry_curve_ref(geometry).is_some(),
            Self::OpenCurve => match geometry_curve_ref(geometry) {
                Some(curve) => !curve.is_closed()?,
                None => false,
            },
            Self::ClosedCurve => match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_closed()?,
                None => false,
            },
            Self::PlanarCurve => match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_planar(tolerance)?,
                None => false,
            },
            Self::Line => match geometry {
                Geometry::Line(_) => true,
                Geometry::NurbsCurve(curve) => {
                    curve.spans().count() == 1 && curve.is_linear_at_zero_tolerance()?
                }
                _ => false,
            },
            Self::Polyline => match geometry {
                Geometry::Polyline(_) => true,
                Geometry::NurbsCurve(curve) => {
                    curve.degree() == 1 && curve.control_points().len() > 2
                }
                _ => false,
            },
            Self::Point => matches!(geometry, Geometry::Point(_)),
            Self::PointCloud => matches!(geometry, Geometry::PointCloud(_)),
            Self::Surface => matches!(geometry, Geometry::NurbsSurface(_)),
            Self::Mesh => matches!(geometry, Geometry::Mesh(_)),
            Self::OpenMesh => match geometry {
                Geometry::Mesh(mesh) => !mesh.topology().is_closed(),
                _ => false,
            },
            Self::ClosedMesh => match geometry {
                Geometry::Mesh(mesh) => mesh.topology().is_closed(),
                _ => false,
            },
        };
        Ok(matches)
    }
}

struct SelShortCurveCommand;

impl Command for SelShortCurveCommand {
    fn name(&self) -> &'static str {
        "SelShortCrv"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [maximum_length] = arguments else {
            return Err(CommandError::Usage("SelShortCrv maximum-length"));
        };
        let maximum_length = parse_positive_curve_length(maximum_length)?;
        let tolerance = document.tolerance();
        let matches = document
            .objects()
            .filter(|object| document.is_object_selectable(object.id()))
            .filter_map(|object| {
                geometry_curve_ref(object.geometry()).map(|curve| (object.id(), curve))
            })
            .map(|(id, curve)| Ok((curve.length(tolerance)? <= maximum_length).then_some(id)))
            .collect::<Result<Vec<Option<ObjectId>>, GeometryError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        document.select_objects(matches, SelectionMode::Add)?;
        Ok(format!(
            "Selected {} object(s)",
            document.selected_object_count()
        ))
    }
}

struct LengthCommand;

impl Command for LengthCommand {
    fn name(&self) -> &'static str {
        "Length"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Len"]
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Length")?;
        let (count, total) = selected_measurement(document, |geometry, tolerance| {
            geometry_curve_ref(geometry)
                .ok_or(CommandError::UnsupportedLengthGeometry)?
                .length(tolerance)
                .map_err(CommandError::from)
        })?;
        Ok(format!(
            "Measured {count} curve(s): total length {total:.12}"
        ))
    }
}

struct AreaCommand;

impl Command for AreaCommand {
    fn name(&self) -> &'static str {
        "Area"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Area")?;
        let (count, total) =
            selected_measurement(document, |geometry, tolerance| match geometry {
                Geometry::Circle(circle) => Ok(circle.area()?),
                Geometry::Ellipse(ellipse) => Ok(ellipse.area()?),
                Geometry::Polyline(polyline) => Ok(polyline.planar_area(tolerance)?),
                Geometry::Mesh(mesh) => Ok(mesh.area()?),
                _ => Err(CommandError::UnsupportedAreaGeometry),
            })?;
        Ok(format!(
            "Measured {count} object(s): total area {total:.12}"
        ))
    }
}

struct VolumeCommand;

impl Command for VolumeCommand {
    fn name(&self) -> &'static str {
        "Volume"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Volume")?;
        let selected = document.selected_objects().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        for object in &selected {
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::UnsupportedVolumeGeometry);
            };
            if !mesh.topology().is_closed() {
                return Err(CommandError::OpenMeshVolume);
            }
            let volume = mesh.signed_volume()?;
            let next = sum + volume;
            if sum.abs() >= volume.abs() {
                correction += (sum - next) + volume;
            } else {
                correction += (volume - next) + sum;
            }
            sum = next;
        }
        let total = sum + correction;
        if !total.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "volume total",
            }
            .into());
        }
        Ok(format!(
            "Measured {} closed mesh(es): total volume {total:.12}",
            selected.len()
        ))
    }
}

fn selected_measurement(
    document: &Document,
    mut measure: impl FnMut(&Geometry, Tolerance) -> Result<Real, CommandError>,
) -> Result<(usize, Real), CommandError> {
    let selected = document.selected_objects().collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let mut sum = 0.0;
    let mut correction = 0.0;
    for object in &selected {
        let value = measure(object.geometry(), document.tolerance())?;
        if !value.is_finite() || value < 0.0 {
            return Err(GeometryError::NonFinite {
                context: "geometry measurement",
            }
            .into());
        }
        let next = sum + value;
        if sum.abs() >= value.abs() {
            correction += (sum - next) + value;
        } else {
            correction += (value - next) + sum;
        }
        sum = next;
    }
    let total = sum + correction;
    if !total.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "measurement total",
        }
        .into());
    }
    Ok((selected.len(), total))
}

struct DivideCommand;

impl Command for DivideCommand {
    fn name(&self) -> &'static str {
        "Divide"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Div"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (specification, mark_ends) = parse_division_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.geometry().clone(), object.attributes().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mut output = Vec::new();
        for (geometry, attributes) in &selected {
            let curve =
                geometry_curve_ref(geometry).ok_or(CommandError::UnsupportedDivideGeometry)?;
            let points =
                command_division_points(curve, specification, mark_ends, document.tolerance())?;
            let output_count = output.len().checked_add(points.len()).ok_or(
                GeometryError::TooManyCurveDivisionPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                },
            )?;
            if output_count > MAX_CURVE_DIVISION_POINTS {
                return Err(GeometryError::TooManyCurveDivisionPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                }
                .into());
            }
            output.extend(points.into_iter().map(|point| (point, attributes.clone())));
        }
        if output.is_empty() {
            return Err(CommandError::NoCurveDivisionPoints);
        }

        let mut ids = Vec::with_capacity(output.len());
        for (point, attributes) in output {
            ids.push(document.add_geometry_with_attributes(Geometry::Point(point), attributes)?);
        }
        replace_selection(document, ids.iter().copied())?;
        Ok(format!(
            "Divided {} curve(s), adding {} point(s)",
            selected.len(),
            ids.len()
        ))
    }
}

#[derive(Clone, Copy)]
enum DivisionSpecification {
    Count(usize),
    Length(Real),
}

fn parse_division_arguments(
    arguments: &[&str],
) -> Result<(DivisionSpecification, bool), CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(
            "Divide segment-count [MarkEnds] | Divide Length segment-length [MarkEnds]",
        ));
    };
    let (specification, option_index) = if first.eq_ignore_ascii_case("length") {
        let value = arguments.get(1).ok_or(CommandError::Usage(
            "Divide Length segment-length [MarkEnds]",
        ))?;
        (DivisionSpecification::Length(parse_finite_real(value)?), 2)
    } else {
        let count = first
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidInteger((*first).to_owned()))?;
        (DivisionSpecification::Count(count), 1)
    };
    let mark_ends = match &arguments[option_index..] {
        [] => false,
        [option] if option.eq_ignore_ascii_case("MarkEnds") => true,
        _ => {
            return Err(CommandError::Usage(
                "Divide segment-count [MarkEnds] | Divide Length segment-length [MarkEnds]",
            ));
        }
    };
    Ok((specification, mark_ends))
}

fn geometry_curve_ref(geometry: &Geometry) -> Option<CurveRef<'_>> {
    match geometry {
        Geometry::Line(line) => Some(CurveRef::Line(line)),
        Geometry::Circle(circle) => Some(CurveRef::Circle(circle)),
        Geometry::Arc(arc) => Some(CurveRef::Arc(arc)),
        Geometry::Ellipse(ellipse) => Some(CurveRef::Ellipse(ellipse)),
        Geometry::Polyline(polyline) => Some(CurveRef::Polyline(polyline)),
        Geometry::NurbsCurve(curve) => Some(CurveRef::NurbsCurve(curve)),
        _ => None,
    }
}

fn command_division_points(
    curve: CurveRef<'_>,
    specification: DivisionSpecification,
    mark_ends: bool,
    tolerance: Tolerance,
) -> Result<Vec<Point3>, CommandError> {
    let closed = curve.is_closed()?;
    let mut points = match specification {
        DivisionSpecification::Count(count) => {
            let mut points = curve.divide_by_count(count, true, tolerance)?;
            if closed {
                points.pop();
            } else if !mark_ends {
                points.remove(0);
                points.pop();
            }
            return Ok(points);
        }
        DivisionSpecification::Length(length) => curve.divide_by_length(length, true, tolerance)?,
    };

    let start = curve.start_point()?;
    let end = curve.end_point()?;
    if closed {
        if points.len() > 1
            && points
                .last()
                .is_some_and(|point| point.is_near(start, tolerance))
        {
            points.pop();
        }
    } else if mark_ends {
        if let Some(last) = points.last_mut()
            && last.is_near(end, tolerance)
        {
            *last = end;
        } else {
            points.push(end);
        }
    } else {
        if points
            .last()
            .is_some_and(|point| point.is_near(end, tolerance))
        {
            points.pop();
        }
        if points
            .first()
            .is_some_and(|point| point.is_near(start, tolerance))
        {
            points.remove(0);
        }
    }
    Ok(points)
}

struct CrvStartCommand;

impl Command for CrvStartCommand {
    fn name(&self) -> &'static str {
        "CrvStart"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CrvStart")?;
        mark_selected_curve_endpoints(document, CurveEndpoint::Start)
    }
}

struct CrvEndCommand;

impl Command for CrvEndCommand {
    fn name(&self) -> &'static str {
        "CrvEnd"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CrvEnd")?;
        mark_selected_curve_endpoints(document, CurveEndpoint::End)
    }
}

#[derive(Clone, Copy)]
enum CurveEndpoint {
    Start,
    End,
}

fn mark_selected_curve_endpoints(
    document: &mut Document,
    endpoint: CurveEndpoint,
) -> Result<String, CommandError> {
    let selected = document
        .selected_objects()
        .map(|object| {
            let curve = geometry_curve_ref(object.geometry())
                .ok_or(CommandError::UnsupportedCurveEndpointGeometry)?;
            let point = match endpoint {
                CurveEndpoint::Start => curve.start_point()?,
                CurveEndpoint::End => curve.end_point()?,
            };
            Ok((point, object.attributes().clone()))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    if selected.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }

    let mut ids = Vec::with_capacity(selected.len());
    for (point, attributes) in selected {
        ids.push(document.add_geometry_with_attributes(Geometry::Point(point), attributes)?);
    }
    replace_selection(document, ids.iter().copied())?;
    let location = match endpoint {
        CurveEndpoint::Start => "starts",
        CurveEndpoint::End => "ends",
    };
    Ok(format!("Added {} point(s) at curve {location}", ids.len()))
}

const EXTRACT_PT_USAGE: &str = "ExtractPt [OutputLayer=Input|Current] [Output=Points|PointCloud]";

struct ExtractPtCommand;

impl Command for ExtractPtCommand {
    fn name(&self) -> &'static str {
        "ExtractPt"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_point_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.geometry().clone(), object.attributes().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let current_layer = document.current_layer_id();
        let mut output = Vec::new();
        let mut source_with_points = 0;
        let mut first_source_has_points = false;
        for (source_index, (geometry, attributes)) in selected.iter().enumerate() {
            let points = geometry.extract_point_locations()?;
            if source_index == 0 {
                first_source_has_points = !points.is_empty();
            }
            if points.is_empty() {
                continue;
            }
            source_with_points += 1;
            let output_count = output.len().checked_add(points.len()).ok_or(
                CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                },
            )?;
            if output_count > MAX_EXTRACTED_POINTS {
                return Err(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                });
            }
            output
                .try_reserve(points.len())
                .map_err(|_| CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                })?;
            let attributes = match options.output_layer {
                ExtractPointOutputLayer::Input => attributes.clone(),
                ExtractPointOutputLayer::Current => ObjectAttributes::on_layer(current_layer),
            };
            output.extend(points.into_iter().map(|point| (point, attributes.clone())));
        }
        if output.is_empty() {
            return Err(CommandError::NoExtractablePoints);
        }

        let point_count = output.len();
        let mut ids = Vec::with_capacity(match options.output {
            ExtractPointOutput::Points => point_count,
            ExtractPointOutput::PointCloud => 1,
        });
        match options.output {
            ExtractPointOutput::Points => {
                for (point, attributes) in output {
                    ids.push(
                        document
                            .add_geometry_with_attributes(Geometry::Point(point), attributes)?,
                    );
                }
            }
            ExtractPointOutput::PointCloud => {
                let attributes = match options.output_layer {
                    ExtractPointOutputLayer::Input => selected
                        .first()
                        .filter(|_| first_source_has_points)
                        .map_or_else(
                            || ObjectAttributes::on_layer(current_layer),
                            |(_, attributes)| attributes.clone(),
                        ),
                    ExtractPointOutputLayer::Current => ObjectAttributes::on_layer(current_layer),
                };
                let cloud =
                    PointCloud3::try_new(output.into_iter().map(|(point, _)| point).collect())?;
                ids.push(
                    document
                        .add_geometry_with_attributes(Geometry::PointCloud(cloud), attributes)?,
                );
            }
        }
        replace_selection(document, ids.iter().copied())?;
        Ok(format!(
            "Extracted {} point(s) from {source_with_points} of {} selected object(s)",
            point_count,
            selected.len()
        ))
    }
}

#[derive(Clone, Copy)]
enum ExtractPointOutputLayer {
    Input,
    Current,
}

#[derive(Clone, Copy)]
enum ExtractPointOutput {
    Points,
    PointCloud,
}

#[derive(Clone, Copy)]
struct ExtractPointOptions {
    output_layer: ExtractPointOutputLayer,
    output: ExtractPointOutput,
}

fn parse_extract_point_arguments(arguments: &[&str]) -> Result<ExtractPointOptions, CommandError> {
    let mut output_layer = ExtractPointOutputLayer::Input;
    let mut output = ExtractPointOutput::Points;
    let mut output_layer_seen = false;
    let mut output_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_PT_USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        let value = value.trim_start_matches('_');
        if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
            output_layer = if value.eq_ignore_ascii_case("Input") {
                ExtractPointOutputLayer::Input
            } else if value.eq_ignore_ascii_case("Current") {
                ExtractPointOutputLayer::Current
            } else {
                return Err(CommandError::Usage(EXTRACT_PT_USAGE));
            };
            output_layer_seen = true;
        } else if name.eq_ignore_ascii_case("Output") && !output_seen {
            if value.eq_ignore_ascii_case("PointCloud") {
                output = ExtractPointOutput::PointCloud;
            } else if value.eq_ignore_ascii_case("Points") {
                output = ExtractPointOutput::Points;
            } else {
                return Err(CommandError::Usage(EXTRACT_PT_USAGE));
            }
            output_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_PT_USAGE));
        }
        index += consumed;
    }
    Ok(ExtractPointOptions {
        output_layer,
        output,
    })
}

const CLOSE_CRV_USAGE: &str = "CloseCrv [CloseWideGapsWithLine=Yes|No] [Tolerance=value]";

struct CloseCrvCommand;

impl Command for CloseCrvCommand {
    fn name(&self) -> &'static str {
        "CloseCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_close_curve_arguments(arguments, document.tolerance().absolute())?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mut endpoint_moved = 0;
        let mut segment_added = 0;
        let mut unchanged = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in &selected {
            let polyline = match geometry {
                Geometry::Line(line) => {
                    Polyline3::try_new(vec![line.start(), line.end()], document.tolerance())?
                }
                Geometry::Polyline(polyline) => polyline.clone(),
                Geometry::Circle(_) | Geometry::Ellipse(_) => {
                    unchanged += 1;
                    continue;
                }
                Geometry::NurbsCurve(curve) if curve.is_closed()? => {
                    unchanged += 1;
                    continue;
                }
                _ => return Err(CommandError::UnsupportedCloseCurveGeometry),
            };
            let (closed, outcome) = polyline.close(
                options.tolerance,
                options.close_wide_gaps_with_line,
                document.tolerance(),
            )?;
            match outcome {
                PolylineClosure::EndpointMoved => {
                    endpoint_moved += 1;
                    replacements.push((*id, Geometry::Polyline(closed)));
                }
                PolylineClosure::SegmentAdded => {
                    segment_added += 1;
                    replacements.push((*id, Geometry::Polyline(closed)));
                }
                PolylineClosure::AlreadyClosed
                | PolylineClosure::GapTooWide
                | PolylineClosure::NotClosable => unchanged += 1,
            }
        }

        let closed = document.replace_object_geometries(replacements)?;
        debug_assert_eq!(closed, endpoint_moved + segment_added);
        Ok(format!(
            "Closed {closed} of {} selected curve(s): {segment_added} with a line, {endpoint_moved} by moving an endpoint; {unchanged} unchanged",
            selected.len()
        ))
    }
}

#[derive(Clone, Copy)]
struct CloseCurveOptions {
    close_wide_gaps_with_line: bool,
    tolerance: Real,
}

fn parse_close_curve_arguments(
    arguments: &[&str],
    default_tolerance: Real,
) -> Result<CloseCurveOptions, CommandError> {
    let mut options = CloseCurveOptions {
        close_wide_gaps_with_line: true,
        tolerance: default_tolerance,
    };
    let mut wide_gap_seen = false;
    let mut tolerance_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(CLOSE_CRV_USAGE))?;
            (argument, *value, 2)
        };
        if name.eq_ignore_ascii_case("CloseWideGapsWithLine") && !wide_gap_seen {
            options.close_wide_gaps_with_line =
                parse_yes_no(value).ok_or(CommandError::Usage(CLOSE_CRV_USAGE))?;
            wide_gap_seen = true;
        } else if name.eq_ignore_ascii_case("Tolerance") && !tolerance_seen {
            options.tolerance = parse_finite_real(value)?;
            if options.tolerance < 0.0 {
                return Err(GeometryError::InvalidCurveClosureTolerance.into());
            }
            tolerance_seen = true;
        } else {
            return Err(CommandError::Usage(CLOSE_CRV_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

fn parse_yes_no(value: &str) -> Option<bool> {
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("yes") {
        Some(true)
    } else if value.eq_ignore_ascii_case("no") {
        Some(false)
    } else {
        None
    }
}

struct FlipCommand;

impl Command for FlipCommand {
    fn name(&self) -> &'static str {
        "Flip"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Reverse", "Rev"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Flip")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let replacements = selected
            .into_iter()
            .map(|(id, geometry)| {
                let reversed = match geometry {
                    Geometry::Line(line) => Geometry::Line(line.reversed()),
                    Geometry::Circle(circle) => Geometry::Circle(circle.reversed()),
                    Geometry::Arc(arc) => Geometry::Arc(arc.reversed(document.tolerance())?),
                    Geometry::Ellipse(ellipse) => Geometry::Ellipse(ellipse.reversed()),
                    Geometry::Polyline(polyline) => Geometry::Polyline(polyline.reversed()),
                    Geometry::NurbsCurve(curve) => Geometry::NurbsCurve(curve.reversed()?),
                    Geometry::Mesh(mesh) => Geometry::Mesh(mesh.reversed()),
                    _ => return Err(CommandError::UnsupportedFlipGeometry),
                };
                Ok((id, reversed))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let count = document.replace_object_geometries(replacements)?;
        Ok(format!("Flipped {count} object(s)"))
    }
}

struct UnifyMeshNormalsCommand;

impl Command for UnifyMeshNormalsCommand {
    fn name(&self) -> &'static str {
        "UnifyMeshNormals"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "UnifyMeshNormals")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mesh_count = selected.len();
        let mut flipped_face_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedUnifyMeshNormalsGeometry);
            };
            let (unified, flipped) = mesh.unified_face_orientations()?;
            flipped_face_count += flipped;
            if flipped > 0 {
                replacements.push((id, Geometry::Mesh(unified)));
            }
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Unified {mesh_count} mesh(es): flipped {flipped_face_count} face(s) in {changed_mesh_count} mesh(es); {} already consistent",
            mesh_count - changed_mesh_count
        ))
    }
}

struct CombineIdenticalMeshVerticesCommand;

impl Command for CombineIdenticalMeshVerticesCommand {
    fn name(&self) -> &'static str {
        "CombineIdenticalMeshVertices"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CombineIdenticalMeshVertices")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mesh_count = selected.len();
        let mut removed_vertex_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedCombineIdenticalMeshVerticesGeometry);
            };
            let (combined, removed) = mesh.combined_identical_vertices();
            removed_vertex_count += removed;
            if removed > 0 {
                replacements.push((id, Geometry::Mesh(combined)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoIdenticalMeshVertices);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Combined {removed_vertex_count} identical vertex occurrence(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

struct CullUnusedMeshVerticesCommand;

impl Command for CullUnusedMeshVerticesCommand {
    fn name(&self) -> &'static str {
        "CullUnusedMeshVertices"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "CullUnusedMeshVertices")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mesh_count = selected.len();
        let mut removed_vertex_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedCullUnusedMeshVerticesGeometry);
            };
            let (culled, removed) = mesh.culled_unused_vertices();
            removed_vertex_count += removed;
            if removed > 0 {
                replacements.push((id, Geometry::Mesh(culled)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoUnusedMeshVertices);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Culled {removed_vertex_count} unused vertex occurrence(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

struct SplitDisjointMeshCommand;

impl Command for SplitDisjointMeshCommand {
    fn name(&self) -> &'static str {
        "SplitDisjointMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "SplitDisjointMesh")?;
        let selected_ids = document.selected_object_ids().collect::<Vec<_>>();
        if selected_ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let inputs = selected_ids
            .into_iter()
            .map(|id| {
                let object = document
                    .object(id)
                    .expect("selected object identities belong to the document");
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedSplitDisjointMeshGeometry);
                };
                let group_ids = document
                    .groups()
                    .filter(|group| group.members().any(|member| member == id))
                    .map(|group| group.id())
                    .collect();
                Ok(SplitMeshInput {
                    id,
                    attributes: object.attributes().clone(),
                    group_ids,
                    pieces: mesh.disjoint_pieces(),
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let split_mesh_count = inputs.iter().filter(|input| input.pieces.len() > 1).count();
        if split_mesh_count == 0 {
            return Err(CommandError::NoDisjointMeshes);
        }
        let unchanged_mesh_count = inputs.len() - split_mesh_count;
        let piece_count = inputs
            .iter()
            .filter(|input| input.pieces.len() > 1)
            .map(|input| input.pieces.len())
            .sum::<usize>();

        let replacements = inputs
            .iter()
            .filter(|input| input.pieces.len() > 1)
            .map(|input| (input.id, Geometry::Mesh(input.pieces[0].clone())))
            .collect::<Vec<_>>();
        let replaced = document.replace_object_geometries(replacements)?;
        debug_assert_eq!(replaced, split_mesh_count);

        let mut output_ids = Vec::with_capacity(inputs.len() + piece_count - split_mesh_count);
        let mut group_additions = BTreeMap::<GroupId, Vec<ObjectId>>::new();
        for input in inputs {
            output_ids.push(input.id);
            if input.pieces.len() <= 1 {
                continue;
            }
            for piece in input.pieces.into_iter().skip(1) {
                let id = document.add_geometry_with_attributes(
                    Geometry::Mesh(piece),
                    input.attributes.clone(),
                )?;
                output_ids.push(id);
                for group_id in &input.group_ids {
                    group_additions.entry(*group_id).or_default().push(id);
                }
            }
        }
        for (group_id, additions) in group_additions {
            document.add_group_members(group_id, additions)?;
        }
        replace_selection(document, output_ids)?;
        Ok(format!(
            "Split {split_mesh_count} mesh(es) into {piece_count} piece(s); {unchanged_mesh_count} mesh(es) unchanged"
        ))
    }
}

struct SplitMeshInput {
    id: ObjectId,
    attributes: ObjectAttributes,
    group_ids: Vec<GroupId>,
    pieces: Vec<TriangleMesh>,
}

const EXTRACT_NON_MANIFOLD_USAGE: &str =
    "ExtractNonManifoldMeshEdges [ExtractHangingFacesOnly=Yes|No] [MinimumFaceCount=count]";

struct ExtractNonManifoldMeshEdgesCommand;

impl Command for ExtractNonManifoldMeshEdgesCommand {
    fn name(&self) -> &'static str {
        "ExtractNonManifoldMeshEdges"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_non_manifold_arguments(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let inputs = stage_selected_mesh_face_extractions(
            document,
            || CommandError::UnsupportedExtractNonManifoldGeometry,
            |mesh| {
                mesh.extract_non_manifold_faces(
                    options.minimum_face_count,
                    options.hanging_faces_only,
                )
            },
        )?;
        let counts = mesh_face_extraction_counts(&inputs);
        if counts.extracted_meshes == 0 {
            return Err(CommandError::NoNonManifoldMeshFaces);
        }
        apply_mesh_face_extractions(document, inputs, counts.extracted_meshes)?;
        Ok(format!(
            "Extracted {} face(s) from {} mesh(es); {} mesh(es) unchanged",
            counts.extracted_faces, counts.extracted_meshes, counts.unchanged_meshes
        ))
    }
}

struct MeshFaceExtractionInput {
    id: ObjectId,
    attributes: ObjectAttributes,
    group_ids: Vec<GroupId>,
    extraction: Option<(Option<TriangleMesh>, TriangleMesh)>,
}

#[derive(Clone, Copy)]
struct MeshFaceExtractionCounts {
    extracted_meshes: usize,
    unchanged_meshes: usize,
    extracted_faces: usize,
}

fn stage_selected_mesh_face_extractions(
    document: &Document,
    unsupported_geometry: impl Fn() -> CommandError,
    mut extract: impl FnMut(&TriangleMesh) -> Result<Option<MeshFaceExtraction>, GeometryError>,
) -> Result<Vec<MeshFaceExtractionInput>, CommandError> {
    document
        .selected_object_ids()
        .map(|id| {
            let object = document
                .object(id)
                .expect("selected object identities belong to the document");
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(unsupported_geometry());
            };
            let group_ids = document
                .groups()
                .filter(|group| group.members().any(|member| member == id))
                .map(|group| group.id())
                .collect();
            Ok(MeshFaceExtractionInput {
                id,
                attributes: object.attributes().clone(),
                group_ids,
                extraction: extract(mesh)?.map(MeshFaceExtraction::into_parts),
            })
        })
        .collect()
}

fn mesh_face_extraction_counts(inputs: &[MeshFaceExtractionInput]) -> MeshFaceExtractionCounts {
    let extracted_meshes = inputs
        .iter()
        .filter(|input| input.extraction.is_some())
        .count();
    let extracted_faces = inputs
        .iter()
        .filter_map(|input| input.extraction.as_ref())
        .map(|(_, extracted)| extracted.triangles().len())
        .sum();
    MeshFaceExtractionCounts {
        extracted_meshes,
        unchanged_meshes: inputs.len() - extracted_meshes,
        extracted_faces,
    }
}

fn apply_mesh_face_extractions(
    document: &mut Document,
    inputs: Vec<MeshFaceExtractionInput>,
    extracted_mesh_count: usize,
) -> Result<(), CommandError> {
    let replacements = inputs
        .iter()
        .filter_map(|input| {
            input.extraction.as_ref().map(|(remainder, extracted)| {
                (
                    input.id,
                    Geometry::Mesh(remainder.as_ref().unwrap_or(extracted).clone()),
                )
            })
        })
        .collect::<Vec<_>>();
    let replaced = document.replace_object_geometries(replacements)?;
    debug_assert_eq!(replaced, extracted_mesh_count);

    let mut output_ids = Vec::with_capacity(inputs.len() + extracted_mesh_count);
    let mut group_additions = BTreeMap::<GroupId, Vec<ObjectId>>::new();
    for input in inputs {
        output_ids.push(input.id);
        let Some((remainder, extracted)) = input.extraction else {
            continue;
        };
        if remainder.is_none() {
            continue;
        }
        let id =
            document.add_geometry_with_attributes(Geometry::Mesh(extracted), input.attributes)?;
        output_ids.push(id);
        for group_id in input.group_ids {
            group_additions.entry(group_id).or_default().push(id);
        }
    }
    for (group_id, additions) in group_additions {
        document.add_group_members(group_id, additions)?;
    }
    replace_selection(document, output_ids)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct ExtractNonManifoldOptions {
    hanging_faces_only: bool,
    minimum_face_count: usize,
}

fn parse_extract_non_manifold_arguments(
    arguments: &[&str],
) -> Result<ExtractNonManifoldOptions, CommandError> {
    let mut options = ExtractNonManifoldOptions {
        hanging_faces_only: false,
        minimum_face_count: 3,
    };
    let mut hanging_seen = false;
    let mut minimum_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_NON_MANIFOLD_USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        if name.eq_ignore_ascii_case("ExtractHangingFacesOnly") && !hanging_seen {
            options.hanging_faces_only =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_NON_MANIFOLD_USAGE))?;
            hanging_seen = true;
        } else if name.eq_ignore_ascii_case("MinimumFaceCount") && !minimum_seen {
            options.minimum_face_count = value
                .parse::<usize>()
                .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
            if options.minimum_face_count < 3 {
                return Err(GeometryError::InvalidNonManifoldMinimumFaceCount(
                    options.minimum_face_count,
                )
                .into());
            }
            minimum_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_NON_MANIFOLD_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

struct ExtractDuplicateMeshFacesCommand;

impl Command for ExtractDuplicateMeshFacesCommand {
    fn name(&self) -> &'static str {
        "ExtractDuplicateMeshFaces"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "ExtractDuplicateMeshFaces")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let inputs = stage_selected_mesh_face_extractions(
            document,
            || CommandError::UnsupportedExtractDuplicateMeshFacesGeometry,
            |mesh| Ok(mesh.extract_duplicate_faces()),
        )?;
        let counts = mesh_face_extraction_counts(&inputs);
        if counts.extracted_meshes == 0 {
            return Err(CommandError::NoDuplicateMeshFaces);
        }
        apply_mesh_face_extractions(document, inputs, counts.extracted_meshes)?;
        Ok(format!(
            "Extracted {} duplicate face(s) from {} mesh(es); {} mesh(es) unchanged",
            counts.extracted_faces, counts.extracted_meshes, counts.unchanged_meshes
        ))
    }
}

struct GroupCommand;

impl Command for GroupCommand {
    fn name(&self) -> &'static str {
        "Group"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let all = arguments
            .first()
            .is_some_and(|argument| argument.eq_ignore_ascii_case("all"));
        let name_arguments = if all { &arguments[1..] } else { arguments };
        let name = if name_arguments.is_empty() {
            document.next_unused_group_name()
        } else {
            name_arguments.join(" ")
        };
        let members: Vec<_> = if all {
            document
                .objects()
                .filter(|object| document.is_object_selectable(object.id()))
                .map(|object| object.id())
                .collect()
        } else {
            document.selected_object_ids().collect()
        };
        let member_count = members.len();
        let id = document.add_group(Some(name.clone()), members)?;
        Ok(format!(
            "Created group '{name}' {id} with {member_count} object(s)"
        ))
    }
}

const SET_OBJECT_NAME_USAGE: &str = "SetObjectName name [AppendCounter=Yes|No]";

struct SetObjectNameCommand;

impl Command for SetObjectNameCommand {
    fn name(&self) -> &'static str {
        "SetObjectName"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (name, append_counter) = parse_set_object_name_arguments(arguments)?;
        let selected = document
            .objects()
            .filter(|object| document.is_selected(object.id()))
            .map(|object| object.id())
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let has_name = name.is_some();
        let assignments = match name.as_ref() {
            Some(name) if append_counter => selected
                .iter()
                .enumerate()
                .map(|(index, id)| (*id, Some(format!("{name} {index}"))))
                .collect::<Vec<_>>(),
            _ => selected
                .iter()
                .map(|id| (*id, name.clone()))
                .collect::<Vec<_>>(),
        };
        document.set_object_names(assignments)?;
        Ok(if has_name {
            format!("Named {} object(s)", selected.len())
        } else {
            format!("Cleared names on {} object(s)", selected.len())
        })
    }
}

fn parse_set_object_name_arguments(
    arguments: &[&str],
) -> Result<(Option<String>, bool), CommandError> {
    let mut append_counter = false;
    let mut name_parts = Vec::new();
    for argument in arguments {
        let normalized = argument.trim_start_matches('_');
        if let Some((option, value)) = normalized.split_once('=')
            && option.eq_ignore_ascii_case("AppendCounter")
        {
            append_counter = parse_yes_no(value.trim_start_matches('_'))
                .ok_or(CommandError::Usage(SET_OBJECT_NAME_USAGE))?;
        } else {
            name_parts.push(*argument);
        }
    }
    if name_parts.is_empty() {
        return Err(CommandError::Usage(SET_OBJECT_NAME_USAGE));
    }
    let joined = name_parts.join(" ");
    let trimmed = joined.trim();
    let unquoted = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    let name = (!unquoted.trim().is_empty()).then(|| unquoted.trim().to_owned());
    Ok((name, append_counter))
}

struct SetObjectColorCommand;

impl Command for SetObjectColorCommand {
    fn name(&self) -> &'static str {
        "SetObjectColor"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        const USAGE: &str = "SetObjectColor r,g,b | ByLayer";
        let [value] = arguments else {
            return Err(CommandError::Usage(USAGE));
        };
        let selected = selected_ids(document)?;
        let value = value.trim_start_matches('_');
        let color = if value.eq_ignore_ascii_case("ByLayer") {
            None
        } else {
            Some(parse_color(value)?)
        };
        let changed = document.set_objects_color(selected, color)?;
        Ok(match color {
            Some(color) => format!(
                "Set {} object color(s) to {},{},{}",
                changed, color.red, color.green, color.blue
            ),
            None => format!("Set {changed} object color(s) to ByLayer"),
        })
    }
}

struct UngroupCommand;

impl Command for UngroupCommand {
    fn name(&self) -> &'static str {
        "Ungroup"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            let selected: BTreeSet<_> = document.selected_object_ids().collect();
            let groups: Vec<_> = document
                .groups()
                .filter(|group| group.members().any(|member| selected.contains(&member)))
                .map(|group| group.id())
                .collect();
            for group in &groups {
                document.remove_group(*group)?;
            }
            return Ok(format!("Removed {} selected group(s)", groups.len()));
        }
        if arguments.len() == 1 && arguments[0].eq_ignore_ascii_case("all") {
            let groups: Vec<_> = document.groups().map(|group| group.id()).collect();
            for group in &groups {
                document.remove_group(*group)?;
            }
            return Ok(format!("Removed {} group(s)", groups.len()));
        }

        let name = arguments.join(" ");
        let id = document
            .group_by_name(&name)
            .map(|group| group.id())
            .ok_or_else(|| CommandError::NamedGroupNotFound(name.clone()))?;
        let members = document.remove_group(id)?;
        Ok(format!("Removed group '{name}' ({members} object(s))"))
    }
}

struct DeleteCommand;

impl Command for DeleteCommand {
    fn name(&self) -> &'static str {
        "Delete"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Delete")?;
        let selected: Vec<_> = document.selected_object_ids().collect();
        for id in &selected {
            document.delete_object(*id)?;
        }
        Ok(format!("Deleted {} object(s)", selected.len()))
    }
}

struct HideCommand;

impl Command for HideCommand {
    fn name(&self) -> &'static str {
        "Hide"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Hide")?;
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.set_objects_visibility(selected, false)?;
        Ok(format!("Hid {count} object(s)"))
    }
}

struct ShowCommand;

impl Command for ShowCommand {
    fn name(&self) -> &'static str {
        "Show"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Show")?;
        let hidden = document
            .objects()
            .filter(|object| !object.attributes().is_visible())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let count = document.set_objects_visibility(hidden, true)?;
        Ok(format!("Showed {count} object(s)"))
    }
}

struct LockCommand;

impl Command for LockCommand {
    fn name(&self) -> &'static str {
        "Lock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Lock")?;
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.set_objects_locked(selected, true)?;
        Ok(format!("Locked {count} object(s)"))
    }
}

struct UnlockCommand;

impl Command for UnlockCommand {
    fn name(&self) -> &'static str {
        "Unlock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Unlock")?;
        let locked = document
            .objects()
            .filter(|object| object.attributes().is_locked())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let count = document.set_objects_locked(locked, false)?;
        Ok(format!("Unlocked {count} object(s)"))
    }
}

struct HideSwapCommand;

impl Command for HideSwapCommand {
    fn name(&self) -> &'static str {
        "HideSwap"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "HideSwap")?;
        let count = document.swap_object_visibility_modes()?;
        Ok(format!("Swapped hidden state on {count} object(s)"))
    }
}

struct LockSwapCommand;

impl Command for LockSwapCommand {
    fn name(&self) -> &'static str {
        "LockSwap"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "LockSwap")?;
        let count = document.swap_object_lock_modes()?;
        Ok(format!("Swapped lock state on {count} object(s)"))
    }
}

struct IsolateCommand;

impl Command for IsolateCommand {
    fn name(&self) -> &'static str {
        "Isolate"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Isolate")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.isolate_selected_objects()?;
        Ok(format!("Isolated {count} object(s)"))
    }
}

struct UnisolateCommand;

impl Command for UnisolateCommand {
    fn name(&self) -> &'static str {
        "Unisolate"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Unisolate")?;
        let count = document.unisolate_objects()?;
        Ok(format!("Unisolated {count} object(s)"))
    }
}

struct IsolateLockCommand;

impl Command for IsolateLockCommand {
    fn name(&self) -> &'static str {
        "IsolateLock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "IsolateLock")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.isolate_lock_selected_objects()?;
        Ok(format!("Locked {count} non-selected object(s)"))
    }
}

struct UnisolateLockCommand;

impl Command for UnisolateLockCommand {
    fn name(&self) -> &'static str {
        "UnisolateLock"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "UnisolateLock")?;
        let count = document.unisolate_locked_objects()?;
        Ok(format!("Unlocked {count} isolated object(s)"))
    }
}

struct JoinCommand;

impl Command for JoinCommand {
    fn name(&self) -> &'static str {
        "Join"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Join")?;
        let inputs = selected_linear_curves(document)?;
        if inputs.len() < 2 {
            return Err(CommandError::NotEnoughCurvesToJoin);
        }
        let polylines = inputs
            .iter()
            .map(|input| input.polyline.clone())
            .collect::<Vec<_>>();
        let components = join_polylines(&polylines, document.tolerance())?;
        let replacements = components
            .iter()
            .filter(|component| component.source_indices().len() > 1)
            .map(|component| {
                let source_indices = component.source_indices().to_vec();
                let attributes = inputs[source_indices[0]].attributes.clone();
                (source_indices, component.polyline().clone(), attributes)
            })
            .collect::<Vec<_>>();
        if replacements.is_empty() {
            return Err(CommandError::NoJoinableCurves);
        }

        let joined_curve_count = replacements
            .iter()
            .map(|(sources, _, _)| sources.len())
            .sum::<usize>();
        let unchanged = inputs.len() - joined_curve_count;
        let unchanged_ids = components
            .iter()
            .filter(|component| component.source_indices().len() == 1)
            .map(|component| inputs[component.source_indices()[0]].id)
            .collect::<Vec<_>>();
        for (sources, _, _) in &replacements {
            for source in sources {
                document.delete_object(inputs[*source].id)?;
            }
        }
        let mut result_ids = Vec::with_capacity(replacements.len());
        for (_, polyline, attributes) in replacements {
            result_ids.push(
                document.add_geometry_with_attributes(Geometry::Polyline(polyline), attributes)?,
            );
        }
        replace_selection(
            document,
            unchanged_ids.into_iter().chain(result_ids.iter().copied()),
        )?;
        Ok(format!(
            "Joined {joined_curve_count} curve(s) into {} polyline(s); {unchanged} curve(s) unchanged",
            result_ids.len()
        ))
    }
}

#[derive(Clone)]
struct SelectedLinearCurve {
    id: ObjectId,
    polyline: Polyline3,
    attributes: ObjectAttributes,
}

fn selected_linear_curves(document: &Document) -> Result<Vec<SelectedLinearCurve>, CommandError> {
    let mut inputs = Vec::new();
    for object in document.selected_objects() {
        let polyline = match object.geometry() {
            Geometry::Line(line) => {
                Polyline3::try_new(vec![line.start(), line.end()], document.tolerance())?
            }
            Geometry::Polyline(polyline) => polyline.clone(),
            _ => return Err(CommandError::UnsupportedJoinGeometry),
        };
        inputs.push(SelectedLinearCurve {
            id: object.id(),
            polyline,
            attributes: object.attributes().clone(),
        });
    }
    if inputs.is_empty() {
        Err(CommandError::NoObjectsSelected)
    } else {
        Ok(inputs)
    }
}

struct ExplodeCommand;

enum ExplodedParts {
    Lines(Vec<LineSegment>),
    Points(Vec<Point3>),
}

impl Command for ExplodeCommand {
    fn name(&self) -> &'static str {
        "Explode"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["X"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Explode")?;
        let selected = document
            .selected_objects()
            .map(|object| {
                (
                    object.id(),
                    object.geometry().clone(),
                    object.attributes().clone(),
                )
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let exploded = selected
            .iter()
            .filter_map(|(id, geometry, attributes)| match geometry {
                Geometry::Polyline(polyline) => Some((
                    *id,
                    ExplodedParts::Lines(polyline.segments().collect()),
                    attributes.clone(),
                )),
                Geometry::PointCloud(cloud) => Some((
                    *id,
                    ExplodedParts::Points(cloud.points().to_vec()),
                    attributes.clone(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        if exploded.is_empty() {
            return Err(CommandError::NoExplodablePolylines);
        }
        let exploded_ids = exploded
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<BTreeSet<_>>();
        let unchanged_ids = selected
            .iter()
            .filter(|(id, _, _)| !exploded_ids.contains(id))
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        let polyline_count = exploded
            .iter()
            .filter(|(_, parts, _)| matches!(parts, ExplodedParts::Lines(_)))
            .count();
        let point_cloud_count = exploded.len() - polyline_count;
        let line_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Lines(lines) => lines.len(),
                ExplodedParts::Points(_) => 0,
            })
            .sum::<usize>();
        let point_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Lines(_) => 0,
                ExplodedParts::Points(points) => points.len(),
            })
            .sum::<usize>();
        for (id, _, _) in &exploded {
            document.delete_object(*id)?;
        }
        let mut selected_result_ids = Vec::with_capacity(line_count);
        for (_, parts, attributes) in exploded {
            match parts {
                ExplodedParts::Lines(lines) => {
                    for line in lines {
                        selected_result_ids.push(document.add_geometry_with_attributes(
                            Geometry::Line(line),
                            attributes.clone(),
                        )?);
                    }
                }
                ExplodedParts::Points(points) => {
                    for point in points {
                        document.add_geometry_with_attributes(
                            Geometry::Point(point),
                            attributes.clone(),
                        )?;
                    }
                }
            }
        }
        replace_selection(
            document,
            unchanged_ids
                .into_iter()
                .chain(selected_result_ids.iter().copied()),
        )?;
        let unchanged_count = selected.len() - exploded_ids.len();
        Ok(match (polyline_count, point_cloud_count) {
            (0, _) => format!(
                "Exploded {point_cloud_count} point cloud(s) into {point_count} point(s); {unchanged_count} object(s) unchanged"
            ),
            (_, 0) => format!(
                "Exploded {polyline_count} polyline(s) into {line_count} line(s); {unchanged_count} object(s) unchanged"
            ),
            _ => format!(
                "Exploded {polyline_count} polyline(s) into {line_count} line(s) and {point_cloud_count} point cloud(s) into {point_count} point(s); {unchanged_count} object(s) unchanged"
            ),
        })
    }
}

fn replace_selection(
    document: &mut Document,
    ids: impl IntoIterator<Item = ObjectId>,
) -> Result<(), DocumentError> {
    document.select_objects(ids, SelectionMode::Replace)?;
    Ok(())
}

struct MoveCommand;

impl Command for MoveCommand {
    fn name(&self) -> &'static str {
        "Move"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["M"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let offset = parse_translation(arguments, "Move from to")?;
        let count =
            document.transform_objects(selected, AffineTransform3::from_translation(offset))?;
        Ok(format!(
            "Moved {count} object(s) by {}",
            format_vector(offset)
        ))
    }
}

struct CopyCommand;

impl Command for CopyCommand {
    fn name(&self) -> &'static str {
        "Copy"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let offset = parse_translation(arguments, "Copy from to")?;
        let copies = document
            .copy_objects_transformed(selected, AffineTransform3::from_translation(offset))?;
        Ok(format!(
            "Copied {} object(s) by {}",
            copies.len(),
            format_vector(offset)
        ))
    }
}

const ORIENT_USAGE: &str = "Orient reference-start reference-end target-start target-end \
    [Scale=No|1D|3D] [Copy=Yes|No]";
const ORIENT_THREE_POINT_USAGE: &str = "Orient3Pt reference-1 reference-2 reference-3 \
    target-1 target-2 target-3 [Scale=Yes|No] [Copy=Yes|No]";

struct OrientCommand;

impl Command for OrientCommand {
    fn name(&self) -> &'static str {
        "Orient"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (reference_start, consumed_1) = parse_point(arguments)?;
        let (reference_end, consumed_2) = parse_point(&arguments[consumed_1..])?;
        let (target_start, consumed_3) = parse_point(&arguments[consumed_1 + consumed_2..])?;
        let (target_end, consumed_4) =
            parse_point(&arguments[consumed_1 + consumed_2 + consumed_3..])?;
        let consumed = consumed_1 + consumed_2 + consumed_3 + consumed_4;
        let options = parse_orient_options(&arguments[consumed..])?;
        let tolerance = document.tolerance();
        let reference_vector = reference_start.vector_to(reference_end)?;
        let target_vector = target_start.vector_to(target_end)?;
        let reference_direction = reference_vector.normalized(tolerance)?;
        let target_direction = target_vector.normalized(tolerance)?;
        let scale_factor = match options.scale {
            OrientScale::No => 1.0,
            OrientScale::OneDimensional | OrientScale::ThreeDimensional => {
                orient_scale_factor(reference_vector, target_vector)?
            }
        };
        let perpendicular_scale = match options.scale {
            OrientScale::ThreeDimensional => scale_factor,
            OrientScale::No | OrientScale::OneDimensional => 1.0,
        };
        let transform = AffineTransform3::try_direction_mapping(
            reference_start,
            reference_direction,
            target_start,
            target_direction,
            scale_factor,
            perpendicular_scale,
            tolerance,
        )?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, options.copy)?;
        Ok(format!(
            "Oriented {transformed} object(s) with {} scaling, creating {copied} copy object(s)",
            options.scale.name()
        ))
    }
}

struct OrientThreePointCommand;

impl Command for OrientThreePointCommand {
    fn name(&self) -> &'static str {
        "Orient3Pt"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Orient3Point"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (reference_1, consumed_1) = parse_point(arguments)?;
        let (reference_2, consumed_2) = parse_point(&arguments[consumed_1..])?;
        let (reference_3, consumed_3) = parse_point(&arguments[consumed_1 + consumed_2..])?;
        let (target_1, consumed_4) =
            parse_point(&arguments[consumed_1 + consumed_2 + consumed_3..])?;
        let (target_2, consumed_5) =
            parse_point(&arguments[consumed_1 + consumed_2 + consumed_3 + consumed_4..])?;
        let (target_3, consumed_6) = parse_point(
            &arguments[consumed_1 + consumed_2 + consumed_3 + consumed_4 + consumed_5..],
        )?;
        let consumed = consumed_1 + consumed_2 + consumed_3 + consumed_4 + consumed_5 + consumed_6;
        let options = parse_orient_three_point_options(&arguments[consumed..])?;
        let tolerance = document.tolerance();
        let source_frame =
            Frame3::try_from_points(reference_1, reference_2, reference_3, tolerance)?;
        let target_frame = Frame3::try_from_points(target_1, target_2, target_3, tolerance)?;
        let scale_factor = if options.scale {
            orient_scale_factor(
                reference_1.vector_to(reference_2)?,
                target_1.vector_to(target_2)?,
            )?
        } else {
            1.0
        };
        let transform =
            AffineTransform3::try_frame_mapping(source_frame, target_frame, [scale_factor; 3])?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, options.copy)?;
        let scale_name = if options.scale { "3D" } else { "No" };
        Ok(format!(
            "Oriented {transformed} object(s) through three-point frames with {scale_name} scaling, creating {copied} copy object(s)"
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrientScale {
    No,
    OneDimensional,
    ThreeDimensional,
}

impl OrientScale {
    const fn name(self) -> &'static str {
        match self {
            Self::No => "No",
            Self::OneDimensional => "1D",
            Self::ThreeDimensional => "3D",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OrientOptions {
    scale: OrientScale,
    copy: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OrientThreePointOptions {
    scale: bool,
    copy: bool,
}

fn parse_orient_options(arguments: &[&str]) -> Result<OrientOptions, CommandError> {
    let mut options = OrientOptions {
        scale: OrientScale::No,
        copy: false,
    };
    let mut scale_seen = false;
    let mut copy_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, ORIENT_USAGE)?;
        if option_name_eq(name, "Scale") && !scale_seen {
            let value = value.trim_start_matches(['_', '-']);
            options.scale = if value.eq_ignore_ascii_case("No") {
                OrientScale::No
            } else if value.eq_ignore_ascii_case("1D") {
                OrientScale::OneDimensional
            } else if value.eq_ignore_ascii_case("3D") {
                OrientScale::ThreeDimensional
            } else {
                return Err(CommandError::Usage(ORIENT_USAGE));
            };
            scale_seen = true;
        } else if option_name_eq(name, "Copy") && !copy_seen {
            options.copy = parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_USAGE))?;
            copy_seen = true;
        } else {
            return Err(CommandError::Usage(ORIENT_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

fn parse_orient_three_point_options(
    arguments: &[&str],
) -> Result<OrientThreePointOptions, CommandError> {
    let mut options = OrientThreePointOptions {
        scale: false,
        copy: false,
    };
    let mut scale_seen = false;
    let mut copy_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, ORIENT_THREE_POINT_USAGE)?;
        if option_name_eq(name, "Scale") && !scale_seen {
            options.scale =
                parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_THREE_POINT_USAGE))?;
            scale_seen = true;
        } else if option_name_eq(name, "Copy") && !copy_seen {
            options.copy =
                parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_THREE_POINT_USAGE))?;
            copy_seen = true;
        } else {
            return Err(CommandError::Usage(ORIENT_THREE_POINT_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

fn orient_option<'a>(
    arguments: &'a [&str],
    index: usize,
    usage: &'static str,
) -> Result<(&'a str, &'a str, usize), CommandError> {
    let argument = arguments[index];
    if let Some((name, value)) = argument.split_once('=') {
        Ok((name, value, 1))
    } else {
        let value = arguments.get(index + 1).ok_or(CommandError::Usage(usage))?;
        Ok((argument, *value, 2))
    }
}

fn orient_scale_factor(reference: Vector3, target: Vector3) -> Result<Real, GeometryError> {
    let factor = target.length()? / reference.length()?;
    if factor.is_finite() && factor > 0.0 {
        Ok(factor)
    } else {
        Err(GeometryError::NonFinite {
            context: "orient scale factor",
        })
    }
}

fn apply_transform_or_copy(
    document: &mut Document,
    selected: &[ObjectId],
    transform: AffineTransform3,
    copy: bool,
) -> Result<(usize, usize), DocumentError> {
    if copy {
        let copies = document.copy_objects_transformed(selected.iter().copied(), transform)?;
        document.select_objects_direct(selected.iter().copied(), SelectionMode::Replace)?;
        Ok((selected.len(), copies.len()))
    } else {
        let transformed = document.transform_objects(selected.iter().copied(), transform)?;
        Ok((transformed, 0))
    }
}

const ORIENT_ON_SURFACE_USAGE: &str = "OrientOnSrf base-point reference-point target-point \
    [Scale=factor] [Rotation=degrees] [Copy=Yes|No] [Rigid=Yes|No] [Flip=Yes|No] \
    [SourceNormal=x,y,z] [ConstrainNormal=Yes|No] [IgnoreTrims=Yes|No] [SurfaceName=name]";

struct OrientOnSurfaceCommand;

impl Command for OrientOnSurfaceCommand {
    fn name(&self) -> &'static str {
        "OrientOnSrf"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["OrientOnSurface"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (base_point, consumed_1) = parse_point(arguments)?;
        let (reference_point, consumed_2) = parse_point(&arguments[consumed_1..])?;
        let (target_point, consumed_3) = parse_point(&arguments[consumed_1 + consumed_2..])?;
        let consumed = consumed_1 + consumed_2 + consumed_3;
        let options = parse_orient_on_surface_options(&arguments[consumed..])?;
        let (surface, surface_id, sources) =
            orient_on_surface_inputs(document, options.surface_name.as_deref())?;
        let tolerance = document.tolerance();
        let source_frame = Frame3::try_from_x_and_normal(
            base_point,
            base_point.vector_to(reference_point)?,
            options.source_normal,
            tolerance,
        )?;
        let (target_u, target_v) = surface.closest_parameters(target_point, tolerance)?;
        let rotation_radians = options.rotation_degrees.to_radians();
        if !rotation_radians.is_finite() {
            return Err(CommandError::InvalidNumber(
                options.rotation_degrees.to_string(),
            ));
        }

        let (transformed, copied) = if options.rigid {
            if options.constrain_normal {
                return Err(CommandError::Usage(ORIENT_ON_SURFACE_USAGE));
            }
            let target_frame = orient_on_surface_target_frame(
                surface.frame_at(target_u, target_v, tolerance)?,
                rotation_radians,
                options.flip,
                tolerance,
            )?;
            let transform = AffineTransform3::try_frame_mapping(
                source_frame,
                target_frame,
                [options.scale; 3],
            )?;
            apply_transform_or_copy(document, sources.as_slice(), transform, options.copy)?
        } else {
            let morph = SurfacePointMorph::try_new(
                source_frame,
                &surface,
                target_u,
                target_v,
                options.scale,
                rotation_radians,
                options.flip,
                tolerance,
            )?;
            if options.constrain_normal {
                // The standalone command has no viewport state yet, so its
                // source construction-plane normal is also the placement
                // construction-plane normal used by Rhino's toggle.
                let morph = morph.with_constrained_normal(options.source_normal)?;
                apply_orient_surface_morph(document, sources.as_slice(), &morph, options.copy)?
            } else {
                apply_orient_surface_morph(document, sources.as_slice(), &morph, options.copy)?
            }
        };
        document.select_objects_direct(sources.iter().copied(), SelectionMode::Replace)?;
        debug_assert!(!document.is_selected(surface_id));
        Ok(format!(
            "Oriented {transformed} object(s) onto surface parameters {target_u:.6},{target_v:.6} with {} placement, creating {copied} copy object(s)",
            if options.rigid { "rigid" } else { "deformable" }
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct OrientOnSurfaceOptions {
    scale: Real,
    rotation_degrees: Real,
    copy: bool,
    rigid: bool,
    flip: bool,
    source_normal: Vector3,
    constrain_normal: bool,
    surface_name: Option<String>,
}

fn parse_orient_on_surface_options(
    arguments: &[&str],
) -> Result<OrientOnSurfaceOptions, CommandError> {
    let mut options = OrientOnSurfaceOptions {
        scale: 1.0,
        rotation_degrees: 0.0,
        copy: true,
        rigid: true,
        flip: false,
        source_normal: Vector3::try_new(0.0, 0.0, 1.0).expect("the world z direction is finite"),
        constrain_normal: false,
        surface_name: None,
    };
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, ORIENT_ON_SURFACE_USAGE)?;
        let normalized_name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if !seen.insert(normalized_name.clone()) {
            return Err(CommandError::Usage(ORIENT_ON_SURFACE_USAGE));
        }
        match normalized_name.as_str() {
            "scale" => {
                options.scale = parse_finite_real(value)?;
                if options.scale <= 0.0 {
                    return Err(CommandError::InvalidScaleFactor(value.to_owned()));
                }
            }
            "rotation" | "angle" => options.rotation_degrees = parse_finite_real(value)?,
            "copy" => {
                options.copy =
                    parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_ON_SURFACE_USAGE))?;
            }
            "rigid" => {
                options.rigid =
                    parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_ON_SURFACE_USAGE))?;
            }
            "flip" => {
                options.flip =
                    parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_ON_SURFACE_USAGE))?;
            }
            "sourcenormal" | "up" => {
                options.source_normal = Vector3::try_from(
                    parse_single_option_point(value, ORIENT_ON_SURFACE_USAGE)?.to_array(),
                )?;
            }
            "constrainnormal" => {
                options.constrain_normal =
                    parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_ON_SURFACE_USAGE))?;
            }
            "ignoretrims" => {
                parse_yes_no(value).ok_or(CommandError::Usage(ORIENT_ON_SURFACE_USAGE))?;
            }
            "surfacename" if !value.is_empty() => options.surface_name = Some(value.to_owned()),
            _ => return Err(CommandError::Usage(ORIENT_ON_SURFACE_USAGE)),
        }
        index += consumed;
    }
    Ok(options)
}

fn orient_on_surface_inputs(
    document: &Document,
    surface_name: Option<&str>,
) -> Result<(NurbsSurface, ObjectId, Vec<ObjectId>), CommandError> {
    let selected = selected_ids(document)?;
    let surface_id = if let Some(name) = surface_name {
        let matches = document
            .objects()
            .filter(|object| {
                object
                    .attributes()
                    .name()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
            .map(|object| object.id())
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => return Err(CommandError::SurfaceOrientTargetNotFound(name.to_owned())),
            [id] => *id,
            _ => return Err(CommandError::AmbiguousSurfaceOrientTarget(name.to_owned())),
        }
    } else {
        *selected
            .last()
            .ok_or(CommandError::SurfaceOrientTargetRequired)?
    };
    let target = document
        .object(surface_id)
        .expect("resolved surface-orient target identifiers are present");
    let Geometry::NurbsSurface(surface) = target.geometry() else {
        return Err(CommandError::SurfaceOrientTargetNotSurface);
    };
    let sources = selected
        .into_iter()
        .filter(|id| *id != surface_id)
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::SurfaceOrientSourcesRequired);
    }
    Ok((surface.clone(), surface_id, sources))
}

fn orient_on_surface_target_frame(
    frame: Frame3,
    rotation_radians: Real,
    flip: bool,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let (sine, cosine) = rotation_radians.sin_cos();
    let y_sign = if flip { -1.0 } else { 1.0 };
    let x = combine_frame_axes(frame.x_axis(), frame.y_axis(), cosine, sine * y_sign)?;
    let y = combine_frame_axes(frame.x_axis(), frame.y_axis(), -sine, cosine * y_sign)?;
    Frame3::try_from_directions(frame.origin(), x, y, tolerance)
}

fn combine_frame_axes(
    first: UnitVector3,
    second: UnitVector3,
    first_scale: Real,
    second_scale: Real,
) -> Result<Vector3, GeometryError> {
    let first = first.as_vector().to_array();
    let second = second.as_vector().to_array();
    Vector3::try_new(
        first_scale.mul_add(first[0], second_scale * second[0]),
        first_scale.mul_add(first[1], second_scale * second[1]),
        first_scale.mul_add(first[2], second_scale * second[2]),
    )
}

fn apply_orient_surface_morph(
    document: &mut Document,
    selected: &[ObjectId],
    morph: &SurfacePointMorph<'_>,
    copy: bool,
) -> Result<(usize, usize), DocumentError> {
    if copy {
        let copies = document.copy_objects_morphed(selected.iter().copied(), morph)?;
        Ok((selected.len(), copies.len()))
    } else {
        let transformed = document.morph_objects(selected.iter().copied(), morph)?;
        Ok((transformed, 0))
    }
}

const ARRAY_USAGE: &str =
    "Array x-count y-count z-count x-distance y-distance z-distance [Mode=UnitCell|Fill]";

struct ArrayCommand;

impl Command for ArrayCommand {
    fn name(&self) -> &'static str {
        "Array"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ArrayRectangular"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.len() < 6 {
            return Err(CommandError::Usage(ARRAY_USAGE));
        }
        let counts = [
            parse_array_dimension_count(arguments[0])?,
            parse_array_dimension_count(arguments[1])?,
            parse_array_dimension_count(arguments[2])?,
        ];
        let distances = [
            parse_finite_real(arguments[3])?,
            parse_finite_real(arguments[4])?,
            parse_finite_real(arguments[5])?,
        ];
        let mode = parse_rectangular_array_mode(&arguments[6..])?;
        let selected = selected_ids(document)?;
        let source_count = selected.len();
        let cell_count = counts
            .into_iter()
            .try_fold(1_usize, |product, count| product.checked_mul(count))
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let copy_instance_count = cell_count - 1;
        let copy_count = selected
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let spacing = match mode {
            RectangularArrayMode::UnitCell => distances,
            RectangularArrayMode::Fill => rectangular_fill_spacing(
                selected_geometry_bounds(document, &selected)?,
                counts,
                distances,
            )?,
        };
        let mut transforms = Vec::new();
        transforms
            .try_reserve_exact(copy_instance_count)
            .map_err(|_| CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        for z_index in 0..counts[2] {
            for y_index in 0..counts[1] {
                for x_index in 0..counts[0] {
                    if x_index == 0 && y_index == 0 && z_index == 0 {
                        continue;
                    }
                    transforms.push(AffineTransform3::from_translation(Vector3::try_new(
                        spacing[0] * x_index as Real,
                        spacing[1] * y_index as Real,
                        spacing[2] * z_index as Real,
                    )?));
                }
            }
        }
        let copies = document
            .copy_objects_with_transforms(selected.iter().copied(), transforms.as_slice())?;
        document.select_objects_direct(selected, SelectionMode::Replace)?;
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {}×{}×{} cells using {} distances {:.6},{:.6},{:.6}, creating {copy_count} copy object(s)",
            source_count,
            counts[0],
            counts[1],
            counts[2],
            mode.name(),
            distances[0],
            distances[1],
            distances[2],
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RectangularArrayMode {
    UnitCell,
    Fill,
}

impl RectangularArrayMode {
    const fn name(self) -> &'static str {
        match self {
            Self::UnitCell => "UnitCell",
            Self::Fill => "Fill",
        }
    }
}

fn parse_rectangular_array_mode(arguments: &[&str]) -> Result<RectangularArrayMode, CommandError> {
    if arguments.is_empty() {
        return Ok(RectangularArrayMode::UnitCell);
    }
    let (name, value) = match arguments {
        [option] => option
            .split_once('=')
            .ok_or(CommandError::Usage(ARRAY_USAGE))?,
        [name, value] => (*name, *value),
        _ => return Err(CommandError::Usage(ARRAY_USAGE)),
    };
    if !name.trim_start_matches('_').eq_ignore_ascii_case("Mode") {
        return Err(CommandError::Usage(ARRAY_USAGE));
    }
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("UnitCell") {
        Ok(RectangularArrayMode::UnitCell)
    } else if value.eq_ignore_ascii_case("Fill") {
        Ok(RectangularArrayMode::Fill)
    } else {
        Err(CommandError::Usage(ARRAY_USAGE))
    }
}

fn parse_array_dimension_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 1)
        .ok_or_else(|| CommandError::InvalidArrayDimensionCount(value.to_owned()))
}

fn rectangular_fill_spacing(
    bounds: BoundingBox3,
    counts: [usize; 3],
    lengths: [Real; 3],
) -> Result<[Real; 3], CommandError> {
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let mut spacing = [0.0; 3];
    for axis in 0..3 {
        if counts[axis] == 1 {
            continue;
        }
        let object_extent = max[axis] - min[axis];
        if !object_extent.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "rectangular array bounds",
            }
            .into());
        }
        if lengths[axis].abs() < object_extent {
            return Err(CommandError::ArrayFillLengthTooSmall {
                axis: ["X", "Y", "Z"][axis],
                minimum: object_extent,
            });
        }
        spacing[axis] = lengths[axis].signum() * (lengths[axis].abs() - object_extent)
            / (counts[axis] - 1) as Real;
    }
    Ok(spacing)
}

const ARRAY_CURVE_USAGE: &str = "ArrayCrv item-count | ArrayCrv Items item-count | \
    ArrayCrv Distance spacing [Orientation=Freeform|Roadlike|Stairlike|NoRotation] \
    [BasePoint=x,y,z] [PathName=name]";

struct ArrayCurveCommand;

impl Command for ArrayCurveCommand {
    fn name(&self) -> &'static str {
        "ArrayCrv"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ArrayCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_curve_array_options(arguments)?;
        let (path_geometry, path_id, sources) =
            curve_array_inputs(document, options.path_name.as_deref())?;
        let tolerance = document.tolerance();
        let curve = geometry_curve_ref(&path_geometry)
            .expect("curve-array input resolution validates the path geometry");
        let samples = curve_array_samples(curve, options.spacing, tolerance)?;
        let retained_source_instance = usize::from(options.base_point.is_none());
        let copy_instance_count = samples.len() - retained_source_instance;
        let copy_count = sources
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let base_point = options.base_point.unwrap_or_else(|| samples[0].point());
        let transforms = curve_array_transforms(
            curve,
            &samples,
            base_point,
            options.orientation,
            retained_source_instance,
            tolerance,
        )?;
        debug_assert_eq!(transforms.len(), copy_instance_count);
        let copies = document
            .copy_objects_with_transforms(sources.iter().copied(), transforms.as_slice())?;
        document.select_objects_direct(sources.iter().copied(), SelectionMode::Replace)?;
        debug_assert!(!document.is_selected(path_id));
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) at {} {} location(s) with {} orientation, creating {copy_count} copy object(s)",
            sources.len(),
            samples.len(),
            options.spacing.name(),
            options.orientation.name(),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CurveArraySpacing {
    Items(usize),
    Distance(Real),
}

impl CurveArraySpacing {
    const fn name(self) -> &'static str {
        match self {
            Self::Items(_) => "item",
            Self::Distance(_) => "distance",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CurveArrayOrientation {
    Freeform,
    Roadlike,
    Stairlike,
    NoRotation,
}

impl CurveArrayOrientation {
    const fn name(self) -> &'static str {
        match self {
            Self::Freeform => "Freeform",
            Self::Roadlike => "Roadlike",
            Self::Stairlike => "Stairlike",
            Self::NoRotation => "NoRotation",
        }
    }
}

#[derive(Debug, PartialEq)]
struct CurveArrayOptions {
    spacing: CurveArraySpacing,
    orientation: CurveArrayOrientation,
    base_point: Option<Point3>,
    path_name: Option<String>,
}

fn parse_curve_array_options(arguments: &[&str]) -> Result<CurveArrayOptions, CommandError> {
    let first = arguments
        .first()
        .ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?;
    let (spacing, mut index) = if let Some((name, value)) = first.split_once('=') {
        (parse_curve_array_spacing(name, value)?, 1)
    } else if option_name_eq(first, "Items") || option_name_eq(first, "Distance") {
        let value = arguments
            .get(1)
            .ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?;
        (parse_curve_array_spacing(first, value)?, 2)
    } else {
        (
            CurveArraySpacing::Items(parse_curve_array_item_count(first)?),
            1,
        )
    };
    let mut orientation = CurveArrayOrientation::Freeform;
    let mut base_point = None;
    let mut path_name = None;
    let mut orientation_seen = false;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?;
            (argument, *value, 2)
        };
        if option_name_eq(name, "Orientation") && !orientation_seen {
            orientation = parse_curve_array_orientation(value)?;
            orientation_seen = true;
        } else if option_name_eq(name, "BasePoint") && base_point.is_none() {
            let (point, point_consumed) = parse_point(&[value])?;
            if point_consumed != 1 {
                return Err(CommandError::Usage(ARRAY_CURVE_USAGE));
            }
            base_point = Some(point);
        } else if option_name_eq(name, "PathName") && path_name.is_none() && !value.is_empty() {
            path_name = Some(value.to_owned());
        } else {
            return Err(CommandError::Usage(ARRAY_CURVE_USAGE));
        }
        index += consumed;
    }
    Ok(CurveArrayOptions {
        spacing,
        orientation,
        base_point,
        path_name,
    })
}

fn option_name_eq(actual: &str, expected: &str) -> bool {
    actual
        .trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case(expected)
}

fn parse_curve_array_spacing(name: &str, value: &str) -> Result<CurveArraySpacing, CommandError> {
    if option_name_eq(name, "Items") {
        Ok(CurveArraySpacing::Items(parse_curve_array_item_count(
            value,
        )?))
    } else if option_name_eq(name, "Distance") {
        let distance = parse_finite_real(value)?;
        if distance <= 0.0 {
            return Err(CommandError::InvalidCurveArrayDistance(value.to_owned()));
        }
        Ok(CurveArraySpacing::Distance(distance))
    } else {
        Err(CommandError::Usage(ARRAY_CURVE_USAGE))
    }
}

fn parse_curve_array_item_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 1)
        .ok_or_else(|| CommandError::InvalidCurveArrayItemCount(value.to_owned()))
}

fn parse_curve_array_orientation(value: &str) -> Result<CurveArrayOrientation, CommandError> {
    let value = value.trim_start_matches(['_', '-']);
    if value.eq_ignore_ascii_case("Freeform") {
        Ok(CurveArrayOrientation::Freeform)
    } else if value.eq_ignore_ascii_case("Roadlike") {
        Ok(CurveArrayOrientation::Roadlike)
    } else if value.eq_ignore_ascii_case("Stairlike") {
        Ok(CurveArrayOrientation::Stairlike)
    } else if value.eq_ignore_ascii_case("NoRotation") {
        Ok(CurveArrayOrientation::NoRotation)
    } else {
        Err(CommandError::Usage(ARRAY_CURVE_USAGE))
    }
}

fn curve_array_inputs(
    document: &Document,
    path_name: Option<&str>,
) -> Result<(Geometry, ObjectId, Vec<ObjectId>), CommandError> {
    let selected = selected_ids(document)?;
    let path_id = if let Some(name) = path_name {
        let matches = document
            .objects()
            .filter(|object| {
                object
                    .attributes()
                    .name()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
            .map(|object| object.id())
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => return Err(CommandError::CurveArrayPathNotFound(name.to_owned())),
            [id] => *id,
            _ => return Err(CommandError::AmbiguousCurveArrayPath(name.to_owned())),
        }
    } else {
        *selected
            .last()
            .ok_or(CommandError::CurveArrayPathRequired)?
    };
    let path = document
        .object(path_id)
        .expect("resolved curve-array path identifiers are present");
    if geometry_curve_ref(path.geometry()).is_none() {
        return Err(CommandError::CurveArrayPathNotCurve);
    }
    let sources = selected
        .into_iter()
        .filter(|id| *id != path_id)
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::CurveArraySourcesRequired);
    }
    Ok((path.geometry().clone(), path_id, sources))
}

fn curve_array_samples(
    curve: CurveRef<'_>,
    spacing: CurveArraySpacing,
    tolerance: Tolerance,
) -> Result<Vec<CurveSample>, CommandError> {
    let closed = curve.is_closed()?;
    let mut samples = match spacing {
        CurveArraySpacing::Items(1) => vec![curve.start_sample(tolerance)?],
        CurveArraySpacing::Items(item_count) if closed => {
            let mut samples = curve.divide_by_count_samples(item_count, true, tolerance)?;
            samples.pop();
            samples
        }
        CurveArraySpacing::Items(item_count) => {
            curve.divide_by_count_samples(item_count - 1, true, tolerance)?
        }
        CurveArraySpacing::Distance(distance) => {
            let mut samples = curve.divide_by_length_samples(distance, true, tolerance)?;
            if closed
                && samples.len() > 1
                && samples[0].point().is_near(
                    samples.last().expect("the start sample is present").point(),
                    tolerance,
                )
            {
                samples.pop();
            }
            samples
        }
    };
    if samples.is_empty() {
        samples.push(curve.start_sample(tolerance)?);
    }
    Ok(samples)
}

fn curve_array_transforms(
    curve: CurveRef<'_>,
    samples: &[CurveSample],
    base_point: Point3,
    orientation: CurveArrayOrientation,
    skip: usize,
    tolerance: Tolerance,
) -> Result<Vec<AffineTransform3>, GeometryError> {
    if orientation == CurveArrayOrientation::NoRotation {
        return samples[skip..]
            .iter()
            .map(|sample| {
                base_point
                    .vector_to(sample.point())
                    .map(AffineTransform3::from_translation)
            })
            .collect();
    }
    let frames = curve_array_frames(curve, samples, orientation, tolerance)?;
    let source_frame = frames[0];
    samples[skip..]
        .iter()
        .zip(&frames[skip..])
        .map(|(sample, frame)| {
            curve_array_frame_transform(base_point, source_frame, sample.point(), *frame, tolerance)
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct CurveArrayFrame {
    x: UnitVector3,
    y: UnitVector3,
    z: UnitVector3,
}

fn curve_array_frames(
    curve: CurveRef<'_>,
    samples: &[CurveSample],
    orientation: CurveArrayOrientation,
    tolerance: Tolerance,
) -> Result<Vec<CurveArrayFrame>, GeometryError> {
    match orientation {
        CurveArrayOrientation::Freeform => freeform_curve_array_frames(curve, samples),
        CurveArrayOrientation::Roadlike => {
            let mut previous = None;
            samples
                .iter()
                .map(|sample| {
                    let frame = roadlike_curve_array_frame(sample.tangent(), previous, tolerance)?;
                    previous = Some(frame);
                    Ok(frame)
                })
                .collect()
        }
        CurveArrayOrientation::Stairlike => {
            let mut previous = None;
            samples
                .iter()
                .map(|sample| {
                    let frame = stairlike_curve_array_frame(sample.tangent(), previous, tolerance)?;
                    previous = Some(frame);
                    Ok(frame)
                })
                .collect()
        }
        CurveArrayOrientation::NoRotation => unreachable!("handled before frame construction"),
    }
}

fn freeform_curve_array_frames(
    curve: CurveRef<'_>,
    samples: &[CurveSample],
) -> Result<Vec<CurveArrayFrame>, GeometryError> {
    let mut frame = stable_curve_array_frame(samples[0].tangent())?;
    let mut previous_sample = samples[0];
    let mut frames = Vec::with_capacity(samples.len());
    frames.push(frame);
    let interval_count = samples.len().saturating_sub(1);
    if interval_count == 0 {
        return Ok(frames);
    }
    // The method has fourth-order global error. Keep roughly 256 transport
    // steps for sparse arrays without multiplying already-dense arrays.
    let subdivisions = 256_usize.div_ceil(interval_count).clamp(1, 64);
    for pair in samples.windows(2) {
        for step in 1..=subdivisions {
            let next_sample = if step == subdivisions {
                pair[1]
            } else {
                let fraction = step as Real / subdivisions as Real;
                let parameter = pair[0]
                    .parameter()
                    .mul_add(1.0 - fraction, pair[1].parameter() * fraction);
                curve.evaluate_with_tangent(parameter)?
            };
            frame = double_reflect_curve_array_frame(frame, previous_sample, next_sample)?;
            previous_sample = next_sample;
        }
        frames.push(frame);
    }
    Ok(frames)
}

fn stable_curve_array_frame(tangent: UnitVector3) -> Result<CurveArrayFrame, GeometryError> {
    let [x, y, z] = tangent.as_vector().to_array().map(Real::abs);
    let reference = if x <= y && x <= z {
        Vector3::try_new(1.0, 0.0, 0.0)?
    } else if y <= z {
        Vector3::try_new(0.0, 1.0, 0.0)?
    } else {
        Vector3::try_new(0.0, 0.0, 1.0)?
    };
    let frame_x = reference.cross(tangent.as_vector())?.normalized_nonzero()?;
    let frame_y = tangent
        .as_vector()
        .cross(frame_x.as_vector())?
        .normalized_nonzero()?;
    Ok(CurveArrayFrame {
        x: frame_x,
        y: frame_y,
        z: tangent,
    })
}

/// Advances a rotation-minimizing frame with the two plane reflections from
/// Wang et al. The chord reflection maps the old frame to a left-handed one;
/// the tangent-bisector reflection restores handedness at the new tangent.
fn double_reflect_curve_array_frame(
    previous: CurveArrayFrame,
    previous_sample: CurveSample,
    next_sample: CurveSample,
) -> Result<CurveArrayFrame, GeometryError> {
    let chord = previous_sample.point().vector_to(next_sample.point())?;
    let reflected_x = reflect_curve_array_vector(previous.x.as_vector(), chord)?;
    let reflected_tangent = reflect_curve_array_vector(previous.z.as_vector(), chord)?;
    let tangent = next_sample.tangent();
    let tangent_bisector = subtract_vectors(tangent.as_vector(), reflected_tangent)?;
    let transported_x = if vector_is_zero(tangent_bisector) {
        // The second reflection is undefined only for degenerate local data.
        // Sufficiently dense regular-curve samples avoid it, but retaining the
        // first reflected reference vector gives a deterministic safe limit.
        reflected_x
    } else {
        reflect_curve_array_vector(reflected_x, tangent_bisector)?
    };
    let provisional_x = transported_x.normalized_nonzero()?;
    let frame_y = tangent
        .as_vector()
        .cross(provisional_x.as_vector())?
        .normalized_nonzero()?;
    let frame_x = frame_y
        .as_vector()
        .cross(tangent.as_vector())?
        .normalized_nonzero()?;
    Ok(CurveArrayFrame {
        x: frame_x,
        y: frame_y,
        z: tangent,
    })
}

fn reflect_curve_array_vector(
    vector: Vector3,
    reflection_normal: Vector3,
) -> Result<Vector3, GeometryError> {
    let scale = reflection_normal
        .x()
        .abs()
        .max(reflection_normal.y().abs())
        .max(reflection_normal.z().abs());
    if scale == 0.0 {
        return Err(GeometryError::Degenerate {
            context: "curve-array reflection",
        });
    }
    let normal = Vector3::try_new(
        reflection_normal.x() / scale,
        reflection_normal.y() / scale,
        reflection_normal.z() / scale,
    )?;
    let denominator = normal.dot(normal)?;
    let projection_scale = 2.0 * vector.dot(normal)? / denominator;
    subtract_vectors(vector, normal.scaled(projection_scale)?)
}

fn vector_is_zero(vector: Vector3) -> bool {
    vector.x() == 0.0 && vector.y() == 0.0 && vector.z() == 0.0
}

fn subtract_vectors(left: Vector3, right: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        left.x() - right.x(),
        left.y() - right.y(),
        left.z() - right.z(),
    )
}

fn roadlike_curve_array_frame(
    tangent: UnitVector3,
    previous: Option<CurveArrayFrame>,
    tolerance: Tolerance,
) -> Result<CurveArrayFrame, GeometryError> {
    let up = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let horizontal_y = up.as_vector().cross(tangent.as_vector())?;
    let provisional_y = if horizontal_y.length()? > tolerance.angular() {
        horizontal_y.normalized_nonzero()?
    } else if let Some(frame) = previous {
        frame.y
    } else {
        Vector3::try_new(0.0, 1.0, 0.0)?.normalized_nonzero()?
    };
    let frame_x = provisional_y
        .as_vector()
        .cross(tangent.as_vector())?
        .normalized_nonzero()?;
    let frame_y = tangent
        .as_vector()
        .cross(frame_x.as_vector())?
        .normalized_nonzero()?;
    Ok(CurveArrayFrame {
        x: frame_x,
        y: frame_y,
        z: tangent,
    })
}

fn stairlike_curve_array_frame(
    tangent: UnitVector3,
    previous: Option<CurveArrayFrame>,
    tolerance: Tolerance,
) -> Result<CurveArrayFrame, GeometryError> {
    let up = Vector3::try_new(0.0, 0.0, 1.0)?.normalized_nonzero()?;
    let horizontal_tangent = Vector3::try_new(tangent.x(), tangent.y(), 0.0)?;
    let frame_x = if horizontal_tangent.length()? > tolerance.angular() {
        horizontal_tangent.normalized_nonzero()?
    } else if let Some(frame) = previous {
        frame.x
    } else {
        Vector3::try_new(1.0, 0.0, 0.0)?.normalized_nonzero()?
    };
    let frame_y = up
        .as_vector()
        .cross(frame_x.as_vector())?
        .normalized_nonzero()?;
    Ok(CurveArrayFrame {
        x: frame_x,
        y: frame_y,
        z: up,
    })
}

fn curve_array_frame_transform(
    base_point: Point3,
    source: CurveArrayFrame,
    target_point: Point3,
    target: CurveArrayFrame,
    tolerance: Tolerance,
) -> Result<AffineTransform3, GeometryError> {
    let source = Frame3::try_from_directions(
        base_point,
        source.x.as_vector(),
        source.y.as_vector(),
        tolerance,
    )?;
    let target = Frame3::try_from_directions(
        target_point,
        target.x.as_vector(),
        target.y.as_vector(),
        tolerance,
    )?;
    AffineTransform3::try_frame_mapping(source, target, [1.0; 3])
}

const ARRAY_SURFACE_USAGE: &str = "ArraySrf u-count v-count BasePoint=x,y,z \
    [Up=x,y,z] [Mode=UV|Isocurve] [SurfaceName=name]";

struct ArraySurfaceCommand;

impl Command for ArraySurfaceCommand {
    fn name(&self) -> &'static str {
        "ArraySrf"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ArraySurface"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let u_count = parse_surface_array_count(
            arguments
                .first()
                .ok_or(CommandError::Usage(ARRAY_SURFACE_USAGE))?,
        )?;
        let v_count = parse_surface_array_count(
            arguments
                .get(1)
                .ok_or(CommandError::Usage(ARRAY_SURFACE_USAGE))?,
        )?;
        let options = parse_surface_array_options(&arguments[2..])?;
        let (surface, surface_id, sources) =
            surface_array_inputs(document, options.surface_name.as_deref())?;
        let cell_count = u_count
            .checked_mul(v_count)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let copy_count = sources
            .len()
            .checked_mul(cell_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let tolerance = document.tolerance();
        let source_frame = Frame3::try_from_normal(options.base_point, options.up, tolerance)?;
        let (u_parameters, v_parameters) =
            surface_array_parameters(&surface, u_count, v_count, options.mode, tolerance)?;
        let mut transforms = Vec::new();
        transforms.try_reserve_exact(cell_count).map_err(|_| {
            CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            }
        })?;
        for v in v_parameters {
            for &u in &u_parameters {
                let target_frame = surface.frame_at(u, v, tolerance)?;
                transforms.push(AffineTransform3::try_frame_mapping(
                    source_frame,
                    target_frame,
                    [1.0; 3],
                )?);
            }
        }
        let copies = document
            .copy_objects_with_transforms(sources.iter().copied(), transforms.as_slice())?;
        document.select_objects_direct(sources.iter().copied(), SelectionMode::Replace)?;
        debug_assert!(!document.is_selected(surface_id));
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {u_count}×{v_count} surface cells using {} spacing, creating {copy_count} copy object(s)",
            sources.len(),
            options.mode.name(),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceArrayMode {
    Uv,
    Isocurve,
}

impl SurfaceArrayMode {
    const fn name(self) -> &'static str {
        match self {
            Self::Uv => "UV",
            Self::Isocurve => "Isocurve",
        }
    }
}

#[derive(Debug, PartialEq)]
struct SurfaceArrayOptions {
    base_point: Point3,
    up: Vector3,
    mode: SurfaceArrayMode,
    surface_name: Option<String>,
}

fn parse_surface_array_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 1)
        .ok_or_else(|| CommandError::InvalidSurfaceArrayCount(value.to_owned()))
}

fn parse_surface_array_options(arguments: &[&str]) -> Result<SurfaceArrayOptions, CommandError> {
    let mut base_point = None;
    let mut up = Vector3::try_new(0.0, 0.0, 1.0).expect("the world z direction is finite");
    let mut mode = SurfaceArrayMode::Uv;
    let mut surface_name = None;
    let mut up_seen = false;
    let mut mode_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, ARRAY_SURFACE_USAGE)?;
        if option_name_eq(name, "BasePoint") && base_point.is_none() {
            base_point = Some(parse_single_option_point(value, ARRAY_SURFACE_USAGE)?);
        } else if (option_name_eq(name, "Up") || option_name_eq(name, "Normal")) && !up_seen {
            up = Vector3::try_from(
                parse_single_option_point(value, ARRAY_SURFACE_USAGE)?.to_array(),
            )?;
            up_seen = true;
        } else if option_name_eq(name, "Mode") && !mode_seen {
            let value = value.trim_start_matches(['_', '-']);
            mode = if value.eq_ignore_ascii_case("UV") {
                SurfaceArrayMode::Uv
            } else if value.eq_ignore_ascii_case("Isocurve") {
                SurfaceArrayMode::Isocurve
            } else {
                return Err(CommandError::Usage(ARRAY_SURFACE_USAGE));
            };
            mode_seen = true;
        } else if option_name_eq(name, "SurfaceName") && surface_name.is_none() && !value.is_empty()
        {
            surface_name = Some(value.to_owned());
        } else {
            return Err(CommandError::Usage(ARRAY_SURFACE_USAGE));
        }
        index += consumed;
    }
    Ok(SurfaceArrayOptions {
        base_point: base_point.ok_or(CommandError::Usage(ARRAY_SURFACE_USAGE))?,
        up,
        mode,
        surface_name,
    })
}

fn parse_single_option_point(value: &str, usage: &'static str) -> Result<Point3, CommandError> {
    let (point, consumed) = parse_point(&[value])?;
    if consumed == 1 {
        Ok(point)
    } else {
        Err(CommandError::Usage(usage))
    }
}

fn surface_array_inputs(
    document: &Document,
    surface_name: Option<&str>,
) -> Result<(NurbsSurface, ObjectId, Vec<ObjectId>), CommandError> {
    let selected = selected_ids(document)?;
    let surface_id = if let Some(name) = surface_name {
        let matches = document
            .objects()
            .filter(|object| {
                object
                    .attributes()
                    .name()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
            .map(|object| object.id())
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => return Err(CommandError::SurfaceArrayTargetNotFound(name.to_owned())),
            [id] => *id,
            _ => return Err(CommandError::AmbiguousSurfaceArrayTarget(name.to_owned())),
        }
    } else {
        *selected
            .last()
            .ok_or(CommandError::SurfaceArrayTargetRequired)?
    };
    let target = document
        .object(surface_id)
        .expect("resolved surface-array target identifiers are present");
    let Geometry::NurbsSurface(surface) = target.geometry() else {
        return Err(CommandError::SurfaceArrayTargetNotSurface);
    };
    let sources = selected
        .into_iter()
        .filter(|id| *id != surface_id)
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::SurfaceArraySourcesRequired);
    }
    Ok((surface.clone(), surface_id, sources))
}

fn surface_array_parameters(
    surface: &NurbsSurface,
    u_count: usize,
    v_count: usize,
    mode: SurfaceArrayMode,
    tolerance: Tolerance,
) -> Result<(Vec<Real>, Vec<Real>), GeometryError> {
    let u_start = *surface.domain_u().start();
    let v_start = *surface.domain_v().start();
    let parameters = match mode {
        SurfaceArrayMode::Uv => (
            normalized_surface_parameters(u_count, |normalized| {
                surface.parameter_at_u(normalized)
            })?,
            normalized_surface_parameters(v_count, |normalized| {
                surface.parameter_at_v(normalized)
            })?,
        ),
        SurfaceArrayMode::Isocurve => (
            if u_count == 1 {
                vec![u_start]
            } else {
                surface.divide_u_isocurve_by_count(v_start, u_count - 1, true, tolerance)?
            },
            if v_count == 1 {
                vec![v_start]
            } else {
                surface.divide_v_isocurve_by_count(u_start, v_count - 1, true, tolerance)?
            },
        ),
    };
    Ok(parameters)
}

fn normalized_surface_parameters(
    count: usize,
    mut parameter_at: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Vec<Real>, GeometryError> {
    if count == 1 {
        return Ok(vec![parameter_at(0.0)?]);
    }
    (0..count)
        .map(|index| parameter_at(index as Real / (count - 1) as Real))
        .collect()
}

struct ArrayLinearCommand;

impl Command for ArrayLinearCommand {
    fn name(&self) -> &'static str {
        "ArrayLinear"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let item_count_text = arguments.first().ok_or(CommandError::Usage(
            "ArrayLinear item-count first-reference second-reference",
        ))?;
        let item_count = parse_array_item_count(item_count_text)?;
        let selected = selected_ids(document)?;
        let copy_instance_count = item_count - 1;
        let copy_count = selected
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let (first_reference, first_consumed) = parse_point(&arguments[1..])?;
        let (second_reference, second_consumed) = parse_point(&arguments[1 + first_consumed..])?;
        require_consumed(
            arguments,
            1 + first_consumed + second_consumed,
            "ArrayLinear item-count first-reference second-reference",
        )?;
        let spacing = first_reference.vector_to(second_reference)?;
        let transforms = (1..item_count)
            .map(|index| {
                spacing
                    .scaled(index as Real)
                    .map(AffineTransform3::from_translation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let copies = document
            .copy_objects_with_transforms(selected.iter().copied(), transforms.as_slice())?;
        document.select_objects_direct(selected, SelectionMode::Replace)?;
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {item_count} total item(s), creating {copy_count} copy object(s) at spacing {}",
            copy_count / copy_instance_count,
            format_vector(spacing)
        ))
    }
}

const ARRAY_POLAR_USAGE: &str =
    "ArrayPolar item-count center angle-degrees [Rotate=Yes|No] [ZOffset=distance]";

struct ArrayPolarCommand;

impl Command for ArrayPolarCommand {
    fn name(&self) -> &'static str {
        "ArrayPolar"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let item_count_text = arguments
            .first()
            .ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
        let item_count = parse_array_item_count(item_count_text)?;
        let selected = selected_ids(document)?;
        let copy_instance_count = item_count - 1;
        let copy_count = selected
            .len()
            .checked_mul(copy_instance_count)
            .filter(|count| *count <= MAX_ARRAY_OBJECTS)
            .ok_or(CommandError::TooManyArrayObjects {
                maximum: MAX_ARRAY_OBJECTS,
            })?;
        let (center, center_consumed) = parse_point(&arguments[1..])?;
        let angle_index = 1 + center_consumed;
        let angle_text = arguments
            .get(angle_index)
            .ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
        let fill_angle_degrees = parse_finite_real(angle_text)?;
        if fill_angle_degrees == 0.0 {
            return Err(CommandError::InvalidPolarArrayAngle(
                (*angle_text).to_owned(),
            ));
        }
        let options = parse_polar_array_options(&arguments[angle_index + 1..])?;
        let axis = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance())?;
        let divisor = if fill_angle_degrees.abs() == 360.0 {
            item_count
        } else {
            copy_instance_count
        };
        let step_radians = (fill_angle_degrees / divisor as Real).to_radians();
        let anchor = if options.rotate {
            None
        } else {
            Some(selected_geometry_bounds(document, &selected)?.center()?)
        };
        let transforms = (1..item_count)
            .map(|index| {
                let rotation =
                    AffineTransform3::try_rotation(center, axis, step_radians * index as Real)?;
                let z_offset = axis.as_vector().scaled(options.z_offset * index as Real)?;
                if let Some(anchor) = anchor {
                    let destination = rotation.transform_point(anchor)?.translated(z_offset)?;
                    Ok(AffineTransform3::from_translation(
                        anchor.vector_to(destination)?,
                    ))
                } else {
                    post_translate(rotation, z_offset)
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let copies = document
            .copy_objects_with_transforms(selected.iter().copied(), transforms.as_slice())?;
        document.select_objects_direct(selected, SelectionMode::Replace)?;
        debug_assert_eq!(copies.len(), copy_count);
        Ok(format!(
            "Arrayed {} object(s) into {item_count} total item(s) over {fill_angle_degrees:.6} degrees, creating {copy_count} copy object(s)",
            copy_count / copy_instance_count
        ))
    }
}

#[derive(Clone, Copy)]
struct PolarArrayOptions {
    rotate: bool,
    z_offset: Real,
}

fn parse_polar_array_options(arguments: &[&str]) -> Result<PolarArrayOptions, CommandError> {
    let mut options = PolarArrayOptions {
        rotate: true,
        z_offset: 0.0,
    };
    let mut rotate_seen = false;
    let mut z_offset_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        if name.eq_ignore_ascii_case("Rotate") && !rotate_seen {
            options.rotate = parse_yes_no(value).ok_or(CommandError::Usage(ARRAY_POLAR_USAGE))?;
            rotate_seen = true;
        } else if name.eq_ignore_ascii_case("ZOffset") && !z_offset_seen {
            options.z_offset = parse_finite_real(value)?;
            z_offset_seen = true;
        } else {
            return Err(CommandError::Usage(ARRAY_POLAR_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}

fn parse_array_item_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 2)
        .ok_or_else(|| CommandError::InvalidArrayItemCount(value.to_owned()))
}

fn post_translate(
    transform: AffineTransform3,
    offset: Vector3,
) -> Result<AffineTransform3, GeometryError> {
    let translation = transform.translation();
    AffineTransform3::try_new(
        transform.linear_rows(),
        Vector3::try_new(
            translation.x() + offset.x(),
            translation.y() + offset.y(),
            translation.z() + offset.z(),
        )?,
    )
}

const SCALE_USAGE: &str = "Scale center factor | center reference target [Copy=Yes|No]";
const SCALE_1D_USAGE: &str =
    "Scale1D origin factor direction | origin reference target [Copy=Yes|No]";
const SCALE_2D_USAGE: &str = "Scale2D center factor | center reference target [Copy=Yes|No]";
const SCALE_NU_USAGE: &str = "ScaleNU origin x-factor y-factor z-factor [Copy=Yes|No]";

struct ScaleCommand;

impl Command for ScaleCommand {
    fn name(&self) -> &'static str {
        "Scale"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SCALE_USAGE)?;
        let (center, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];
        let factor = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_nonzero_scale(remaining[0])?
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(remaining, reference_consumed + target_consumed, SCALE_USAGE)?;
            scale_factor_from_reference(center, reference, target, document.tolerance())?
        };
        let transform = AffineTransform3::try_uniform_scale(center, factor)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) uniformly by {factor:.6}, creating {copied} copy object(s)"
        ))
    }
}

struct ScaleOneDimensionalCommand;

impl Command for ScaleOneDimensionalCommand {
    fn name(&self) -> &'static str {
        "Scale1D"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SCALE_1D_USAGE)?;
        let (origin, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];

        let typed = remaining.first().and_then(|factor| {
            if factor.contains(',') {
                return None;
            }
            parse_point(&remaining[1..])
                .ok()
                .filter(|(_, point_consumed)| 1 + point_consumed == remaining.len())
                .map(|(direction, _)| (*factor, direction))
        });
        let (direction_point, factor) = if let Some((factor, direction)) = typed {
            (direction, parse_finite_real(factor)?)
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                SCALE_1D_USAGE,
            )?;
            let factor = scale_factor_from_reference_allow_zero(
                origin,
                reference,
                target,
                document.tolerance(),
            )?;
            (reference, factor)
        };
        let direction = origin
            .vector_to(direction_point)?
            .normalized(document.tolerance())?;
        let transform = AffineTransform3::try_directional_scale(origin, direction, factor)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) in one direction by {factor:.6}, creating {copied} copy object(s)"
        ))
    }
}

struct ScaleTwoDimensionalCommand;

impl Command for ScaleTwoDimensionalCommand {
    fn name(&self) -> &'static str {
        "Scale2D"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SCALE_2D_USAGE)?;
        let (center, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];
        let factor = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_nonzero_scale(remaining[0])?
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                SCALE_2D_USAGE,
            )?;
            top_view_scale_factor_from_reference(center, reference, target, document.tolerance())?
        };
        let transform = AffineTransform3::try_nonuniform_scale(center, [factor, factor, 1.0])?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) in two dimensions by {factor:.6}, creating {copied} copy object(s)"
        ))
    }
}

struct ScaleNonUniformCommand;

impl Command for ScaleNonUniformCommand {
    fn name(&self) -> &'static str {
        "ScaleNU"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SCALE_NU_USAGE)?;
        let (origin, consumed) = parse_point(&positional)?;
        let [x_factor, y_factor, z_factor] = positional[consumed..] else {
            return Err(CommandError::Usage(SCALE_NU_USAGE));
        };
        let factors = [
            parse_nonzero_scale(x_factor)?,
            parse_nonzero_scale(y_factor)?,
            parse_nonzero_scale(z_factor)?,
        ];
        let transform = AffineTransform3::try_nonuniform_scale(origin, factors)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Scaled {transformed} object(s) non-uniformly by {:.6},{:.6},{:.6}, creating {copied} copy object(s)",
            factors[0], factors[1], factors[2]
        ))
    }
}

const ROTATE_USAGE: &str = "Rotate center degrees | center reference target [Copy=Yes|No]";
const ROTATE_3D_USAGE: &str =
    "Rotate3D axis-start axis-end degrees | axis-start axis-end reference target [Copy=Yes|No]";
const MIRROR_USAGE: &str = "Mirror axis-start axis-end [Copy=Yes|No]";
const SHEAR_USAGE: &str = "Shear origin reference degrees | origin reference target [Copy=Yes|No]";

struct RotateCommand;

impl Command for RotateCommand {
    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, ROTATE_USAGE)?;
        let (center, consumed) = parse_point(&positional)?;
        let remaining = &positional[consumed..];
        let angle_radians = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?.to_radians()
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                ROTATE_USAGE,
            )?;
            top_view_angle(center, reference, target, document.tolerance())?
        };
        let axis = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance())?;
        let transform = AffineTransform3::try_rotation(center, axis, angle_radians)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Rotated {transformed} object(s) by {:.6} degrees, creating {copied} copy object(s)",
            angle_radians.to_degrees(),
        ))
    }
}

struct RotateThreeDimensionalCommand;

impl Command for RotateThreeDimensionalCommand {
    fn name(&self) -> &'static str {
        "Rotate3D"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, ROTATE_3D_USAGE)?;
        let (axis_start, start_consumed) = parse_point(&positional)?;
        let (axis_end, end_consumed) = parse_point(&positional[start_consumed..])?;
        let consumed = start_consumed + end_consumed;
        let remaining = &positional[consumed..];
        let axis = axis_start
            .vector_to(axis_end)?
            .normalized(document.tolerance())?;
        let angle_radians = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?.to_radians()
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                ROTATE_3D_USAGE,
            )?;
            axis_rotation_angle(axis_start, axis, reference, target, document.tolerance())?
        };
        let transform = AffineTransform3::try_rotation(axis_start, axis, angle_radians)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Rotated {transformed} object(s) around a 3D axis by {:.6} degrees, creating {copied} copy object(s)",
            angle_radians.to_degrees(),
        ))
    }
}

struct MirrorCommand;

impl Command for MirrorCommand {
    fn name(&self) -> &'static str {
        "Mirror"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, MIRROR_USAGE)?;
        let (axis_start, consumed) = parse_point(&positional)?;
        let (axis_end, end_consumed) = parse_point(&positional[consumed..])?;
        require_consumed(&positional, consumed + end_consumed, MIRROR_USAGE)?;
        let normal = top_view_mirror_normal(axis_start, axis_end, document.tolerance())?;
        let transform = AffineTransform3::try_reflection(axis_start, normal)?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Mirrored {transformed} object(s), creating {copied} copy object(s)"
        ))
    }
}

struct ShearCommand;

impl Command for ShearCommand {
    fn name(&self) -> &'static str {
        "Shear"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (positional, copy) = parse_transform_copy_arguments(arguments, SHEAR_USAGE)?;
        let (origin, origin_consumed) = parse_point(&positional)?;
        let (reference, reference_consumed) = parse_point(&positional[origin_consumed..])?;
        let consumed = origin_consumed + reference_consumed;
        let remaining = &positional[consumed..];
        let angle_radians = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?.to_radians()
        } else {
            let (target, target_consumed) = parse_point(remaining)?;
            require_consumed(remaining, target_consumed, SHEAR_USAGE)?;
            top_view_angle(origin, reference, target, document.tolerance())?
        };
        let reference_direction =
            top_view_vector(origin, reference)?.normalized(document.tolerance())?;
        let shear_direction = UnitVector3::try_new(
            -reference_direction.y(),
            reference_direction.x(),
            0.0,
            document.tolerance(),
        )?;
        let factor = angle_radians.tan();
        let transform = AffineTransform3::try_shear(
            origin,
            reference_direction,
            shear_direction,
            factor,
            document.tolerance(),
        )?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Sheared {transformed} object(s) by {:.6} degrees, creating {copied} copy object(s)",
            angle_radians.to_degrees()
        ))
    }
}

const PROJECT_TO_CPLANE_USAGE: &str = "ProjectToCPlane [DeleteInput=Yes|No]";

struct ProjectToConstructionPlaneCommand;

impl Command for ProjectToConstructionPlaneCommand {
    fn name(&self) -> &'static str {
        "ProjectToCPlane"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let delete_input =
            parse_delete_input(arguments, PROJECT_TO_CPLANE_USAGE, &["DeleteInput"])?;
        let origin = Point3::try_new(0.0, 0.0, 0.0)?;
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance())?;
        let transform = AffineTransform3::try_planar_projection(Plane::new(origin, normal))?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, !delete_input)?;
        Ok(format!(
            "Projected {transformed} object(s) to the construction plane, creating {copied} copy object(s)"
        ))
    }
}

const TO_NURBS_USAGE: &str = "ToNURBS [DeleteInputObjects=Yes|No]";

struct ToNurbsCommand;

impl Command for ToNurbsCommand {
    fn name(&self) -> &'static str {
        "ToNURBS"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let delete_input = parse_delete_input(
            arguments,
            TO_NURBS_USAGE,
            &["DeleteInputObjects", "DeleteInput"],
        )?;
        let conversions = selected
            .into_iter()
            .filter_map(|id| {
                document
                    .object(id)
                    .expect("selected objects exist")
                    .geometry()
                    .converted_to_nurbs_curve()
                    .transpose()
                    .map(|result| result.map(|curve| (id, Geometry::NurbsCurve(curve))))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if conversions.is_empty() {
            return Err(CommandError::NoConvertibleNurbsCurves);
        }

        let converted = conversions.len();
        let copied = if delete_input {
            document.replace_object_geometries(conversions)?;
            0
        } else {
            document
                .copy_object_geometries_into_source_groups(conversions)?
                .len()
        };
        Ok(format!(
            "Converted {converted} curve object(s) to exact NURBS geometry, creating {copied} copy object(s)"
        ))
    }
}

const EXTRUDE_CURVE_USAGE: &str = "ExtrudeCrv distance | base target [BothSides=Yes|No] [DeleteInput=Yes|No] [Output=Surface] [Solid=No]";

struct ExtrudeCurveCommand;

impl Command for ExtrudeCurveCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (start_offset, end_offset, delete_input, both_sides) =
            parse_curve_extrusion(arguments, document.tolerance())?;
        let mut extrusions = Vec::new();
        for id in selected {
            let Some(curve) = document
                .object(id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            extrusions.push((
                id,
                NurbsSurface::try_extruded_curve(&curve, start_offset, end_offset)?,
            ));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }

        let output_count = extrusions.len();
        for (_, surface) in &extrusions {
            document.add_geometry(Geometry::NurbsSurface(surface.clone()))?;
        }
        if delete_input {
            for (id, _) in extrusions {
                document.delete_object(id)?;
            }
        }
        Ok(format!(
            "Extruded {output_count} curve object(s) into exact NURBS surfaces{}{}",
            if both_sides { " on both sides" } else { "" },
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

fn parse_curve_extrusion(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<(Vector3, Vector3, bool, bool), CommandError> {
    let mut positional = Vec::new();
    let mut both_sides = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid_seen = false;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "BothSides") && both_sides.is_none() {
            both_sides = parse_yes_no(value);
            if both_sides.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = parse_yes_no(value);
            if delete_input.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveExtrusionOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            let solid = parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_USAGE))?;
            if solid {
                return Err(CommandError::SolidCurveExtrusionUnsupported);
            }
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
        }
    }

    let direction = if positional.len() == 1 && !positional[0].contains(',') {
        Vector3::try_new(0.0, 0.0, parse_finite_real(positional[0])?)?
    } else {
        let (base, base_consumed) = parse_point(&positional)?;
        let (target, target_consumed) = parse_point(&positional[base_consumed..])?;
        require_consumed(
            &positional,
            base_consumed + target_consumed,
            EXTRUDE_CURVE_USAGE,
        )?;
        base.vector_to(target)?
    };
    direction.normalized(tolerance)?;
    let both_sides = both_sides.unwrap_or(false);
    let start_offset = if both_sides {
        direction.scaled(-1.0)?
    } else {
        Vector3::try_new(0.0, 0.0, 0.0)?
    };
    Ok((
        start_offset,
        direction,
        delete_input.unwrap_or(false),
        both_sides,
    ))
}

const EXTRUDE_CURVE_ALONG_CURVE_USAGE: &str = "ExtrudeCrvAlongCrv [PathName=name] [DeleteInput=Yes|No] [Output=Surface] [Solid=No] [SplitAtTangents=Yes|No]";

struct ExtrudeCurveAlongCurveCommand;

impl Command for ExtrudeCurveAlongCurveCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvAlongCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (path_name, delete_input) = parse_curve_along_curve_options(arguments)?;
        let selected = selected_ids(document)?;
        let path_id = resolve_curve_along_curve_path(document, &selected, path_name.as_deref())?;
        let path = document
            .object(path_id)
            .expect("resolved extrusion paths exist")
            .geometry()
            .nurbs_curve_representation()?
            .ok_or(CommandError::CurveAlongCurvePathNotCurve)?;
        let profile_ids = selected
            .iter()
            .copied()
            .filter(|id| *id != path_id)
            .collect::<Vec<_>>();
        if profile_ids.is_empty() {
            return Err(CommandError::CurveAlongCurveProfilesRequired);
        }

        let mut extrusions = Vec::new();
        for id in &profile_ids {
            let Some(profile) = document
                .object(*id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            extrusions.push((
                *id,
                NurbsSurface::try_extruded_curve_along_curve(&profile, &path)?,
            ));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoCurveAlongCurveProfiles);
        }

        let output_count = extrusions.len();
        for (_, surface) in &extrusions {
            document.add_geometry(Geometry::NurbsSurface(surface.clone()))?;
        }
        if delete_input {
            for (id, _) in &extrusions {
                document.delete_object(*id)?;
            }
        }
        let retained_selection = selected
            .into_iter()
            .filter(|id| *id != path_id && document.object(*id).is_some())
            .collect::<Vec<_>>();
        document.select_objects_direct(retained_selection, SelectionMode::Replace)?;
        Ok(format!(
            "Extruded {output_count} curve object(s) along the fixed-orientation path into exact NURBS surfaces{}",
            if delete_input {
                ", deleting the input profiles"
            } else {
                ""
            }
        ))
    }
}

fn parse_curve_along_curve_options(
    arguments: &[&str],
) -> Result<(Option<String>, bool), CommandError> {
    let mut path_name = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid_seen = false;
    let mut split_at_tangents_seen = false;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE));
        };
        if option_name_eq(name, "PathName") && path_name.is_none() && !value.is_empty() {
            path_name = Some(value.to_owned());
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = Some(
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?,
            );
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveAlongCurveOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            let solid =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?;
            if solid {
                return Err(CommandError::SolidCurveExtrusionUnsupported);
            }
            solid_seen = true;
        } else if option_name_eq(name, "SplitAtTangents") && !split_at_tangents_seen {
            parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?;
            split_at_tangents_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE));
        }
    }
    Ok((path_name, delete_input.unwrap_or(false)))
}

fn resolve_curve_along_curve_path(
    document: &Document,
    selected: &[ObjectId],
    path_name: Option<&str>,
) -> Result<ObjectId, CommandError> {
    if let Some(name) = path_name {
        let matches = document
            .objects()
            .filter(|object| {
                object
                    .attributes()
                    .name()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
            .map(|object| object.id())
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(CommandError::CurveAlongCurvePathNotFound(name.to_owned())),
            [id] => Ok(*id),
            _ => Err(CommandError::AmbiguousCurveAlongCurvePath(name.to_owned())),
        }
    } else {
        selected
            .last()
            .copied()
            .ok_or(CommandError::CurveAlongCurvePathRequired)
    }
}

const EXTRUDE_CURVE_TO_POINT_USAGE: &str =
    "ExtrudeCrvToPoint apex [DeleteInput=Yes|No] [Output=Surface] [Solid=No]";

struct ExtrudeCurveToPointCommand;

impl Command for ExtrudeCurveToPointCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvToPoint"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (apex, delete_input) = parse_curve_to_point_extrusion(arguments)?;
        let mut extrusions = Vec::new();
        for id in selected {
            let Some(curve) = document
                .object(id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            extrusions.push((id, NurbsSurface::try_extruded_curve_to_point(&curve, apex)?));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }

        let output_count = extrusions.len();
        for (_, surface) in &extrusions {
            document.add_geometry(Geometry::NurbsSurface(surface.clone()))?;
        }
        if delete_input {
            for (id, _) in extrusions {
                document.delete_object(id)?;
            }
        }
        Ok(format!(
            "Extruded {output_count} curve object(s) to an exact NURBS apex surface{}",
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

fn parse_curve_to_point_extrusion(arguments: &[&str]) -> Result<(Point3, bool), CommandError> {
    let mut positional = Vec::new();
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid_seen = false;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = parse_yes_no(value);
            if delete_input.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE));
            }
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveToPointExtrusionOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && !solid_seen {
            let solid =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE))?;
            if solid {
                return Err(CommandError::SolidCurveExtrusionUnsupported);
            }
            solid_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE));
        }
    }

    let (apex, consumed) = parse_point(&positional)?;
    require_consumed(&positional, consumed, EXTRUDE_CURVE_TO_POINT_USAGE)?;
    Ok((apex, delete_input.unwrap_or(false)))
}

const REVOLVE_USAGE: &str = "Revolve axis-start axis-end angle-degrees [StartAngle=degrees] [FullCircle=Yes|No] [DeleteInput=Yes|No] [Output=Surface] [Deformable=No] [SplitAtTangents=Yes|No]";

struct RevolveCommand;

impl Command for RevolveCommand {
    fn name(&self) -> &'static str {
        "Revolve"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (axis_origin, axis, start_angle, sweep_angle, delete_input) =
            parse_revolution(arguments, document.tolerance())?;
        let mut revolutions = Vec::new();
        for id in selected {
            let Some(curve) = document
                .object(id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            revolutions.push((
                id,
                NurbsSurface::try_revolved_curve(
                    &curve,
                    axis_origin,
                    axis,
                    start_angle,
                    sweep_angle,
                )?,
            ));
        }
        if revolutions.is_empty() {
            return Err(CommandError::NoRevolvableCurves);
        }

        let output_count = revolutions.len();
        for (_, surface) in &revolutions {
            document.add_geometry(Geometry::NurbsSurface(surface.clone()))?;
        }
        if delete_input {
            for (id, _) in revolutions {
                document.delete_object(id)?;
            }
        }
        Ok(format!(
            "Revolved {output_count} curve object(s) through {:.6} degrees into exact NURBS surfaces{}",
            sweep_angle.to_degrees(),
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

fn parse_revolution(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<(Point3, UnitVector3, Real, Real, bool), CommandError> {
    let mut positional = Vec::new();
    let mut start_angle_degrees = None;
    let mut full_circle = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut deformable_seen = false;
    let mut split_at_tangents = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "StartAngle") && start_angle_degrees.is_none() {
            start_angle_degrees = Some(parse_finite_real(value)?);
        } else if option_name_eq(name, "FullCircle") && full_circle.is_none() {
            full_circle = Some(parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?);
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = Some(parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?);
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedRevolveOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Deformable") && !deformable_seen {
            let deformable = parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?;
            if deformable {
                return Err(CommandError::DeformableRevolveUnsupported);
            }
            deformable_seen = true;
        } else if option_name_eq(name, "SplitAtTangents") && split_at_tangents.is_none() {
            split_at_tangents =
                Some(parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE))?);
        } else {
            return Err(CommandError::Usage(REVOLVE_USAGE));
        }
    }

    let (axis_origin, origin_consumed) = parse_point(&positional)?;
    let (axis_end, end_consumed) = parse_point(&positional[origin_consumed..])?;
    let remaining = &positional[origin_consumed + end_consumed..];
    let sweep_degrees = if full_circle.unwrap_or(false) {
        if !remaining.is_empty() {
            return Err(CommandError::Usage(REVOLVE_USAGE));
        }
        360.0
    } else {
        let [angle] = remaining else {
            return Err(CommandError::Usage(REVOLVE_USAGE));
        };
        let degrees = parse_finite_real(angle)?;
        if degrees == 0.0 || degrees.abs() > 360.0 {
            return Err(CommandError::InvalidRevolutionAngle((*angle).to_owned()));
        }
        degrees
    };
    let start_angle = start_angle_degrees.unwrap_or(0.0).to_radians();
    let sweep_angle = if sweep_degrees.abs() == 360.0 {
        sweep_degrees.signum() * std::f64::consts::TAU
    } else {
        sweep_degrees.to_radians()
    };
    if !start_angle.is_finite() || !sweep_angle.is_finite() {
        return Err(CommandError::InvalidRevolutionAngle(
            sweep_degrees.to_string(),
        ));
    }
    let axis = axis_origin.vector_to(axis_end)?.normalized(tolerance)?;
    Ok((
        axis_origin,
        axis,
        start_angle,
        sweep_angle,
        delete_input.unwrap_or(false),
    ))
}

fn parse_delete_input(
    arguments: &[&str],
    usage: &'static str,
    option_names: &[&str],
) -> Result<bool, CommandError> {
    let [argument] = arguments else {
        return if arguments.is_empty() {
            Ok(false)
        } else {
            Err(CommandError::Usage(usage))
        };
    };
    if let Some((name, value)) = argument.split_once('=') {
        if !option_names
            .iter()
            .any(|option_name| option_name_eq(name, option_name))
        {
            return Err(CommandError::Usage(usage));
        }
        parse_yes_no(value).ok_or(CommandError::Usage(usage))
    } else {
        parse_yes_no(argument).ok_or(CommandError::Usage(usage))
    }
}

fn create_current_layer(
    document: &mut Document,
    name_arguments: &[&str],
) -> Result<String, CommandError> {
    let name = joined_argument(name_arguments, "Layer [New] name")?;
    let color = suggested_layer_color(document.layers().len());
    let id = document.add_layer(name.clone(), color)?;
    document.set_current_layer(id)?;
    Ok(format!("Created current layer '{name}'"))
}

fn joined_argument(arguments: &[&str], usage: &'static str) -> Result<String, CommandError> {
    if arguments.is_empty() {
        Err(CommandError::Usage(usage))
    } else {
        Ok(arguments.join(" "))
    }
}

fn named_layer_id(document: &Document, name: &str) -> Result<LayerId, CommandError> {
    document
        .layer_by_name(name)
        .map(|layer| layer.id())
        .ok_or_else(|| CommandError::NamedLayerNotFound(name.to_owned()))
}

fn parse_color(value: &str) -> Result<ColorRgb, CommandError> {
    let components: Vec<_> = value.split(',').collect();
    if components.len() != 3 {
        return Err(CommandError::InvalidColor(value.to_owned()));
    }
    let mut parsed = [0_u8; 3];
    for (target, component) in parsed.iter_mut().zip(components) {
        *target = component
            .parse::<u8>()
            .map_err(|_| CommandError::InvalidColor(value.to_owned()))?;
    }
    Ok(ColorRgb::new(parsed[0], parsed[1], parsed[2]))
}

fn parse_translation(
    arguments: &[&str],
    usage: &'static str,
) -> Result<viboceros_geometry::Vector3, CommandError> {
    let (from, consumed) = parse_point(arguments)?;
    let (to, to_consumed) = parse_point(&arguments[consumed..])?;
    require_consumed(arguments, consumed + to_consumed, usage)?;
    Ok(from.vector_to(to)?)
}

fn format_vector(vector: viboceros_geometry::Vector3) -> String {
    format!("{:.6},{:.6},{:.6}", vector.x(), vector.y(), vector.z())
}

fn selected_ids(document: &Document) -> Result<Vec<viboceros_document::ObjectId>, CommandError> {
    let selected: Vec<_> = document.selected_object_ids().collect();
    if selected.is_empty() {
        Err(CommandError::NoObjectsSelected)
    } else {
        Ok(selected)
    }
}

fn selected_geometry_bounds(
    document: &Document,
    ids: &[ObjectId],
) -> Result<BoundingBox3, CommandError> {
    let mut objects = ids.iter().map(|id| {
        document
            .object(*id)
            .expect("selected object identifiers are present")
    });
    let first = objects
        .next()
        .ok_or(CommandError::NoObjectsSelected)?
        .geometry()
        .bounds();
    Ok(objects.try_fold(first, |bounds, object| {
        bounds.union(object.geometry().bounds())
    })?)
}

fn parse_finite_real(value: &str) -> Result<Real, CommandError> {
    let parsed = value
        .parse::<Real>()
        .map_err(|_| CommandError::InvalidNumber(value.to_owned()))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(CommandError::InvalidNumber(value.to_owned()))
    }
}

fn parse_positive_curve_length(value: &str) -> Result<Real, CommandError> {
    let length = parse_finite_real(value)?;
    if length > 0.0 {
        Ok(length)
    } else {
        Err(CommandError::InvalidMaximumCurveLength(value.to_owned()))
    }
}

fn parse_nonzero_scale(value: &str) -> Result<Real, CommandError> {
    let factor = parse_finite_real(value)?;
    if factor != 0.0 {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(value.to_owned()))
    }
}

fn parse_transform_copy_arguments<'a>(
    arguments: &[&'a str],
    usage: &'static str,
) -> Result<(Vec<&'a str>, bool), CommandError> {
    let mut positional = Vec::with_capacity(arguments.len());
    let mut copy = false;
    let mut copy_seen = false;
    for argument in arguments {
        if let Some((name, value)) = argument.split_once('=') {
            if !option_name_eq(name, "Copy") || copy_seen {
                return Err(CommandError::Usage(usage));
            }
            copy = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
            copy_seen = true;
        } else {
            positional.push(*argument);
        }
    }
    Ok((positional, copy))
}

fn scale_factor_from_reference(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    scale_factor_from_reference_impl(center, reference, target, tolerance, false)
}

fn scale_factor_from_reference_allow_zero(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    scale_factor_from_reference_impl(center, reference, target, tolerance, true)
}

fn scale_factor_from_reference_impl(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
    allow_zero: bool,
) -> Result<Real, CommandError> {
    let reference_distance = center.distance_to(reference)?;
    if reference_distance <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "scale reference",
        }
        .into());
    }
    let target_distance = center.distance_to(target)?;
    let factor = target_distance / reference_distance;
    if factor.is_finite() && (factor > 0.0 || allow_zero && factor == 0.0) {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(format!("{factor}")))
    }
}

fn top_view_scale_factor_from_reference(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    let reference_distance = top_view_vector(center, reference)?.length()?;
    if reference_distance <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "scale reference",
        }
        .into());
    }
    let factor = top_view_vector(center, target)?.length()? / reference_distance;
    if factor.is_finite() && factor > 0.0 {
        Ok(factor)
    } else {
        Err(CommandError::InvalidScaleFactor(format!("{factor}")))
    }
}

fn top_view_angle(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    let from = top_view_vector(center, reference)?.normalized(tolerance)?;
    let to = top_view_vector(center, target)?.normalized(tolerance)?;
    let cosine = from.as_vector().dot(to.as_vector())?.clamp(-1.0, 1.0);
    let sine = from.as_vector().cross(to.as_vector())?.z();
    Ok(sine.atan2(cosine))
}

fn axis_rotation_angle(
    axis_origin: Point3,
    axis: UnitVector3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    let perpendicular_direction = |point: Point3| -> Result<UnitVector3, CommandError> {
        let vector = axis_origin.vector_to(point)?;
        let axial = axis.as_vector().scaled(vector.dot(axis.as_vector())?)?;
        Ok(subtract_vectors(vector, axial)?.normalized(tolerance)?)
    };
    let from = perpendicular_direction(reference)?;
    let to = perpendicular_direction(target)?;
    let cosine = from.as_vector().dot(to.as_vector())?.clamp(-1.0, 1.0);
    let sine = axis
        .as_vector()
        .dot(from.as_vector().cross(to.as_vector())?)?
        .clamp(-1.0, 1.0);
    Ok(sine.atan2(cosine))
}

fn top_view_mirror_normal(
    axis_start: Point3,
    axis_end: Point3,
    tolerance: Tolerance,
) -> Result<UnitVector3, CommandError> {
    let axis = top_view_vector(axis_start, axis_end)?;
    Ok(Vector3::try_new(-axis.y(), axis.x(), 0.0)?.normalized(tolerance)?)
}

fn top_view_vector(origin: Point3, target: Point3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(target.x() - origin.x(), target.y() - origin.y(), 0.0)
}

struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &'static str {
        "Clear"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DeleteAll"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Clear")?;
        let count = document.clear_objects();
        Ok(format!("Deleted {count} object(s)"))
    }
}

struct UndoCommand;

impl Command for UndoCommand {
    fn name(&self) -> &'static str {
        "Undo"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Undo")?;
        Ok(match document.undo()? {
            Some(label) => format!("Undid {label}"),
            None => "Nothing to undo".to_owned(),
        })
    }
}

struct RedoCommand;

impl Command for RedoCommand {
    fn name(&self) -> &'static str {
        "Redo"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Redo")?;
        Ok(match document.redo()? {
            Some(label) => format!("Redid {label}"),
            None => "Nothing to redo".to_owned(),
        })
    }
}

struct ImportStlCommand;

impl Command for ImportStlCommand {
    fn name(&self) -> &'static str {
        "ImportStl"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ImportStl path"));
        }
        let path = arguments.join(" ");
        let mesh = read_stl_file(&path, document.tolerance())?;
        let triangle_count = mesh.triangles().len();
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Imported STL mesh {id} ({triangle_count} triangles) from '{path}'"
        ))
    }
}

struct ExportStlCommand;

impl Command for ExportStlCommand {
    fn name(&self) -> &'static str {
        "ExportStl"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ExportStl [Ascii|Binary] path"));
        }
        let (format, path_arguments) = if arguments[0].eq_ignore_ascii_case("ascii") {
            (StlFormat::Ascii, &arguments[1..])
        } else if arguments[0].eq_ignore_ascii_case("binary") {
            (StlFormat::Binary, &arguments[1..])
        } else {
            (StlFormat::Binary, arguments)
        };
        if path_arguments.is_empty() {
            return Err(CommandError::Usage("ExportStl [Ascii|Binary] path"));
        }
        let path = path_arguments.join(" ");
        let mesh = combined_document_mesh(document)?;
        let triangle_count = mesh.triangles().len();
        write_stl_file(&path, &mesh, format)?;
        Ok(format!(
            "Exported {triangle_count} triangles as {format:?} STL to '{path}'"
        ))
    }
}

struct ImportThreeDmCommand;

struct ImportStepCommand;

impl Command for ImportStepCommand {
    fn name(&self) -> &'static str {
        "ImportStep"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ImportStp"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ImportStep path"));
        }
        let path = arguments.join(" ");
        let import = read_step_file(&path, document.tolerance())?;
        let object_count = import.objects.len();
        let triangle_count = import
            .objects
            .iter()
            .map(|object| object.mesh.triangles().len())
            .sum::<usize>();
        let warning_count = import.report.warning_count();
        let layer_id = document.current_layer_id();
        for object in import.objects {
            let mut attributes = ObjectAttributes::on_layer(layer_id);
            if let Some(name) = object.name {
                attributes = attributes.with_name(name);
            }
            document.add_geometry_with_attributes(Geometry::Mesh(object.mesh), attributes)?;
        }
        Ok(format!(
            "Imported {object_count} STEP mesh object(s) ({triangle_count} triangles) from '{path}' ({warning_count} conversion warning(s))"
        ))
    }
}

struct ExportStepCommand;

impl Command for ExportStepCommand {
    fn name(&self) -> &'static str {
        "ExportStep"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ExportStp"]
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ExportStep path"));
        }
        let path = arguments.join(" ");
        let mesh = combined_document_mesh(document)?;
        let triangle_count = mesh.triangles().len();
        write_step_file(&path, std::slice::from_ref(&mesh))?;
        Ok(format!(
            "Exported {triangle_count} triangles as a STEP faceted shell to '{path}'"
        ))
    }
}

impl Command for ImportThreeDmCommand {
    fn name(&self) -> &'static str {
        "Import3dm"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("Import3dm path"));
        }
        let path = arguments.join(" ");
        let model = read_3dm_file(&path, document.tolerance())?;
        let unsupported = model.unsupported_object_count();
        let layer_count = model.layers.len();
        let object_count = model.objects.len();

        let mut imported_layers = Vec::with_capacity(layer_count);
        for layer in &model.layers {
            let name = unique_import_layer_name(document, &layer.name);
            let id = document.add_layer(
                name,
                ColorRgb::new(layer.color[0], layer.color[1], layer.color[2]),
            )?;
            imported_layers.push(id);
        }

        let mut imported_objects = Vec::with_capacity(object_count);
        for object in model.objects {
            let layer_id = imported_layers[object.layer_index];
            let mut attributes = ObjectAttributes::on_layer(layer_id)
                .with_object_color(ColorRgb::new(
                    object.object_color[0],
                    object.object_color[1],
                    object.object_color[2],
                ))
                .with_color_source(document_color_source_from_3dm(object.color_source))
                .with_visibility(object.visible)
                .with_locked(object.locked);
            if let Some(name) = object.name {
                attributes = attributes.with_name(name);
            }
            let id = document.add_geometry_with_attributes(
                document_geometry_from_3dm(object.geometry, document.tolerance()),
                attributes,
            )?;
            imported_objects.push((id, object.group_indices));
        }

        let mut imported_group_count = 0;
        for (group_index, group) in model.groups.iter().enumerate() {
            let members = imported_objects
                .iter()
                .filter_map(|(id, groups)| groups.contains(&group_index).then_some(*id))
                .collect::<Vec<_>>();
            let name = unique_import_group_name(document, &group.name);
            if members.is_empty() {
                document.add_empty_group(Some(name))?;
            } else {
                document.add_group(Some(name), members)?;
            }
            imported_group_count += 1;
        }

        for (source, id) in model.layers.iter().zip(imported_layers) {
            document.set_layer_visibility(id, source.visible)?;
            document.set_layer_locked(id, source.locked)?;
        }

        Ok(format!(
            "Imported {object_count} objects in {imported_group_count} groups on {layer_count} layers from '{path}' ({unsupported} unsupported objects skipped)"
        ))
    }
}

struct ExportThreeDmCommand;

impl Command for ExportThreeDmCommand {
    fn name(&self) -> &'static str {
        "Export3dm"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("Export3dm path"));
        }
        let path = arguments.join(" ");
        let model = document_3dm_model(document)?;
        let object_count = model.objects.len();
        let group_count = model.groups.len();
        let layer_count = model.layers.len();
        write_3dm_file(&path, &model)?;
        Ok(format!(
            "Exported {object_count} objects in {group_count} groups on {layer_count} layers to '{path}'"
        ))
    }
}

fn document_3dm_model(document: &Document) -> Result<ThreeDmModel, GeometryError> {
    let layers = document
        .layers()
        .map(|layer| {
            let color = layer.color();
            ThreeDmLayer {
                name: layer.name().to_owned(),
                color: [color.red, color.green, color.blue],
                visible: layer.is_visible(),
                locked: layer.is_locked(),
            }
        })
        .collect();
    let layer_indices: BTreeMap<_, _> = document
        .layers()
        .enumerate()
        .map(|(index, layer)| (layer.id(), index))
        .collect();
    let mut used_group_names = document
        .groups()
        .filter_map(|group| group.name().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let document_groups = document.groups().collect::<Vec<_>>();
    let groups = document_groups
        .iter()
        .map(|group| ThreeDmGroup {
            name: group.name().map_or_else(
                || next_serialized_group_name(&mut used_group_names),
                str::to_owned,
            ),
        })
        .collect::<Vec<_>>();
    let mut group_indices_by_object = BTreeMap::<ObjectId, Vec<usize>>::new();
    for (group_index, group) in document_groups.iter().enumerate() {
        for member in group.members() {
            group_indices_by_object
                .entry(member)
                .or_default()
                .push(group_index);
        }
    }
    let objects = document
        .objects()
        .map(|object| {
            Ok(ThreeDmObject {
                geometry: geometry_to_3dm(object.geometry())?,
                layer_index: layer_indices[&object.attributes().layer_id()],
                name: object.attributes().name().map(str::to_owned),
                visible: object.attributes().is_visible(),
                locked: object.attributes().is_locked(),
                object_color: {
                    let color = object.attributes().object_color();
                    [color.red, color.green, color.blue]
                },
                color_source: three_dm_color_source_from_document(
                    object.attributes().color_source(),
                ),
                group_indices: group_indices_by_object
                    .get(&object.id())
                    .cloned()
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<_, GeometryError>>()?;
    Ok(ThreeDmModel::new(layers, groups, objects))
}

fn next_serialized_group_name(used: &mut BTreeSet<String>) -> String {
    for number in 1_u64..=u64::MAX {
        let candidate = format!("Group{number:02}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("the finite document cannot contain every numbered group name")
}

const fn document_color_source_from_3dm(source: ThreeDmColorSource) -> ObjectColorSource {
    match source {
        ThreeDmColorSource::Layer => ObjectColorSource::Layer,
        ThreeDmColorSource::Object => ObjectColorSource::Object,
        ThreeDmColorSource::Material => ObjectColorSource::Material,
        ThreeDmColorSource::Parent => ObjectColorSource::Parent,
    }
}

const fn three_dm_color_source_from_document(source: ObjectColorSource) -> ThreeDmColorSource {
    match source {
        ObjectColorSource::Layer => ThreeDmColorSource::Layer,
        ObjectColorSource::Object => ThreeDmColorSource::Object,
        ObjectColorSource::Material => ThreeDmColorSource::Material,
        ObjectColorSource::Parent => ThreeDmColorSource::Parent,
    }
}

fn geometry_to_3dm(geometry: &Geometry) -> Result<ThreeDmGeometry, GeometryError> {
    Ok(match geometry {
        Geometry::Point(point) => ThreeDmGeometry::Point(*point),
        Geometry::PointCloud(cloud) => ThreeDmGeometry::PointCloud(cloud.clone()),
        Geometry::Line(line) => ThreeDmGeometry::Line(*line),
        Geometry::Circle(circle) => ThreeDmGeometry::NurbsCurve(circle.to_nurbs()?),
        Geometry::Arc(arc) => ThreeDmGeometry::NurbsCurve(arc.to_nurbs()?),
        Geometry::Ellipse(ellipse) => ThreeDmGeometry::NurbsCurve(ellipse.to_nurbs()?),
        Geometry::Polyline(polyline) => ThreeDmGeometry::NurbsCurve(polyline.to_nurbs()?),
        Geometry::NurbsCurve(curve) => ThreeDmGeometry::NurbsCurve(curve.clone()),
        Geometry::NurbsSurface(surface) => ThreeDmGeometry::NurbsSurface(surface.clone()),
        Geometry::Mesh(mesh) => ThreeDmGeometry::Mesh(mesh.clone()),
    })
}

fn document_geometry_from_3dm(geometry: ThreeDmGeometry, tolerance: Tolerance) -> Geometry {
    match geometry {
        ThreeDmGeometry::Point(point) => Geometry::Point(point),
        ThreeDmGeometry::PointCloud(cloud) => Geometry::PointCloud(cloud),
        ThreeDmGeometry::Line(line) => Geometry::Line(line),
        ThreeDmGeometry::NurbsCurve(curve) => exported_polyline(&curve, tolerance)
            .map_or_else(|| Geometry::NurbsCurve(curve), Geometry::Polyline),
        ThreeDmGeometry::NurbsSurface(surface) => Geometry::NurbsSurface(surface),
        ThreeDmGeometry::Mesh(mesh) => Geometry::Mesh(mesh),
    }
}

fn exported_polyline(curve: &NurbsCurve, tolerance: Tolerance) -> Option<Polyline3> {
    if curve.degree() != 1
        || curve
            .control_points()
            .iter()
            .any(|control| control.weight() != 1.0)
    {
        return None;
    }
    let polyline = Polyline3::try_new(
        curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect(),
        tolerance,
    )
    .ok()?;
    let expected = polyline.to_nurbs().ok()?;
    (expected.knots() == curve.knots()).then_some(polyline)
}

fn unique_import_layer_name(document: &Document, source_name: &str) -> String {
    let base = if source_name.trim().is_empty() {
        "Imported Layer"
    } else {
        source_name.trim()
    };
    if document.layer_by_name(base).is_none() {
        return base.to_owned();
    }
    for suffix in 1_u32.. {
        let candidate = format!("{base} (Imported {suffix})");
        if document.layer_by_name(&candidate).is_none() {
            return candidate;
        }
    }
    unreachable!("the finite document cannot contain every numbered layer name")
}

fn unique_import_group_name(document: &Document, source_name: &str) -> String {
    let base = if source_name.trim().is_empty() {
        "Imported Group"
    } else {
        source_name.trim()
    };
    if document.group_by_name(base).is_none() {
        return base.to_owned();
    }
    for suffix in 1_u32.. {
        let candidate = format!("{base} (Imported {suffix})");
        if document.group_by_name(&candidate).is_none() {
            return candidate;
        }
    }
    unreachable!("the finite document cannot contain every numbered group name")
}

fn combined_document_mesh(document: &Document) -> Result<TriangleMesh, CommandError> {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for object in document.objects() {
        if !object.attributes().is_visible()
            || !document
                .layer(object.attributes().layer_id())
                .is_some_and(|layer| layer.is_visible())
        {
            continue;
        }
        let tessellation;
        let mesh = match object.geometry() {
            Geometry::Mesh(mesh) => mesh,
            Geometry::NurbsSurface(surface) => {
                tessellation =
                    surface.tessellate(SURFACE_EXPORT_SAMPLES_PER_SPAN, document.tolerance())?;
                &tessellation
            }
            _ => continue,
        };
        let offset =
            u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.extend_from_slice(mesh.vertices());
        for triangle in mesh.triangles() {
            triangles.push([
                triangle[0]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
                triangle[1]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
                triangle[2]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
            ]);
        }
    }
    if triangles.is_empty() {
        return Err(CommandError::NoMeshToExport);
    }
    Ok(TriangleMesh::try_new(
        vertices,
        triangles,
        document.tolerance(),
    )?)
}

fn parse_point(arguments: &[&str]) -> Result<(Point3, usize), CommandError> {
    let first = arguments
        .first()
        .ok_or(CommandError::Usage("expected a point"))?;
    let (coordinates, consumed) = if first.contains(',') {
        let coordinates: Vec<_> = first.split(',').collect();
        if !(2..=3).contains(&coordinates.len()) {
            return Err(CommandError::Usage("point syntax is x,y or x,y,z"));
        }
        (coordinates, 1)
    } else {
        if arguments.len() < 3 {
            return Err(CommandError::Usage("point syntax is x y z or x,y,z"));
        }
        (arguments[..3].to_vec(), 3)
    };

    let mut parsed = [0.0; 3];
    for (index, coordinate) in coordinates.iter().enumerate() {
        parsed[index] = coordinate
            .parse::<Real>()
            .map_err(|_| CommandError::InvalidNumber((*coordinate).to_owned()))?;
    }
    Ok((Point3::try_from(parsed)?, consumed))
}

fn require_consumed(
    arguments: &[&str],
    consumed: usize,
    usage: &'static str,
) -> Result<(), CommandError> {
    if arguments.len() == consumed {
        Ok(())
    } else {
        Err(CommandError::Usage(usage))
    }
}

fn format_point(point: Point3) -> String {
    format!("{:.6},{:.6},{:.6}", point.x(), point.y(), point.z())
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("enter a command")]
    EmptyInput,

    #[error("unknown command '{0}'; enter Help to list commands")]
    UnknownCommand(String),

    #[error("command name or alias '{0}' is already registered")]
    DuplicateCommand(String),

    #[error("usage: {0}")]
    Usage(&'static str),

    #[error("'{0}' is not a valid finite number")]
    InvalidNumber(String),

    #[error("'{0}' is not a valid non-negative integer")]
    InvalidInteger(String),

    #[error("'{0}' is not a valid r,g,b color with components from 0 through 255")]
    InvalidColor(String),

    #[error("'{0}' is not a valid finite, non-zero scale factor")]
    InvalidScaleFactor(String),

    #[error("'{0}' is not a valid array item count of 2 or more")]
    InvalidArrayItemCount(String),

    #[error("'{0}' is not a valid curve-array item count of 1 or more")]
    InvalidCurveArrayItemCount(String),

    #[error("'{0}' is not a valid finite, strictly positive curve-array distance")]
    InvalidCurveArrayDistance(String),

    #[error("'{0}' is not a valid surface-array dimension count of 1 or more")]
    InvalidSurfaceArrayCount(String),

    #[error("no object named '{0}' was found for the surface-array target")]
    SurfaceArrayTargetNotFound(String),

    #[error("more than one object named '{0}' could be the surface-array target")]
    AmbiguousSurfaceArrayTarget(String),

    #[error("ArraySrf requires a named target surface or a selected surface as the last object")]
    SurfaceArrayTargetRequired,

    #[error("the ArraySrf target must be an untrimmed NURBS surface")]
    SurfaceArrayTargetNotSurface,

    #[error("ArraySrf requires at least one selected source object besides the target surface")]
    SurfaceArraySourcesRequired,

    #[error("no object named '{0}' was found for the surface-orient target")]
    SurfaceOrientTargetNotFound(String),

    #[error("more than one object named '{0}' could be the surface-orient target")]
    AmbiguousSurfaceOrientTarget(String),

    #[error("OrientOnSrf requires a named target surface or a selected surface as the last object")]
    SurfaceOrientTargetRequired,

    #[error("the OrientOnSrf target must be an untrimmed NURBS surface")]
    SurfaceOrientTargetNotSurface,

    #[error("OrientOnSrf requires at least one selected source object besides the target surface")]
    SurfaceOrientSourcesRequired,

    #[error("no object named '{0}' was found for the curve-array path")]
    CurveArrayPathNotFound(String),

    #[error("more than one object named '{0}' could be the curve-array path")]
    AmbiguousCurveArrayPath(String),

    #[error("ArrayCrv requires a named path or a selected path as the last selected object")]
    CurveArrayPathRequired,

    #[error("the curve-array path must be a line, analytic curve, polyline, or NURBS curve")]
    CurveArrayPathNotCurve,

    #[error("ArrayCrv requires at least one selected source object besides the path")]
    CurveArraySourcesRequired,

    #[error("'{0}' is not a valid rectangular-array dimension count of 1 or more")]
    InvalidArrayDimensionCount(String),

    #[error("rectangular-array {axis} fill length is smaller than the selected extent {minimum}")]
    ArrayFillLengthTooSmall { axis: &'static str, minimum: Real },

    #[error("'{0}' is not a valid non-zero polar-array angle")]
    InvalidPolarArrayAngle(String),

    #[error("'{0}' is not a valid finite, strictly positive maximum curve length")]
    InvalidMaximumCurveLength(String),

    #[error("no layer named '{0}' was found")]
    NamedLayerNotFound(String),

    #[error("no group named '{0}' was found")]
    NamedGroupNotFound(String),

    #[error("no objects are selected")]
    NoObjectsSelected,

    #[error("Join requires at least two selected lines or polylines")]
    NotEnoughCurvesToJoin,

    #[error("Join currently supports selected lines and polylines only")]
    UnsupportedJoinGeometry,

    #[error("the selected curves do not have endpoints within the document tolerance")]
    NoJoinableCurves,

    #[error("none of the selected objects is an explodable polyline or point cloud")]
    NoExplodablePolylines,

    #[error("Length supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedLengthGeometry,

    #[error("Area supports selected circles, ellipses, closed planar polylines, and meshes only")]
    UnsupportedAreaGeometry,

    #[error("Volume currently supports selected meshes only")]
    UnsupportedVolumeGeometry,

    #[error("Volume requires every selected mesh to be closed")]
    OpenMeshVolume,

    #[error("Divide supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedDivideGeometry,

    #[error("the requested division creates no point objects")]
    NoCurveDivisionPoints,

    #[error(
        "Flip supports selected lines, analytic curves, polylines, NURBS curves, and meshes only"
    )]
    UnsupportedFlipGeometry,

    #[error("UnifyMeshNormals supports selected meshes only")]
    UnsupportedUnifyMeshNormalsGeometry,

    #[error("CombineIdenticalMeshVertices supports selected meshes only")]
    UnsupportedCombineIdenticalMeshVerticesGeometry,

    #[error("none of the selected meshes contains identical vertex locations")]
    NoIdenticalMeshVertices,

    #[error("CullUnusedMeshVertices supports selected meshes only")]
    UnsupportedCullUnusedMeshVerticesGeometry,

    #[error("none of the selected meshes contains unused vertices")]
    NoUnusedMeshVertices,

    #[error("SplitDisjointMesh supports selected meshes only")]
    UnsupportedSplitDisjointMeshGeometry,

    #[error("none of the selected meshes contains multiple edge-connected pieces")]
    NoDisjointMeshes,

    #[error("ExtractNonManifoldMeshEdges supports selected meshes only")]
    UnsupportedExtractNonManifoldGeometry,

    #[error("none of the selected meshes has faces on a qualifying non-manifold edge")]
    NoNonManifoldMeshFaces,

    #[error("ExtractDuplicateMeshFaces supports selected meshes only")]
    UnsupportedExtractDuplicateMeshFacesGeometry,

    #[error("none of the selected meshes contains duplicate faces")]
    NoDuplicateMeshFaces,

    #[error(
        "CrvStart and CrvEnd support selected lines, analytic curves, polylines, and NURBS curves only"
    )]
    UnsupportedCurveEndpointGeometry,

    #[error("none of the selected objects has extractable defining points")]
    NoExtractablePoints,

    #[error("ExtractPt would extract more than {maximum} points")]
    TooManyExtractedPoints { maximum: usize },

    #[error("the array would create more than {maximum} object copies")]
    TooManyArrayObjects { maximum: usize },

    #[error(
        "CloseCrv currently supports line and polyline inputs only; open arcs and NURBS curves require polycurve support"
    )]
    UnsupportedCloseCurveGeometry,

    #[error("none of the selected objects is a supported non-NURBS curve")]
    NoConvertibleNurbsCurves,

    #[error("none of the selected objects is an extrudable curve")]
    NoExtrudableCurves,

    #[error("ExtrudeCrv currently supports Output=Surface only")]
    UnsupportedCurveExtrusionOutput,

    #[error("ExtrudeCrvToPoint currently supports Output=Surface only")]
    UnsupportedCurveToPointExtrusionOutput,

    #[error("solid curve extrusion requires capped polysurface support")]
    SolidCurveExtrusionUnsupported,

    #[error("ExtrudeCrvAlongCrv currently supports Output=Surface only")]
    UnsupportedCurveAlongCurveOutput,

    #[error(
        "ExtrudeCrvAlongCrv requires a named path or a selected path as the last selected object"
    )]
    CurveAlongCurvePathRequired,

    #[error("no object named '{0}' was found for the extrusion path")]
    CurveAlongCurvePathNotFound(String),

    #[error("more than one object named '{0}' could be the extrusion path")]
    AmbiguousCurveAlongCurvePath(String),

    #[error("the curve-along-curve extrusion path is not a supported curve")]
    CurveAlongCurvePathNotCurve,

    #[error("ExtrudeCrvAlongCrv requires at least one selected profile besides the path")]
    CurveAlongCurveProfilesRequired,

    #[error("none of the selected profile objects is an extrudable curve")]
    NoCurveAlongCurveProfiles,

    #[error("none of the selected objects is a revolvable curve")]
    NoRevolvableCurves,

    #[error("Revolve currently supports Output=Surface only")]
    UnsupportedRevolveOutput,

    #[error("deformable Revolve output is not yet supported")]
    DeformableRevolveUnsupported,

    #[error("'{0}' is not a valid non-zero revolution angle from -360 through 360 degrees")]
    InvalidRevolutionAngle(String),

    #[error("the document contains no visible mesh or NURBS surface to export")]
    NoMeshToExport,

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error(transparent)]
    Stl(#[from] StlError),

    #[error(transparent)]
    Step(#[from] StepError),

    #[error(transparent)]
    ThreeDm(#[from] ThreeDmError),

    #[error(transparent)]
    Document(#[from] DocumentError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::SelectionMode;
    use viboceros_geometry::WeightedPoint3;

    #[test]
    fn dispatches_case_insensitively_and_accepts_rhino_prefix() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "_pT 1,2,3").unwrap();
        assert_eq!(document.objects().len(), 1);
    }

    #[test]
    fn creates_lines_from_comma_or_space_points() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0 3,4,0").unwrap();
        registry.execute(&mut document, "Line 0 0 0 0 0 2").unwrap();
        assert_eq!(document.objects().len(), 2);
    }

    #[test]
    fn failed_command_does_not_mutate_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert!(registry.execute(&mut document, "Line 0,0,0 0,0,0").is_err());
        assert_eq!(document.objects().len(), 0);
    }

    #[test]
    fn reports_sorted_help() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert_eq!(
            registry.execute(&mut document, "Help").unwrap(),
            "Commands: Arc, Area, Array, ArrayCrv, ArrayLinear, ArrayPolar, ArraySrf, ChangeLayer, Circle, Clear, CloseCrv, CombineIdenticalMeshVertices, ControlPointCurve, Copy, CopyToLayer, CrvEnd, CrvStart, CullUnusedMeshVertices, Curve, Delete, Divide, Ellipse, Explode, Export3dm, ExportStep, ExportStl, ExtractDuplicateMeshFaces, ExtractNonManifoldMeshEdges, ExtractPt, ExtrudeCrv, ExtrudeCrvAlongCrv, ExtrudeCrvToPoint, Flip, Group, Hide, HideSwap, Import3dm, ImportStep, ImportStl, InterpCrv, Invert, Isolate, IsolateLock, Join, Layer, Length, Line, Lock, LockSwap, Mirror, Move, Orient, Orient3Pt, OrientOnSrf, Point, Polygon, Polyline, ProjectToCPlane, Rectangle, Redo, Revolve, Rotate, Rotate3D, Scale, Scale1D, Scale2D, ScaleNU, SelAll, SelClosedCrv, SelClosedMesh, SelColor, SelCrv, SelDup, SelDupAll, SelGroup, SelLast, SelLayer, SelLine, SelMesh, SelName, SelNone, SelOpenCrv, SelOpenMesh, SelPlanarCrv, SelPolyline, SelPrev, SelPt, SelPtCloud, SelShortCrv, SelSrf, SetObjectColor, SetObjectName, Shear, Show, SplitDisjointMesh, SrfPt, ToNURBS, Undo, Ungroup, UnifyMeshNormals, Unisolate, UnisolateLock, Unlock, Volume"
        );
    }

    #[test]
    fn creates_exact_circles_and_oriented_three_point_arcs_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Circle 1,2,3 5").unwrap();
        registry
            .execute(&mut document, "Arc 1,0,0 0,-1,0 -1,0,0")
            .unwrap();

        let mut objects = document.objects();
        let Geometry::Circle(circle) = objects.next().unwrap().geometry() else {
            panic!("expected a circle")
        };
        assert_eq!(circle.center(), Point3::try_new(1.0, 2.0, 3.0).unwrap());
        assert_eq!(circle.radius(), 5.0);
        let Geometry::Arc(arc) = objects.next().unwrap().geometry() else {
            panic!("expected an arc")
        };
        assert!(
            document
                .tolerance()
                .approx_eq(arc.sweep_radians(), std::f64::consts::PI)
        );
        assert!(arc.point_at(0.5).unwrap().is_near(
            Point3::try_new(0.0, -1.0, 0.0).unwrap(),
            document.tolerance()
        ));
        drop(objects);
        assert_eq!(document.undo_label(), Some("Arc"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 1);

        assert!(registry.execute(&mut document, "Arc 0,0 1,0 2,0").is_err());
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.undo_label(), Some("Circle"));
    }

    #[test]
    fn creates_validated_polylines_and_top_view_rectangles() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Polyline 0,0 3,0 3,4")
            .unwrap();
        registry
            .execute(&mut document, "Rectangle -1,-2,7 4,5,9")
            .unwrap();

        let mut objects = document.objects();
        let Geometry::Polyline(open) = objects.next().unwrap().geometry() else {
            panic!("expected a polyline")
        };
        assert!(!open.is_closed());
        assert_eq!(open.segment_count(), 2);
        assert_eq!(open.length().unwrap(), 7.0);
        let Geometry::Polyline(rectangle) = objects.next().unwrap().geometry() else {
            panic!("expected a rectangle polyline")
        };
        assert!(rectangle.is_closed());
        assert_eq!(rectangle.segment_count(), 4);
        assert_eq!(
            rectangle.vertices(),
            &[
                Point3::try_new(-1.0, -2.0, 7.0).unwrap(),
                Point3::try_new(4.0, -2.0, 7.0).unwrap(),
                Point3::try_new(4.0, 5.0, 7.0).unwrap(),
                Point3::try_new(-1.0, 5.0, 7.0).unwrap(),
                Point3::try_new(-1.0, -2.0, 7.0).unwrap(),
            ]
        );
        drop(objects);
        assert_eq!(document.undo_label(), Some("Rectangle"));

        assert!(
            registry
                .execute(&mut document, "Rectangle 0,0 0,5")
                .is_err()
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), Some("Rectangle"));

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "Move 0,0,0 10,-2,1")
            .unwrap();
        let Geometry::Polyline(moved) = document.objects().next().unwrap().geometry() else {
            panic!("expected a moved polyline")
        };
        assert_eq!(
            moved.vertices()[0],
            Point3::try_new(10.0, -2.0, 1.0).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();
        let Geometry::Polyline(restored) = document.objects().next().unwrap().geometry() else {
            panic!("expected a restored polyline")
        };
        assert_eq!(
            restored.vertices()[0],
            Point3::try_new(0.0, 0.0, 0.0).unwrap()
        );
    }

    #[test]
    fn creates_exact_ellipses_and_regular_polygons_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Ellipse 1,2,3 5,2,3 3,-4,3")
            .unwrap();
        registry
            .execute(&mut document, "Polygon 6 10,20,7 4")
            .unwrap();

        let mut objects = document.objects();
        let Geometry::Ellipse(ellipse) = objects.next().unwrap().geometry() else {
            panic!("expected an ellipse")
        };
        assert_eq!(ellipse.center(), Point3::try_new(1.0, 2.0, 3.0).unwrap());
        assert_eq!(ellipse.radius_x(), 4.0);
        assert_eq!(ellipse.radius_y(), 40.0_f64.sqrt());
        let Geometry::Polyline(polygon) = objects.next().unwrap().geometry() else {
            panic!("expected a polygon polyline")
        };
        assert!(polygon.is_closed());
        assert_eq!(polygon.segment_count(), 6);
        assert_eq!(
            polygon.vertices()[0],
            Point3::try_new(14.0, 20.0, 7.0).unwrap()
        );
        drop(objects);
        assert_eq!(document.undo_label(), Some("Polygon"));

        assert!(
            registry
                .execute(&mut document, "Ellipse 0,0 1,0 2,0")
                .is_err()
        );
        assert!(registry.execute(&mut document, "Polygon 2 0,0 5").is_err());
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), Some("Polygon"));
    }

    #[test]
    fn length_and_area_measure_mixed_selected_geometry_without_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Circle 0,0 2").unwrap();
        registry
            .execute(&mut document, "Rectangle 0,0 3,4")
            .unwrap();
        registry
            .execute(&mut document, "Ellipse 0,0 3,0 0,2")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let history = document.undo_label().map(str::to_owned);

        let length_message = registry.execute(&mut document, "Len").unwrap();
        let length = length_message
            .split_whitespace()
            .next_back()
            .unwrap()
            .parse::<Real>()
            .unwrap();
        let expected_length = 4.0 * std::f64::consts::PI + 14.0 + 15.865_439_589_290_588;
        assert!(
            Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-12)
                .unwrap()
                .approx_eq(length, expected_length)
        );

        let area_message = registry.execute(&mut document, "Area").unwrap();
        let area = area_message
            .split_whitespace()
            .next_back()
            .unwrap()
            .parse::<Real>()
            .unwrap();
        assert!(
            Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-12)
                .unwrap()
                .approx_eq(area, 10.0 * std::f64::consts::PI + 12.0)
        );
        assert_eq!(document.undo_label(), history.as_deref());

        registry.execute(&mut document, "Point 9,9").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        assert!(matches!(
            registry.execute(&mut document, "Length"),
            Err(CommandError::UnsupportedLengthGeometry)
        ));
        assert!(matches!(
            registry.execute(&mut document, "Area"),
            Err(CommandError::UnsupportedAreaGeometry)
        ));
        assert_eq!(document.undo_label(), Some("Point"));
    }

    #[test]
    fn volume_measures_closed_meshes_with_signed_accumulation_without_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let vertices = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        ];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let outward =
            TriangleMesh::try_new(vertices.clone(), faces.clone(), document.tolerance()).unwrap();
        let reversed = outward.reversed();
        let open =
            TriangleMesh::try_new(vertices, faces[..3].to_vec(), document.tolerance()).unwrap();
        let outward_id = document.add_geometry(Geometry::Mesh(outward)).unwrap();
        let reversed_id = document.add_geometry(Geometry::Mesh(reversed)).unwrap();
        let open_id = document.add_geometry(Geometry::Mesh(open)).unwrap();
        let history = document.undo_label().map(str::to_owned);

        document
            .select_object(outward_id, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "Volume").unwrap(),
            "Measured 1 closed mesh(es): total volume 4.000000000000"
        );
        document
            .select_object(reversed_id, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "Volume").unwrap(),
            "Measured 1 closed mesh(es): total volume -4.000000000000"
        );
        document
            .select_object(outward_id, SelectionMode::Add)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "Volume").unwrap(),
            "Measured 2 closed mesh(es): total volume 0.000000000000"
        );
        assert_eq!(document.undo_label(), history.as_deref());

        document
            .select_object(open_id, SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "Volume"),
            Err(CommandError::OpenMeshVolume)
        ));
        registry.execute(&mut document, "Point 9,9").unwrap();
        registry.execute(&mut document, "SelNone").unwrap();
        registry.execute(&mut document, "SelPt").unwrap();
        assert!(matches!(
            registry.execute(&mut document, "Volume"),
            Err(CommandError::UnsupportedVolumeGeometry)
        ));
    }

    #[test]
    fn divides_selected_curves_by_count_or_length_and_preserves_attributes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Construction")
            .unwrap();
        let source_layer = document.current_layer_id();
        registry.execute(&mut document, "Line 0,0 10,0").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();

        assert_eq!(
            registry.execute(&mut document, "Div 5").unwrap(),
            "Divided 1 curve(s), adding 4 point(s)"
        );
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.selected_object_count(), 4);
        let mut points = document
            .selected_objects()
            .map(|object| {
                assert_eq!(object.attributes().layer_id(), source_layer);
                let Geometry::Point(point) = object.geometry() else {
                    panic!("expected a division point")
                };
                *point
            })
            .collect::<Vec<_>>();
        points.sort_by(|left, right| left.x().total_cmp(&right.x()));
        assert_eq!(
            points,
            [2.0, 4.0, 6.0, 8.0].map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
        );
        assert_eq!(document.undo_label(), Some("Divide"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 1);

        registry.execute(&mut document, "SelAll").unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "Divide Length 3 MarkEnds")
                .unwrap(),
            "Divided 1 curve(s), adding 5 point(s)"
        );
        let mut points = document
            .selected_objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected a division point"),
            })
            .collect::<Vec<_>>();
        points.sort_by(|left, right| left.x().total_cmp(&right.x()));
        assert_eq!(
            points,
            [0.0, 3.0, 6.0, 9.0, 10.0].map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
        );
    }

    #[test]
    fn divides_closed_curves_once_at_the_seam_and_rolls_back_errors() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Circle 0,0 2").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Divide 4").unwrap();
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.selected_object_count(), 4);
        let points = document
            .selected_objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected a division point"),
            })
            .collect::<Vec<_>>();
        let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
            panic!("expected source circle")
        };
        for expected in circle.quadrants().unwrap() {
            assert!(
                points
                    .iter()
                    .any(|actual| actual.is_near(expected, document.tolerance()))
            );
        }

        registry.execute(&mut document, "Point 9,9").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let object_count = document.objects().len();
        assert!(matches!(
            registry.execute(&mut document, "Divide 3"),
            Err(CommandError::UnsupportedDivideGeometry)
        ));
        assert_eq!(document.objects().len(), object_count);
        assert!(registry.execute(&mut document, "Divide 0").is_err());
        assert!(registry.execute(&mut document, "Divide Length -1").is_err());
        assert_eq!(document.objects().len(), object_count);
    }

    #[test]
    fn marks_curve_starts_and_ends_with_attribute_preserving_points() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Markers")
            .unwrap();
        let source_layer = document.current_layer_id();
        registry.execute(&mut document, "Line 1,2,3 7,5,3").unwrap();
        registry.execute(&mut document, "Circle 10,2,3 4").unwrap();
        let source_ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let expected_starts = source_ids
            .iter()
            .map(|id| {
                geometry_curve_ref(document.object(*id).unwrap().geometry())
                    .unwrap()
                    .start_point()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let expected_ends = source_ids
            .iter()
            .map(|id| {
                geometry_curve_ref(document.object(*id).unwrap().geometry())
                    .unwrap()
                    .end_point()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();

        assert_eq!(
            registry.execute(&mut document, "CrvStart").unwrap(),
            "Added 2 point(s) at curve starts"
        );
        let start_points = document
            .selected_objects()
            .map(|object| {
                assert_eq!(object.attributes().layer_id(), source_layer);
                let Geometry::Point(point) = object.geometry() else {
                    panic!("expected a start marker")
                };
                *point
            })
            .collect::<Vec<_>>();
        assert_eq!(start_points.len(), 2);
        assert!(
            expected_starts
                .iter()
                .all(|expected| start_points.contains(expected))
        );
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.undo_label(), Some("CrvStart"));

        registry.execute(&mut document, "Undo").unwrap();
        for id in &source_ids {
            document.select_object(*id, SelectionMode::Add).unwrap();
        }
        assert_eq!(
            registry.execute(&mut document, "CrvEnd").unwrap(),
            "Added 2 point(s) at curve ends"
        );
        let end_points = document
            .selected_objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected an end marker"),
            })
            .collect::<Vec<_>>();
        assert!(
            expected_ends
                .iter()
                .all(|expected| end_points.contains(expected))
        );
        assert_eq!(expected_starts[1], expected_ends[1]);

        registry.execute(&mut document, "Point 99,99").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let object_count = document.objects().len();
        assert!(matches!(
            registry.execute(&mut document, "CrvStart"),
            Err(CommandError::UnsupportedCurveEndpointGeometry)
        ));
        assert_eq!(document.objects().len(), object_count);
    }

    #[test]
    fn extracts_defining_points_in_rhino_order_with_layers_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Layer New Input").unwrap();
        let input_layer = document.current_layer_id();
        registry.execute(&mut document, "Line 0,0,0 2,3,4").unwrap();
        registry.execute(&mut document, "Circle 10,0,0 2").unwrap();
        registry
            .execute(&mut document, "Polyline 0,0 2,0 2,2 0,0")
            .unwrap();
        let periodic = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 2.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        document
            .add_geometry(Geometry::NurbsCurve(periodic))
            .unwrap();
        let surface = NurbsSurface::try_bilinear([
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
            Point3::try_new(2.0, 0.0, 2.0).unwrap(),
            Point3::try_new(2.0, 3.0, 4.0).unwrap(),
            Point3::try_new(0.0, 3.0, 3.0).unwrap(),
        ])
        .unwrap();
        document
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
        let mesh = TriangleMesh::try_new(
            vec![
                Point3::try_new(99.0, 99.0, 99.0).unwrap(),
                Point3::try_new(0.0, 0.0, 5.0).unwrap(),
                Point3::try_new(2.0, 0.0, 5.0).unwrap(),
                Point3::try_new(0.0, 2.0, 5.0).unwrap(),
            ],
            vec![[1, 2, 3]],
            document.tolerance(),
        )
        .unwrap();
        document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        registry.execute(&mut document, "Point 8,8,8").unwrap();

        let source_ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let expected_points = document
            .objects()
            .map(|object| object.geometry().extract_point_locations().unwrap())
            .collect::<Vec<_>>()
            .concat();
        assert_eq!(expected_points.len(), 24);
        registry.execute(&mut document, "Layer New Output").unwrap();
        let output_layer = document.current_layer_id();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group Sources").unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "ExtractPt OutputLayer=Input Output Points",)
                .unwrap(),
            "Extracted 24 point(s) from 6 of 7 selected object(s)"
        );
        assert_eq!(document.undo_label(), Some("ExtractPt"));
        assert_eq!(document.selected_object_count(), 24);
        let extracted = document
            .objects()
            .skip(source_ids.len())
            .map(|object| {
                assert_eq!(object.attributes().layer_id(), input_layer);
                let Geometry::Point(point) = object.geometry() else {
                    panic!("expected an extracted point")
                };
                *point
            })
            .collect::<Vec<_>>();
        assert_eq!(extracted, expected_points);
        assert_eq!(
            document
                .group_by_name("Sources")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            source_ids.iter().copied().collect()
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), source_ids.len());
        document
            .select_object(source_ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "ExtractPt OutputLayer Current")
                .unwrap(),
            "Extracted 24 point(s) from 6 of 7 selected object(s)"
        );
        for object in document.objects().skip(source_ids.len()) {
            assert_eq!(object.attributes().layer_id(), output_layer);
        }

        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_object(source_ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractPt Output=PointCloud OutputLayer=Input",
                )
                .unwrap(),
            "Extracted 24 point(s) from 6 of 7 selected object(s)"
        );
        assert_eq!(document.selected_object_count(), 1);
        let cloud_object = document.selected_objects().next().unwrap();
        assert_eq!(cloud_object.attributes().layer_id(), input_layer);
        let Geometry::PointCloud(cloud) = cloud_object.geometry() else {
            panic!("expected an extracted point cloud")
        };
        assert_eq!(cloud.points(), expected_points);

        let mut point_only = Document::default();
        registry.execute(&mut point_only, "Point 1,2,3").unwrap();
        registry.execute(&mut point_only, "SelAll").unwrap();
        let history = point_only.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut point_only, "ExtractPt"),
            Err(CommandError::NoExtractablePoints)
        ));
        assert_eq!(point_only.undo_label(), history.as_deref());
        assert_eq!(point_only.objects().len(), 1);
        assert!(matches!(
            registry.execute(&mut point_only, "ExtractPt Output=PointCloud"),
            Err(CommandError::NoExtractablePoints)
        ));
    }

    #[test]
    fn point_cloud_extraction_selection_and_explode_match_rhino() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Layer New First").unwrap();
        let first_layer = document.current_layer_id();
        registry.execute(&mut document, "Line 0,0 2,0").unwrap();
        registry.execute(&mut document, "SelLast").unwrap();
        registry
            .execute(&mut document, "SetObjectName Guide")
            .unwrap();
        registry.execute(&mut document, "Layer New Second").unwrap();
        let second_layer = document.current_layer_id();
        registry
            .execute(&mut document, "Polyline 10,0 12,0 10,2")
            .unwrap();
        let source_points = document
            .objects()
            .flat_map(|object| object.geometry().extract_point_locations().unwrap())
            .collect::<Vec<_>>();
        let source_ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let reversed_source_points = [source_ids[1], source_ids[0]]
            .into_iter()
            .flat_map(|id| {
                document
                    .object(id)
                    .unwrap()
                    .geometry()
                    .extract_point_locations()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document
            .select_object(source_ids[1], SelectionMode::Replace)
            .unwrap();
        document
            .select_object(source_ids[0], SelectionMode::Add)
            .unwrap();
        registry
            .execute(&mut document, "ExtractPt Output=PointCloud")
            .unwrap();
        let reversed_cloud = document.selected_objects().next().unwrap();
        assert_eq!(reversed_cloud.attributes().layer_id(), second_layer);
        assert_eq!(reversed_cloud.attributes().name(), None);
        let Geometry::PointCloud(reversed_cloud) = reversed_cloud.geometry() else {
            panic!("expected a reverse-selection point cloud")
        };
        assert_eq!(reversed_cloud.points(), reversed_source_points);
        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "SelNone").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "ExtractPt Output=PointCloud")
                .unwrap(),
            "Extracted 5 point(s) from 2 of 2 selected object(s)"
        );
        let input_cloud_id = document.selected_object_ids().next().unwrap();
        let input_cloud = document.object(input_cloud_id).unwrap();
        assert_eq!(input_cloud.attributes().layer_id(), first_layer);
        assert_eq!(input_cloud.attributes().name(), Some("Guide"));
        let Geometry::PointCloud(cloud) = input_cloud.geometry() else {
            panic!("expected a point cloud")
        };
        assert_eq!(cloud.points(), source_points);
        assert_eq!(document.selected_object_count(), 1);

        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "ExtractPt OutputLayer=Current Output=PointCloud",
            )
            .unwrap();
        let current_cloud_id = document.selected_object_ids().next().unwrap();
        let current_cloud = document.object(current_cloud_id).unwrap();
        assert_eq!(current_cloud.attributes().layer_id(), second_layer);
        assert_eq!(current_cloud.attributes().name(), None);

        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelPt").unwrap(),
            "Selected 0 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelPtCloud").unwrap(),
            "Selected 1 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "Explode").unwrap(),
            "Exploded 1 point cloud(s) into 5 point(s); 0 object(s) unchanged"
        );
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(document.objects().len(), 7);
        assert!(document.objects().skip(2).all(|object| {
            matches!(object.geometry(), Geometry::Point(_))
                && object.attributes().layer_id() == second_layer
                && object.attributes().name().is_none()
        }));
        registry.execute(&mut document, "Undo").unwrap();
        assert!(matches!(
            document.object(current_cloud_id).unwrap().geometry(),
            Geometry::PointCloud(_)
        ));
    }

    #[test]
    fn closes_polylines_without_changing_identity_attributes_or_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Boundary")
            .unwrap();
        let layer_id = document.current_layer_id();
        registry
            .execute(&mut document, "Polyline 0,0 4,0 2,3")
            .unwrap();
        let id = document.objects().next().unwrap().id();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group Loop").unwrap();

        assert_eq!(
            registry.execute(&mut document, "CloseCrv").unwrap(),
            "Closed 1 of 1 selected curve(s): 1 with a line, 0 by moving an endpoint; 0 unchanged"
        );
        let object = document.object(id).unwrap();
        assert_eq!(object.attributes().layer_id(), layer_id);
        let Geometry::Polyline(closed) = object.geometry() else {
            panic!("expected a closed polyline")
        };
        assert_eq!(
            closed.vertices(),
            &[
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(4.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 3.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            ]
        );
        assert!(document.is_selected(id));
        assert!(
            document
                .group_by_name("Loop")
                .unwrap()
                .members()
                .any(|member| member == id)
        );
        assert_eq!(document.undo_label(), Some("CloseCrv"));

        registry.execute(&mut document, "Undo").unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Polyline(polyline) if !polyline.is_closed()
        ));
        registry.execute(&mut document, "Redo").unwrap();
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Polyline(polyline) if polyline.is_closed()
        ));

        registry.execute(&mut document, "Line 10,0 14,0").unwrap();
        let line_id = document.objects().last().unwrap().id();
        document.clear_selection();
        document.select_object(line_id, SelectionMode::Add).unwrap();
        let history = document.undo_label().map(str::to_owned);
        assert_eq!(
            registry.execute(&mut document, "CloseCrv").unwrap(),
            "Closed 0 of 1 selected curve(s): 0 with a line, 0 by moving an endpoint; 1 unchanged"
        );
        assert!(matches!(
            document.object(line_id).unwrap().geometry(),
            Geometry::Line(_)
        ));
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn closes_near_polyline_endpoints_and_respects_wide_gap_policy_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Polyline 0,0 2,0 2,2 0,0.0000000005")
            .unwrap();
        let id = document.objects().next().unwrap().id();
        registry.execute(&mut document, "SelAll").unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "CloseCrv CloseWideGapsWithLine=No Tolerance=0.000000001",
                )
                .unwrap(),
            "Closed 1 of 1 selected curve(s): 0 with a line, 1 by moving an endpoint; 0 unchanged"
        );
        let Geometry::Polyline(closed) = document.object(id).unwrap().geometry() else {
            panic!("expected a closed polyline")
        };
        assert!(closed.is_closed());
        assert_eq!(closed.segment_count(), 3);

        registry.execute(&mut document, "Undo").unwrap();
        let history = document.undo_label().map(str::to_owned);
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "CloseCrv CloseWideGapsWithLine No Tolerance 0.0000000001",
                )
                .unwrap(),
            "Closed 0 of 1 selected curve(s): 0 with a line, 0 by moving an endpoint; 1 unchanged"
        );
        assert_eq!(document.undo_label(), history.as_deref());
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Polyline(polyline) if !polyline.is_closed()
        ));

        registry.execute(&mut document, "Arc 1,0 0,1 -1,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut document, "CloseCrv"),
            Err(CommandError::UnsupportedCloseCurveGeometry)
        ));
        assert_eq!(
            document.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
        assert!(
            registry
                .execute(&mut document, "CloseCrv Tolerance=-1")
                .is_err()
        );
    }

    #[test]
    fn reverses_curve_direction_without_changing_identity_or_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 1,2 7,5").unwrap();
        registry.execute(&mut document, "Circle 10,0 3").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let Geometry::Line(original_line) = document.object(ids[0]).unwrap().geometry() else {
            panic!("expected source line")
        };
        let original_line = *original_line;
        let Geometry::Circle(original_circle) = document.object(ids[1]).unwrap().geometry() else {
            panic!("expected source circle")
        };
        let original_circle = *original_circle;
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group Pair").unwrap();

        assert_eq!(
            registry.execute(&mut document, "Rev").unwrap(),
            "Flipped 2 object(s)"
        );
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(
            document
                .group_by_name("Pair")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            ids.iter().copied().collect()
        );
        assert!(matches!(
            document.object(ids[0]).unwrap().geometry(),
            Geometry::Line(line) if line.start() == original_line.end() && line.end() == original_line.start()
        ));
        let Geometry::Circle(reversed_circle) = document.object(ids[1]).unwrap().geometry() else {
            panic!("expected reversed circle")
        };
        assert_eq!(
            reversed_circle.point_at_angle(0.0).unwrap(),
            original_circle.point_at_angle(0.0).unwrap()
        );
        assert!(
            document.tolerance().approx_eq(
                reversed_circle
                    .normal()
                    .unwrap()
                    .as_vector()
                    .dot(original_circle.normal().unwrap().as_vector())
                    .unwrap(),
                -1.0
            )
        );
        assert_eq!(document.undo_label(), Some("Flip"));

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(ids[0]).unwrap().geometry(),
            &Geometry::Line(original_line)
        );
        assert_eq!(
            document.object(ids[1]).unwrap().geometry(),
            &Geometry::Circle(original_circle)
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert!(matches!(
            document.object(ids[0]).unwrap().geometry(),
            Geometry::Line(line) if *line == original_line.reversed()
        ));

        registry.execute(&mut document, "Point 99,99").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut document, "Reverse"),
            Err(CommandError::UnsupportedFlipGeometry)
        ));
        assert_eq!(
            document.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn unifies_and_flips_mesh_normals_without_changing_identity_or_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Printable")
            .unwrap();
        let vertices = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        ];
        let oriented_faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let mut inconsistent_faces = oriented_faces.clone();
        inconsistent_faces[1].swap(1, 2);
        let inconsistent = TriangleMesh::try_new(
            vertices.clone(),
            inconsistent_faces.clone(),
            document.tolerance(),
        )
        .unwrap();
        let oriented =
            TriangleMesh::try_new(vertices, oriented_faces.clone(), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::Mesh(inconsistent)).unwrap();
        let second = document
            .add_geometry(Geometry::Mesh(oriented.clone()))
            .unwrap();
        let attributes = document.object(first).unwrap().attributes().clone();
        registry.execute(&mut document, "SelMesh").unwrap();
        registry.execute(&mut document, "Group Shells").unwrap();

        assert_eq!(
            registry.execute(&mut document, "UnifyMeshNormals").unwrap(),
            "Unified 2 mesh(es): flipped 1 face(s) in 1 mesh(es); 1 already consistent"
        );
        assert_eq!(document.undo_label(), Some("UnifyMeshNormals"));
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.object(first).unwrap().attributes(), &attributes);
        assert_eq!(
            document
                .group_by_name("Shells")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            [first, second].into_iter().collect()
        );
        for id in [first, second] {
            let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
                panic!("expected mesh")
            };
            assert!(mesh.topology().is_oriented());
            assert_eq!(mesh.triangles(), oriented_faces);
        }

        registry.execute(&mut document, "Undo").unwrap();
        let Geometry::Mesh(restored) = document.object(first).unwrap().geometry() else {
            panic!("expected restored mesh")
        };
        assert_eq!(restored.triangles(), inconsistent_faces);
        assert!(!restored.topology().is_oriented());
        registry.execute(&mut document, "Redo").unwrap();

        assert_eq!(
            registry.execute(&mut document, "Flip").unwrap(),
            "Flipped 2 object(s)"
        );
        for id in [first, second] {
            let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
                panic!("expected flipped mesh")
            };
            assert_eq!(mesh, &oriented.reversed());
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.undo_label(), Some("UnifyMeshNormals"));

        registry.execute(&mut document, "Point 9,9").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut document, "UnifyMeshNormals"),
            Err(CommandError::UnsupportedUnifyMeshNormalsGeometry)
        ));
        assert_eq!(
            document.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn combines_identical_mesh_vertices_with_identity_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Cleanup")
            .unwrap();
        let duplicated = TriangleMesh::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, -2.0, 0.0).unwrap(),
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(99.0, 99.0, 99.0).unwrap(),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            document.tolerance(),
        )
        .unwrap();
        let unique = TriangleMesh::try_new(
            vec![
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let source = document
            .add_geometry(Geometry::Mesh(duplicated.clone()))
            .unwrap();
        let unique_id = document
            .add_geometry(Geometry::Mesh(unique.clone()))
            .unwrap();
        let attributes = document.object(source).unwrap().attributes().clone();
        registry.execute(&mut document, "SelMesh").unwrap();
        registry.execute(&mut document, "Group CleanupSet").unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "CombineIdenticalMeshVertices")
                .unwrap(),
            "Combined 3 identical vertex occurrence(s) in 1 mesh(es); 1 mesh(es) unchanged"
        );
        assert_eq!(document.undo_label(), Some("CombineIdenticalMeshVertices"));
        assert_eq!(document.object(source).unwrap().attributes(), &attributes);
        let Geometry::Mesh(combined) = document.object(source).unwrap().geometry() else {
            panic!("expected combined mesh")
        };
        assert_eq!(
            combined.vertices(),
            &[
                Point3::try_new(99.0, 99.0, 99.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, -2.0, 0.0).unwrap(),
            ]
        );
        assert_eq!(combined.triangles(), &[[3, 1, 2], [1, 3, 4]]);
        assert_eq!(
            document.object(unique_id).unwrap().geometry(),
            &Geometry::Mesh(unique.clone())
        );
        assert_eq!(
            document
                .group_by_name("CleanupSet")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, unique_id])
        );
        assert_eq!(document.selected_object_count(), 2);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(duplicated)
        );

        let mut unique_only = Document::default();
        let unique_only_id = unique_only.add_geometry(Geometry::Mesh(unique)).unwrap();
        unique_only
            .select_object(unique_only_id, SelectionMode::Replace)
            .unwrap();
        let history = unique_only.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut unique_only, "CombineIdenticalMeshVertices"),
            Err(CommandError::NoIdenticalMeshVertices)
        ));
        assert_eq!(unique_only.undo_label(), history.as_deref());

        registry.execute(&mut unique_only, "Point 9,9").unwrap();
        registry.execute(&mut unique_only, "SelAll").unwrap();
        let before = unique_only.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut unique_only, "CombineIdenticalMeshVertices"),
            Err(CommandError::UnsupportedCombineIdenticalMeshVerticesGeometry)
        ));
        assert_eq!(
            unique_only.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn culls_unused_mesh_vertices_with_identity_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Cleanup")
            .unwrap();
        let with_unused = TriangleMesh::try_new(
            vec![
                Point3::try_new(99.0, 99.0, 99.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(98.0, 98.0, 98.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(97.0, 97.0, 97.0).unwrap(),
            ],
            vec![[1, 3, 4], [3, 5, 4]],
            document.tolerance(),
        )
        .unwrap();
        let compact = TriangleMesh::try_new(
            vec![
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let source = document
            .add_geometry(Geometry::Mesh(with_unused.clone()))
            .unwrap();
        let compact_id = document
            .add_geometry(Geometry::Mesh(compact.clone()))
            .unwrap();
        let attributes = document.object(source).unwrap().attributes().clone();
        registry.execute(&mut document, "SelMesh").unwrap();
        registry.execute(&mut document, "Group CleanupSet").unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "CullUnusedMeshVertices")
                .unwrap(),
            "Culled 3 unused vertex occurrence(s) in 1 mesh(es); 1 mesh(es) unchanged"
        );
        assert_eq!(document.undo_label(), Some("CullUnusedMeshVertices"));
        assert_eq!(document.object(source).unwrap().attributes(), &attributes);
        let Geometry::Mesh(culled) = document.object(source).unwrap().geometry() else {
            panic!("expected culled mesh")
        };
        assert_eq!(
            culled.vertices(),
            &[
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            ]
        );
        assert_eq!(culled.triangles(), &[[0, 1, 2], [1, 3, 2]]);
        assert_eq!(
            document.object(compact_id).unwrap().geometry(),
            &Geometry::Mesh(compact.clone())
        );
        assert_eq!(
            document
                .group_by_name("CleanupSet")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, compact_id])
        );
        assert_eq!(document.selected_object_count(), 2);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(with_unused.clone())
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.undo_label(), Some("CullUnusedMeshVertices"));

        let mut compact_only = Document::default();
        let compact_only_id = compact_only
            .add_geometry(Geometry::Mesh(compact.clone()))
            .unwrap();
        compact_only
            .select_object(compact_only_id, SelectionMode::Replace)
            .unwrap();
        let history = compact_only.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut compact_only, "CullUnusedMeshVertices"),
            Err(CommandError::NoUnusedMeshVertices)
        ));
        assert_eq!(compact_only.undo_label(), history.as_deref());

        let mut mixed = Document::default();
        mixed.add_geometry(Geometry::Mesh(with_unused)).unwrap();
        registry.execute(&mut mixed, "Point 9,9").unwrap();
        registry.execute(&mut mixed, "SelAll").unwrap();
        let before = mixed.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut mixed, "CullUnusedMeshVertices"),
            Err(CommandError::UnsupportedCullUnusedMeshVerticesGeometry)
        ));
        assert_eq!(
            mixed.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn splits_disjoint_meshes_with_identity_attributes_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Components")
            .unwrap();
        let disjoint = TriangleMesh::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            document.tolerance(),
        )
        .unwrap();
        let connected = TriangleMesh::try_new(
            vec![
                Point3::try_new(20.0, 0.0, 0.0).unwrap(),
                Point3::try_new(21.0, 0.0, 0.0).unwrap(),
                Point3::try_new(21.0, 1.0, 0.0).unwrap(),
                Point3::try_new(20.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            document.tolerance(),
        )
        .unwrap();
        let first = document
            .add_geometry(Geometry::Mesh(disjoint.clone()))
            .unwrap();
        let second = document
            .add_geometry(Geometry::Mesh(connected.clone()))
            .unwrap();
        let attributes = document.object(first).unwrap().attributes().clone();
        registry.execute(&mut document, "SelMesh").unwrap();
        registry.execute(&mut document, "Group Assembly").unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "SplitDisjointMesh")
                .unwrap(),
            "Split 1 mesh(es) into 2 piece(s); 1 mesh(es) unchanged"
        );
        assert_eq!(document.undo_label(), Some("SplitDisjointMesh"));
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.selected_object_count(), 3);
        let third = document
            .objects()
            .map(|object| object.id())
            .find(|id| *id != first && *id != second)
            .unwrap();
        assert_eq!(document.object(third).unwrap().attributes(), &attributes);
        assert_eq!(
            document
                .group_by_name("Assembly")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second, third])
        );
        for id in [first, third] {
            let Geometry::Mesh(piece) = document.object(id).unwrap().geometry() else {
                panic!("expected split mesh piece")
            };
            assert_eq!(piece.triangles().len(), 1);
            assert_eq!(piece.vertices().len(), 3);
        }
        assert_eq!(
            document.object(second).unwrap().geometry(),
            &Geometry::Mesh(connected)
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(
            document.object(first).unwrap().geometry(),
            &Geometry::Mesh(disjoint.clone())
        );
        assert_eq!(
            document
                .group_by_name("Assembly")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 3);
        assert_eq!(
            document
                .group_by_name("Assembly")
                .unwrap()
                .members()
                .count(),
            3
        );

        let mut connected_only = Document::default();
        let connected_id = connected_only
            .add_geometry(Geometry::Mesh(disjoint.disjoint_pieces()[0].clone()))
            .unwrap();
        connected_only
            .select_object(connected_id, SelectionMode::Replace)
            .unwrap();
        let history = connected_only.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut connected_only, "SplitDisjointMesh"),
            Err(CommandError::NoDisjointMeshes)
        ));
        assert_eq!(connected_only.undo_label(), history.as_deref());

        registry.execute(&mut connected_only, "Point 9,9").unwrap();
        registry.execute(&mut connected_only, "SelAll").unwrap();
        let before = connected_only.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut connected_only, "SplitDisjointMesh"),
            Err(CommandError::UnsupportedSplitDisjointMeshGeometry)
        ));
        assert_eq!(
            connected_only.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn extracts_non_manifold_faces_with_options_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Layer New Repair").unwrap();
        let vertices = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
            Point3::try_new(0.0, -1.0, 1.0).unwrap(),
        ];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]];
        let non_manifold = TriangleMesh::try_new(vertices, faces, document.tolerance()).unwrap();
        let clean = TriangleMesh::try_new(
            vec![
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let source = document
            .add_geometry(Geometry::Mesh(non_manifold.clone()))
            .unwrap();
        let clean_id = document
            .add_geometry(Geometry::Mesh(clean.clone()))
            .unwrap();
        let attributes = document.object(source).unwrap().attributes().clone();
        registry.execute(&mut document, "SelMesh").unwrap();
        registry.execute(&mut document, "Group RepairSet").unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "ExtractNonManifoldMeshEdges")
                .unwrap(),
            "Extracted 3 face(s) from 1 mesh(es); 1 mesh(es) unchanged"
        );
        assert_eq!(document.undo_label(), Some("ExtractNonManifoldMeshEdges"));
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.selected_object_count(), 3);
        let extracted_id = document
            .objects()
            .map(|object| object.id())
            .find(|id| *id != source && *id != clean_id)
            .unwrap();
        assert_eq!(
            document.object(extracted_id).unwrap().attributes(),
            &attributes
        );
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("expected remainder mesh")
        };
        assert_eq!(remainder.triangles(), &[[0, 3, 2], [1, 2, 3]]);
        let Geometry::Mesh(extracted) = document.object(extracted_id).unwrap().geometry() else {
            panic!("expected extracted mesh")
        };
        assert_eq!(extracted.triangles().len(), 3);
        assert_eq!(
            document
                .group_by_name("RepairSet")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, clean_id, extracted_id])
        );
        assert_eq!(
            document.object(clean_id).unwrap().geometry(),
            &Geometry::Mesh(clean)
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(non_manifold.clone())
        );
        assert_eq!(
            document
                .group_by_name("RepairSet")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, clean_id])
        );

        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(
                &mut document,
                "ExtractNonManifoldMeshEdges MinimumFaceCount=4"
            ),
            Err(CommandError::NoNonManifoldMeshFaces)
        ));
        assert_eq!(document.undo_label(), history.as_deref());
        assert!(matches!(
            registry.execute(
                &mut document,
                "ExtractNonManifoldMeshEdges MinimumFaceCount=2"
            ),
            Err(CommandError::Geometry(
                GeometryError::InvalidNonManifoldMinimumFaceCount(2)
            ))
        ));

        let mut selective = Document::default();
        let selective_source = selective
            .add_geometry(Geometry::Mesh(non_manifold))
            .unwrap();
        selective
            .select_object(selective_source, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut selective,
                    "ExtractNonManifoldMeshEdges ExtractHangingFacesOnly Yes MinimumFaceCount 3"
                )
                .unwrap(),
            "Extracted 1 face(s) from 1 mesh(es); 0 mesh(es) unchanged"
        );
        assert_eq!(selective.objects().len(), 2);
        let Geometry::Mesh(selective_remainder) =
            selective.object(selective_source).unwrap().geometry()
        else {
            panic!("expected selective remainder mesh")
        };
        assert_eq!(selective_remainder.triangles().len(), 4);
        let selective_extracted = selective
            .objects()
            .find(|object| object.id() != selective_source)
            .unwrap();
        assert!(matches!(
            selective_extracted.geometry(),
            Geometry::Mesh(mesh) if mesh.triangles().len() == 1
        ));

        registry.execute(&mut selective, "Point 9,9").unwrap();
        registry.execute(&mut selective, "SelAll").unwrap();
        assert!(matches!(
            registry.execute(&mut selective, "ExtractNonManifoldMeshEdges"),
            Err(CommandError::UnsupportedExtractNonManifoldGeometry)
        ));
    }

    #[test]
    fn extracts_duplicate_mesh_faces_with_identity_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Layer New Repair").unwrap();
        let vertices = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        ];
        let faces = vec![[0, 1, 2], [0, 1, 6], [3, 5, 4]];
        let duplicate = TriangleMesh::try_new(vertices, faces, document.tolerance()).unwrap();
        let clean = TriangleMesh::try_new(
            vec![
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            document.tolerance(),
        )
        .unwrap();
        let source = document
            .add_geometry(Geometry::Mesh(duplicate.clone()))
            .unwrap();
        let clean_id = document
            .add_geometry(Geometry::Mesh(clean.clone()))
            .unwrap();
        let attributes = document.object(source).unwrap().attributes().clone();
        registry.execute(&mut document, "SelMesh").unwrap();
        registry
            .execute(&mut document, "Group DuplicateSet")
            .unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "ExtractDuplicateMeshFaces")
                .unwrap(),
            "Extracted 1 duplicate face(s) from 1 mesh(es); 1 mesh(es) unchanged"
        );
        assert_eq!(document.undo_label(), Some("ExtractDuplicateMeshFaces"));
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.selected_object_count(), 3);
        let extracted_id = document
            .objects()
            .map(|object| object.id())
            .find(|id| *id != source && *id != clean_id)
            .unwrap();
        assert_eq!(
            document.object(extracted_id).unwrap().attributes(),
            &attributes
        );
        let Geometry::Mesh(remainder) = document.object(source).unwrap().geometry() else {
            panic!("expected duplicate-free remainder")
        };
        assert_eq!(remainder.triangles(), &[[0, 1, 2], [0, 1, 3]]);
        let Geometry::Mesh(extracted) = document.object(extracted_id).unwrap().geometry() else {
            panic!("expected extracted duplicate mesh")
        };
        assert_eq!(extracted.triangles(), &[[0, 2, 1]]);
        assert_eq!(
            document
                .group_by_name("DuplicateSet")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, clean_id, extracted_id])
        );
        assert_eq!(
            document.object(clean_id).unwrap().geometry(),
            &Geometry::Mesh(clean.clone())
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(duplicate)
        );
        assert_eq!(
            document
                .group_by_name("DuplicateSet")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, clean_id])
        );

        let mut clean_only = Document::default();
        let clean_only_id = clean_only.add_geometry(Geometry::Mesh(clean)).unwrap();
        clean_only
            .select_object(clean_only_id, SelectionMode::Replace)
            .unwrap();
        let history = clean_only.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut clean_only, "ExtractDuplicateMeshFaces"),
            Err(CommandError::NoDuplicateMeshFaces)
        ));
        assert_eq!(clean_only.undo_label(), history.as_deref());

        registry.execute(&mut clean_only, "Point 9,9").unwrap();
        registry.execute(&mut clean_only, "SelAll").unwrap();
        let before = clean_only.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut clean_only, "ExtractDuplicateMeshFaces"),
            Err(CommandError::UnsupportedExtractDuplicateMeshFacesGeometry)
        ));
        assert_eq!(
            clean_only.objects().collect::<Vec<_>>(),
            before.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn joins_selected_linear_chains_and_preserves_disconnected_inputs() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for command in [
            "Line 1,0 2,0",
            "Line 4,0 3,0",
            "Line 3,0 2,0",
            "Line 10,0 11,0",
        ] {
            registry.execute(&mut document, command).unwrap();
        }
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        for id in &ids {
            document.select_object(*id, SelectionMode::Add).unwrap();
        }

        assert_eq!(
            registry.execute(&mut document, "Join").unwrap(),
            "Joined 3 curve(s) into 1 polyline(s); 1 curve(s) unchanged"
        );
        assert_eq!(document.objects().len(), 2);
        assert!(document.object(ids[3]).is_some());
        let Geometry::Polyline(joined) = document
            .objects()
            .find(|object| object.id() != ids[3])
            .unwrap()
            .geometry()
        else {
            panic!("expected a joined polyline")
        };
        assert_eq!(
            joined.vertices(),
            &[
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(3.0, 0.0, 0.0).unwrap(),
                Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            ]
        );
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.undo_label(), Some("Join"));

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 4);
        assert!(ids.iter().all(|id| document.object(*id).is_some()));
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 2);
    }

    #[test]
    fn join_rejects_branches_and_unsupported_geometry_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for command in ["Line -1,0 0,0", "Line 0,0 1,0", "Line 0,0 0,1"] {
            registry.execute(&mut document, command).unwrap();
        }
        registry.execute(&mut document, "SelAll").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "Join"),
            Err(CommandError::Geometry(
                GeometryError::AmbiguousPolylineJoin { endpoint_count: 3 }
            ))
        ));
        assert_eq!(
            document
                .objects()
                .map(|object| object.id())
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(document.undo_label(), history.as_deref());

        registry.execute(&mut document, "Point 5,5").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        assert!(matches!(
            registry.execute(&mut document, "Join"),
            Err(CommandError::UnsupportedJoinGeometry)
        ));
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.undo_label(), Some("Point"));
    }

    #[test]
    fn explodes_selected_polylines_into_attribute_preserving_lines() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Construction")
            .unwrap();
        let construction = document.current_layer_id();
        registry
            .execute(&mut document, "Rectangle 0,0 4,3")
            .unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry.execute(&mut document, "Point 9,9").unwrap();
        let point_id = document.objects().nth(1).unwrap().id();
        registry.execute(&mut document, "SelAll").unwrap();

        assert_eq!(
            registry.execute(&mut document, "X").unwrap(),
            "Exploded 1 polyline(s) into 4 line(s); 1 object(s) unchanged"
        );
        assert_eq!(document.objects().len(), 5);
        assert!(document.object(point_id).is_some());
        assert_eq!(
            document
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::Line(_)))
                .count(),
            4
        );
        assert!(
            document
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::Line(_)))
                .all(|object| object.attributes().layer_id() == construction)
        );
        assert_eq!(document.selected_object_count(), 5);
        assert_eq!(document.undo_label(), Some("Explode"));

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(
            document
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::Polyline(_)))
                .count(),
            1
        );
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "Explode"),
            Err(CommandError::NoExplodablePolylines)
        ));
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn three_dm_polyline_recognition_preserves_noncanonical_nurbs() {
        let points = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 1.0, 0.0).unwrap(),
        ];
        let canonical = Polyline3::try_new(points.clone(), Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        assert!(exported_polyline(&canonical, Tolerance::DEFAULT).is_some());

        let nonuniform = NurbsCurve::try_new(1, points, vec![0.0, 0.0, 0.25, 1.0, 1.0]).unwrap();
        assert!(exported_polyline(&nonuniform, Tolerance::DEFAULT).is_none());
    }

    #[test]
    fn creates_a_clamped_control_point_curve() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,0 2,3 5,3 8,0")
            .unwrap();
        let object = document.objects().next().unwrap();
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("expected a NURBS curve")
        };
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 4);
        assert_eq!(
            curve.evaluate(0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap()
        );
        assert_eq!(
            curve.evaluate(1.0).unwrap(),
            Point3::try_new(8.0, 0.0, 0.0).unwrap()
        );
    }

    #[test]
    fn curve_preserves_controls_and_matches_rhino_degree_rules() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let controls = [
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.5).unwrap(),
            Point3::try_new(4.0, -1.0, 2.0).unwrap(),
            Point3::try_new(4.5, 3.0, -0.5).unwrap(),
            Point3::try_new(10.0, 0.0, 1.0).unwrap(),
        ];
        let message = registry
            .execute(
                &mut document,
                "Curve 0,0,0 1,2,.5 4,-1,2 4.5,3,-.5 10,0,1 Degree=3",
            )
            .unwrap();
        assert!(message.contains("degree 3 curve"));
        let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
            panic!("expected a control-point NURBS curve")
        };
        assert_eq!(curve.degree(), 3);
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            controls
        );
        let domain_end = 17.976_753_701_093_052;
        for (actual, expected) in curve.knots().iter().zip([
            0.0,
            0.0,
            0.0,
            0.0,
            domain_end / 2.0,
            domain_end,
            domain_end,
            domain_end,
            domain_end,
        ]) {
            assert!((actual - expected).abs() <= 2.0e-14);
        }

        let message = registry
            .execute(&mut document, "Curve Degree=5 0,0 2,3 10,0")
            .unwrap();
        assert!(message.contains("degree 2 curve"));
        let Geometry::NurbsCurve(lowered) = document.objects().nth(1).unwrap().geometry() else {
            panic!("expected a lowered-degree NURBS curve")
        };
        assert_eq!(lowered.degree(), 2);

        let message = registry
            .execute(
                &mut document,
                "Curve Degree=15 0,0 1,0 2,0 3,0 4,0 5,0 6,0 7,0 8,0 9,0 10,0 11,0",
            )
            .unwrap();
        assert!(message.contains("degree 11 curve"));
        let Geometry::NurbsCurve(clamped) = document.objects().nth(2).unwrap().geometry() else {
            panic!("expected a maximum-degree NURBS curve")
        };
        assert_eq!(clamped.degree(), 11);
    }

    #[test]
    fn curve_supports_smooth_periodic_and_sharp_closed_seams() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let message = registry
            .execute(
                &mut document,
                "Curve Degree=4 Close=Smooth 0,0,0 1,2,.5 4,-1,2 4.5,3,-.5 10,0,1 8,-4,2 2,-3,-1 -2,1,.25",
            )
            .unwrap();
        assert!(message.contains("periodic degree 4 curve"));
        let Geometry::NurbsCurve(periodic) = document.objects().next().unwrap().geometry() else {
            panic!("expected a periodic control-point curve")
        };
        assert_eq!(periodic.degree(), 4);
        assert_eq!(periodic.control_points().len(), 12);
        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());

        let message = registry
            .execute(
                &mut document,
                "Curve Close=Sharp Degree=5 0,0,0 2,3,1 10,0,0",
            )
            .unwrap();
        assert!(message.contains("sharp closed degree 3 curve"));
        let Geometry::NurbsCurve(sharp) = document.objects().nth(1).unwrap().geometry() else {
            panic!("expected a sharp closed control-point curve")
        };
        assert_eq!(sharp.degree(), 3);
        assert_eq!(sharp.control_points().len(), 4);
        assert!(!sharp.is_periodic());
        assert!(sharp.is_closed().unwrap());

        let message = registry
            .execute(&mut document, "Curve Degree=1 Close=Smooth 0,0 2,3 10,0")
            .unwrap();
        assert!(message.contains("closed degree 1 curve"));
        let Geometry::NurbsCurve(linear) = document.objects().nth(2).unwrap().geometry() else {
            panic!("expected a closed degree-one control-point curve")
        };
        assert!(linear.is_closed().unwrap());
        assert!(!linear.is_periodic());
    }

    #[test]
    fn invalid_curve_arguments_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for input in [
            "Curve 0,0",
            "Curve 0,0 1,1 Degree=3 Degree=4",
            "Curve 0,0 1,1 Knots=Chord",
            "Curve 0,0 1,1 2,0 Close=Bad",
            "Curve 0,0 1,1 Close=Smooth",
            "Curve 0,0 1,1 2,0 Close=Sharp Close=Smooth",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(document.objects().len(), 0);
        }
    }

    #[test]
    fn creates_interpolated_curves_with_rhino_style_options() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let message = registry
            .execute(&mut document, "InterpCrv 0,0 1,2 4,-1 6,0")
            .unwrap();
        assert!(message.contains("degree 3 interpolated curve"));
        let Geometry::NurbsCurve(default_curve) = document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interpolated NURBS curve")
        };
        let expected_points = [
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(4.0, -1.0, 0.0).unwrap(),
            Point3::try_new(6.0, 0.0, 0.0).unwrap(),
        ];
        let mut parameter = 0.0;
        for (index, expected) in expected_points.into_iter().enumerate() {
            assert!(
                default_curve
                    .evaluate(parameter)
                    .unwrap()
                    .is_near(expected, Tolerance::DEFAULT)
            );
            if let Some(next) = expected_points.get(index + 1) {
                parameter += expected.distance_to(*next).unwrap();
            }
        }

        registry
            .execute(
                &mut document,
                "InterpCurve Knots=SqrtChrd Close=Smooth 0,0 3,0 3,2 0,2",
            )
            .unwrap();
        let Geometry::NurbsCurve(periodic) = document.objects().nth(1).unwrap().geometry() else {
            panic!("expected a periodic NURBS curve")
        };
        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());

        registry
            .execute(
                &mut document,
                "InterpCrv Degree=3 StartTangent=-1,0,0 EndTangent=0,1,0 10,0 12,2",
            )
            .unwrap();
        let Geometry::NurbsCurve(tangent_curve) = document.objects().nth(2).unwrap().geometry()
        else {
            panic!("expected a tangent-constrained NURBS curve")
        };
        assert_eq!(tangent_curve.degree(), 3);
        assert_eq!(tangent_curve.control_points().len(), 4);
        assert!(tangent_curve.control_points()[1].point().x() < 10.0);

        let message = registry
            .execute(&mut document, "InterpCrv 20,0 24,0")
            .unwrap();
        assert!(message.contains("degree 3 interpolated curve"));

        let message = registry
            .execute(&mut document, "InterpCrv Degree=1 30,0 34,0")
            .unwrap();
        assert!(message.contains("degree 1 interpolated curve"));
    }

    #[test]
    fn invalid_interp_crv_options_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for input in [
            "InterpCrv 0,0 1,1 Degree=2",
            "InterpCrv 0,0 1,1 Knots=Bad",
            "InterpCrv 0,0 1,1 Degree=3 Degree=3",
            "InterpCrv 0,0 1,1 Close=Smooth StartTangent=1,0,0",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(document.objects().len(), 0);
        }
    }

    #[test]
    fn creates_and_tessellates_a_four_corner_surface_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let message = registry
            .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,2 0,3,2")
            .unwrap();
        assert!(message.contains("four-corner NURBS surface"));
        let Geometry::NurbsSurface(surface) = document.objects().next().unwrap().geometry() else {
            panic!("expected a NURBS surface")
        };
        assert_eq!(surface.degree_u(), 1);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(
            surface.evaluate(0.5, 0.5).unwrap(),
            Point3::try_new(2.0, 1.5, 1.0).unwrap()
        );
        assert_eq!(
            combined_document_mesh(&document).unwrap().triangles().len(),
            2 * SURFACE_EXPORT_SAMPLES_PER_SPAN * SURFACE_EXPORT_SAMPLES_PER_SPAN
        );
        assert_eq!(document.undo_label(), Some("SrfPt"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 0);

        assert!(
            registry
                .execute(&mut document, "SrfPt 0,0 1,0 2,0 3,0")
                .is_err()
        );
        assert_eq!(document.objects().len(), 0);
    }

    #[test]
    fn undo_and_redo_commands_replay_a_model_edit() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        assert_eq!(document.objects().len(), 1);

        assert_eq!(
            registry.execute(&mut document, "Undo").unwrap(),
            "Undid Point"
        );
        assert_eq!(document.objects().len(), 0);
        assert_eq!(
            registry.execute(&mut document, "Redo").unwrap(),
            "Redid Point"
        );
        assert_eq!(document.objects().len(), 1);
    }

    #[test]
    fn compound_layer_command_is_one_undo_step() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let default_layer = document.current_layer_id();
        registry
            .execute(&mut document, "Layer Construction")
            .unwrap();
        assert_eq!(document.layers().len(), 2);
        assert_ne!(document.current_layer_id(), default_layer);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.layers().len(), 1);
        assert_eq!(document.current_layer_id(), default_layer);
    }

    #[test]
    fn layer_command_manages_named_state_and_preserves_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Layer New Construction")
            .unwrap();
        registry
            .execute(
                &mut document,
                "Layer Rename Construction => Reference Geometry",
            )
            .unwrap();
        registry
            .execute(&mut document, "Layer Color 12,34,56 Reference Geometry")
            .unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry
            .execute(&mut document, "Layer Hide Reference Geometry")
            .unwrap();

        let layer = document.layer_by_name("reference geometry").unwrap();
        assert_eq!(layer.color(), ColorRgb::new(12, 34, 56));
        assert!(!layer.is_visible());
        registry.execute(&mut document, "Undo").unwrap();
        assert!(
            document
                .layer_by_name("Reference Geometry")
                .unwrap()
                .is_visible()
        );

        registry
            .execute(&mut document, "Layer Lock Reference Geometry")
            .unwrap();
        assert!(
            registry
                .execute(&mut document, "Layer Current Reference Geometry")
                .is_err()
        );
        registry
            .execute(&mut document, "Layer Unlock Reference Geometry")
            .unwrap();
        registry
            .execute(&mut document, "Layer Current Reference Geometry")
            .unwrap();
    }

    #[test]
    fn change_and_copy_to_layer_preserve_rhino_identity_selection_and_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let default = document.current_layer_id();
        registry
            .execute(&mut document, "Layer New Target Parts")
            .unwrap();
        let target = document.layer_by_name("Target Parts").unwrap().id();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group Assembly").unwrap();
        let original_ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();

        assert_eq!(
            registry
                .execute(&mut document, "ChangeLayer target parts")
                .unwrap(),
            "Changed 1 object(s) to layer 'target parts'"
        );
        assert_eq!(document.undo_label(), Some("ChangeLayer"));
        assert_eq!(document.current_layer_id(), default);
        assert_eq!(document.selected_object_count(), 2);
        assert!(
            original_ids
                .iter()
                .all(|id| { document.object(*id).unwrap().attributes().layer_id() == target })
        );
        assert_eq!(
            document.group_by_name("Assembly").unwrap().members().len(),
            2
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "CopyToLayer Target Parts")
                .unwrap(),
            "Copied 1 object(s) to layer 'Target Parts'"
        );
        assert_eq!(document.undo_label(), Some("CopyToLayer"));
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.selected_object_count(), 2);
        assert!(original_ids.iter().all(|id| document.is_selected(*id)));
        let copy = document
            .objects()
            .find(|object| !original_ids.contains(&object.id()))
            .unwrap();
        assert_eq!(copy.attributes().layer_id(), target);
        assert!(!document.is_selected(copy.id()));
        assert_eq!(
            document.group_by_name("Group01").unwrap().members().len(),
            1
        );
        assert_eq!(document.current_layer_id(), default);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert!(document.group_by_name("Group01").is_none());
        document.clear_selection();
        assert!(matches!(
            registry.execute(&mut document, "ChangeLayer Target Parts"),
            Err(CommandError::NoObjectsSelected)
        ));
    }

    #[test]
    fn layer_delete_and_color_validation_are_safe() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert!(
            registry
                .execute(&mut document, "Layer Color 256,0,0 Default")
                .is_err()
        );
        assert_eq!(
            document.layer(document.current_layer_id()).unwrap().color(),
            ColorRgb::BLACK
        );

        registry.execute(&mut document, "Layer New Empty").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry
            .execute(&mut document, "Layer Delete Empty")
            .unwrap();
        assert!(document.layer_by_name("Empty").is_none());
        registry.execute(&mut document, "Undo").unwrap();
        assert!(document.layer_by_name("Empty").is_some());
    }

    #[test]
    fn group_and_ungroup_all_visible_unlocked_objects_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        registry
            .execute(&mut document, "Layer New Hidden Geometry")
            .unwrap();
        registry.execute(&mut document, "Point 2,0,0").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry
            .execute(&mut document, "Layer Hide Hidden Geometry")
            .unwrap();
        registry
            .execute(&mut document, "Layer New Locked Geometry")
            .unwrap();
        registry.execute(&mut document, "Point 3,0,0").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry
            .execute(&mut document, "Layer Lock Locked Geometry")
            .unwrap();

        registry.execute(&mut document, "Group All Pair").unwrap();
        let group = document.group_by_name("Pair").unwrap();
        assert_eq!(group.members().len(), 2);
        assert!(document.group_by_name("pair").is_none());
        registry.execute(&mut document, "Group All PAIR").unwrap();
        assert_eq!(document.groups().len(), 2);

        registry.execute(&mut document, "Ungroup Pair").unwrap();
        assert_eq!(document.groups().len(), 1);
        assert!(document.group_by_name("PAIR").is_some());
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.group_by_name("Pair").unwrap().members().len(), 2);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.groups().len(), 1);
    }

    #[test]
    fn set_object_name_matches_rhino_counter_order_and_preserves_selection_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0").unwrap();
        registry.execute(&mut document, "Point 1,0").unwrap();
        registry.execute(&mut document, "Point 2,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group").unwrap();
        assert_eq!(
            document.group_by_name("Group01").unwrap().members().len(),
            3
        );
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();

        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "SetObjectName _AppendCounter=_Yes \"Fastener Part\"",
                )
                .unwrap(),
            "Named 3 object(s)"
        );
        assert_eq!(document.undo_label(), Some("SetObjectName"));
        assert_eq!(
            ids.iter()
                .map(|id| document.object(*id).unwrap().attributes().name())
                .collect::<Vec<_>>(),
            vec![
                Some("Fastener Part 0"),
                Some("Fastener Part 1"),
                Some("Fastener Part 2")
            ]
        );
        assert_eq!(document.selected_object_count(), 3);
        assert_eq!(
            document.group_by_name("Group01").unwrap().members().len(),
            3
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert!(
            document
                .objects()
                .all(|object| object.attributes().name().is_none())
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "SetObjectName Shared Name")
                .unwrap(),
            "Named 3 object(s)"
        );
        assert!(
            document
                .objects()
                .all(|object| object.attributes().name() == Some("Shared Name"))
        );
        assert_eq!(
            registry
                .execute(&mut document, "SetObjectName \"\"")
                .unwrap(),
            "Cleared names on 3 object(s)"
        );
        assert!(
            document
                .objects()
                .all(|object| object.attributes().name().is_none())
        );

        let before = document.objects().cloned().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "SetObjectName AppendCounter=Maybe Part"),
            Err(CommandError::Usage(SET_OBJECT_NAME_USAGE))
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.undo_label(), history.as_deref());
        document.clear_selection();
        assert!(matches!(
            registry.execute(&mut document, "SetObjectName Part"),
            Err(CommandError::NoObjectsSelected)
        ));
    }

    #[test]
    fn set_object_color_preserves_identity_groups_selection_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0").unwrap();
        registry.execute(&mut document, "Point 1,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Group Colors").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();

        assert_eq!(
            registry
                .execute(&mut document, "_SetObjectColor 12,34,56")
                .unwrap(),
            "Set 2 object color(s) to 12,34,56"
        );
        assert_eq!(document.undo_label(), Some("SetObjectColor"));
        for id in &ids {
            let attributes = document.object(*id).unwrap().attributes();
            assert_eq!(attributes.color_source(), ObjectColorSource::Object);
            assert_eq!(attributes.object_color(), ColorRgb::new(12, 34, 56));
        }
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.group_by_name("Colors").unwrap().members().len(), 2);

        registry.execute(&mut document, "Undo").unwrap();
        assert!(
            document
                .objects()
                .all(|object| { object.attributes().color_source() == ObjectColorSource::Layer })
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "SetObjectColor _ByLayer")
                .unwrap(),
            "Set 2 object color(s) to ByLayer"
        );
        for object in document.objects() {
            assert_eq!(object.attributes().color_source(), ObjectColorSource::Layer);
            assert_eq!(
                object.attributes().object_color(),
                ColorRgb::new(12, 34, 56)
            );
        }

        let before = document.objects().cloned().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "SetObjectColor 256,0,0"),
            Err(CommandError::InvalidColor(_))
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn selection_commands_drive_group_ungroup_and_delete() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        let history_label = document.undo_label().map(str::to_owned);

        assert_eq!(
            registry.execute(&mut document, "SelAll").unwrap(),
            "Selected 2 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelNone").unwrap(),
            "Deselected 2 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "Invert").unwrap(),
            "Selected 2 object(s)"
        );
        assert_eq!(document.undo_label(), history_label.as_deref());

        registry.execute(&mut document, "Group Pair").unwrap();
        assert_eq!(document.group_by_name("Pair").unwrap().members().len(), 2);
        let first = document.objects().next().unwrap().id();
        document.clear_selection();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.selected_object_count(), 2);

        registry.execute(&mut document, "Ungroup").unwrap();
        assert_eq!(document.groups().len(), 0);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.groups().len(), 1);

        registry.execute(&mut document, "Delete").unwrap();
        assert_eq!(document.objects().len(), 0);
        assert_eq!(document.groups().len(), 0);
        assert_eq!(document.undo_label(), Some("Delete"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
    }

    #[test]
    fn action_order_selection_commands_match_rhino_replace_add_and_toggle_modes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for x in 0..4 {
            registry
                .execute(&mut document, &format!("Point {x},0,0"))
                .unwrap();
        }
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let selected = |document: &Document| {
            document
                .selected_object_ids()
                .collect::<BTreeSet<ObjectId>>()
        };
        let history = document.undo_label().map(str::to_owned);

        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelLast").unwrap(),
            "Selection contains 1 object(s)"
        );
        assert_eq!(selected(&document), BTreeSet::from([ids[3]]));
        registry.execute(&mut document, "SelPrev").unwrap();
        assert_eq!(selected(&document), BTreeSet::from([ids[0]]));
        registry.execute(&mut document, "SelPrev").unwrap();
        assert_eq!(selected(&document), BTreeSet::from([ids[3]]));

        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "SelLast DeselectOthersBeforeSelect=No",)
                .unwrap(),
            "Selection contains 2 object(s)"
        );
        assert_eq!(selected(&document), BTreeSet::from([ids[0], ids[3]]));

        document
            .select_objects([ids[0], ids[1]], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "SelNone").unwrap();
        document.select_object(ids[2], SelectionMode::Add).unwrap();
        registry.execute(&mut document, "SelPrev").unwrap();
        assert_eq!(selected(&document), BTreeSet::from([ids[0], ids[1]]));
        registry.execute(&mut document, "SelPrev").unwrap();
        assert_eq!(selected(&document), BTreeSet::from([ids[2]]));

        document
            .select_objects([ids[0], ids[1]], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "SelNone").unwrap();
        document.select_object(ids[2], SelectionMode::Add).unwrap();
        registry
            .execute(&mut document, "SelPrev DeselectOthersBeforeSelect No")
            .unwrap();
        assert_eq!(
            selected(&document),
            BTreeSet::from([ids[0], ids[1], ids[2]])
        );

        let before = selected(&document);
        assert!(matches!(
            registry.execute(&mut document, "SelLast DeselectOthersBeforeSelect=Maybe"),
            Err(CommandError::Usage(
                "SelLast [DeselectOthersBeforeSelect=Yes|No]"
            ))
        ));
        assert_eq!(selected(&document), before);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn hide_show_lock_and_unlock_preserve_identity_groups_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        registry.execute(&mut document, "Point 2,0,0").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        document.select_object(ids[1], SelectionMode::Add).unwrap();
        registry.execute(&mut document, "Group Pair").unwrap();
        let group = document.group_by_name("Pair").unwrap().id();
        document.clear_selection();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(document.selected_object_count(), 2);

        assert_eq!(
            registry.execute(&mut document, "Hide").unwrap(),
            "Hid 2 object(s)"
        );
        assert_eq!(document.undo_label(), Some("Hide"));
        assert_eq!(document.selected_object_count(), 0);
        for id in &ids[..2] {
            let object = document.object(*id).unwrap();
            assert_eq!(object.id(), *id);
            assert!(!object.attributes().is_visible());
            assert!(!document.is_object_selectable(*id));
        }
        assert_eq!(document.group(group).unwrap().members().len(), 2);
        assert!(document.object(ids[2]).unwrap().attributes().is_visible());

        document
            .select_object(ids[2], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "Show").unwrap(),
            "Showed 2 object(s)"
        );
        assert_eq!(document.undo_label(), Some("Show"));
        assert!(document.is_selected(ids[2]));
        assert!(
            ids[..2]
                .iter()
                .all(|id| document.object(*id).unwrap().attributes().is_visible())
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert!(
            ids[..2]
                .iter()
                .all(|id| !document.object(*id).unwrap().attributes().is_visible())
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert!(
            ids[..2]
                .iter()
                .all(|id| document.object(*id).unwrap().attributes().is_visible())
        );

        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "Lock").unwrap(),
            "Locked 2 object(s)"
        );
        assert_eq!(document.undo_label(), Some("Lock"));
        assert_eq!(document.selected_object_count(), 0);
        for id in &ids[..2] {
            assert!(document.object(*id).unwrap().attributes().is_locked());
            assert_eq!(
                document.select_object(*id, SelectionMode::Replace),
                Err(DocumentError::ObjectNotSelectable(*id))
            );
        }
        assert_eq!(
            registry.execute(&mut document, "Unlock").unwrap(),
            "Unlocked 2 object(s)"
        );
        assert_eq!(document.undo_label(), Some("Unlock"));
        assert!(
            ids[..2]
                .iter()
                .all(|id| !document.object(*id).unwrap().attributes().is_locked())
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert!(
            ids[..2]
                .iter()
                .all(|id| document.object(*id).unwrap().attributes().is_locked())
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert!(
            ids[..2]
                .iter()
                .all(|id| !document.object(*id).unwrap().attributes().is_locked())
        );

        let history = document.undo_label().map(str::to_owned);
        assert_eq!(
            registry.execute(&mut document, "Show").unwrap(),
            "Showed 0 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "Unlock").unwrap(),
            "Unlocked 0 object(s)"
        );
        assert_eq!(document.undo_label(), history.as_deref());
        assert!(matches!(
            registry.execute(&mut document, "Hide unexpected"),
            Err(CommandError::Usage("Hide"))
        ));
        document.clear_selection();
        assert!(matches!(
            registry.execute(&mut document, "Lock"),
            Err(CommandError::NoObjectsSelected)
        ));
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(document.group(group).unwrap().members().len(), 2);
    }

    #[test]
    fn hide_swap_and_lock_swap_are_reversible_three_mode_involutions() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        registry.execute(&mut document, "Point 2,0,0").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document.set_objects_visibility([ids[1]], false).unwrap();
        document.set_objects_locked([ids[2]], true).unwrap();
        let modes = |document: &Document| {
            ids.iter()
                .map(|id| {
                    let attributes = document.object(*id).unwrap().attributes();
                    if !attributes.is_visible() {
                        "hidden"
                    } else if attributes.is_locked() {
                        "locked"
                    } else {
                        "normal"
                    }
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);

        assert_eq!(
            registry.execute(&mut document, "HideSwap").unwrap(),
            "Swapped hidden state on 2 object(s)"
        );
        assert_eq!(document.undo_label(), Some("HideSwap"));
        assert_eq!(modes(&document), vec!["hidden", "normal", "locked"]);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(modes(&document), vec!["hidden", "normal", "locked"]);
        registry.execute(&mut document, "HideSwap").unwrap();
        assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);

        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "LockSwap").unwrap(),
            "Swapped lock state on 2 object(s)"
        );
        assert_eq!(document.undo_label(), Some("LockSwap"));
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(modes(&document), vec!["locked", "hidden", "normal"]);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(modes(&document), vec!["locked", "hidden", "normal"]);
        registry.execute(&mut document, "LockSwap").unwrap();
        assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);

        let before = document.objects().cloned().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "HideSwap unexpected"),
            Err(CommandError::Usage("HideSwap"))
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn isolate_commands_restore_only_their_own_object_modes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for x in 0..4 {
            registry
                .execute(&mut document, &format!("Point {x},0,0"))
                .unwrap();
        }
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document.set_objects_visibility([ids[2]], false).unwrap();
        document.set_objects_locked([ids[3]], true).unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        let modes = |document: &Document| {
            ids.iter()
                .map(|id| {
                    let attributes = document.object(*id).unwrap().attributes();
                    if !attributes.is_visible() {
                        "hidden"
                    } else if attributes.is_locked() {
                        "locked"
                    } else {
                        "normal"
                    }
                })
                .collect::<Vec<_>>()
        };
        let initial = vec!["normal", "normal", "hidden", "locked"];

        assert_eq!(
            registry.execute(&mut document, "Isolate").unwrap(),
            "Isolated 1 object(s)"
        );
        assert_eq!(document.undo_label(), Some("Isolate"));
        assert_eq!(document.isolated_hidden_object_count(), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
        assert_eq!(
            modes(&document),
            vec!["normal", "hidden", "hidden", "locked"]
        );
        assert_eq!(
            registry.execute(&mut document, "Isolate").unwrap(),
            "Isolated 0 object(s)"
        );
        assert_eq!(document.undo_label(), Some("Isolate"));
        assert_eq!(
            registry.execute(&mut document, "Unisolate").unwrap(),
            "Unisolated 1 object(s)"
        );
        assert_eq!(modes(&document), initial);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.isolated_hidden_object_count(), 1);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(modes(&document), initial);

        assert_eq!(
            registry.execute(&mut document, "IsolateLock").unwrap(),
            "Locked 1 non-selected object(s)"
        );
        assert_eq!(document.undo_label(), Some("IsolateLock"));
        assert_eq!(document.isolated_locked_object_count(), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
        assert_eq!(
            modes(&document),
            vec!["normal", "locked", "hidden", "locked"]
        );
        assert_eq!(
            registry.execute(&mut document, "UnisolateLock").unwrap(),
            "Unlocked 1 isolated object(s)"
        );
        assert_eq!(modes(&document), initial);

        let before = document.objects().cloned().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "Unisolate extra"),
            Err(CommandError::Usage("Unisolate"))
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.undo_label(), history.as_deref());
        document.clear_selection();
        assert!(matches!(
            registry.execute(&mut document, "Isolate"),
            Err(CommandError::NoObjectsSelected)
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn attribute_selection_matches_names_layers_and_exact_case_sensitive_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let default = document.current_layer_id();
        let add_named_point = |document: &mut Document, x, attributes: ObjectAttributes| {
            document
                .add_geometry_with_attributes(
                    Geometry::Point(Point3::try_new(x, 0.0, 0.0).unwrap()),
                    attributes,
                )
                .unwrap()
        };
        let unrelated = add_named_point(&mut document, 0.0, ObjectAttributes::on_layer(default));
        let upper = add_named_point(
            &mut document,
            1.0,
            ObjectAttributes::on_layer(default).with_name("BoltA"),
        );
        let lower = add_named_point(
            &mut document,
            2.0,
            ObjectAttributes::on_layer(default).with_name("bolta"),
        );
        let wildcard = add_named_point(
            &mut document,
            3.0,
            ObjectAttributes::on_layer(default).with_name("BoltLong"),
        );
        let peer = add_named_point(
            &mut document,
            4.0,
            ObjectAttributes::on_layer(default).with_name("Peer"),
        );
        let hidden_object = add_named_point(
            &mut document,
            5.0,
            ObjectAttributes::on_layer(default)
                .with_name("BoltA")
                .with_visibility(false),
        );
        let locked_object = add_named_point(
            &mut document,
            6.0,
            ObjectAttributes::on_layer(default)
                .with_name("BoltA")
                .with_locked(true),
        );

        let hidden_layer = document
            .add_layer("Hidden Parts", ColorRgb::new(10, 20, 30))
            .unwrap();
        let hidden_layer_match = add_named_point(
            &mut document,
            7.0,
            ObjectAttributes::on_layer(hidden_layer).with_name("BoltA"),
        );
        let hidden_on_hidden_layer = add_named_point(
            &mut document,
            8.0,
            ObjectAttributes::on_layer(hidden_layer)
                .with_name("BoltA")
                .with_visibility(false),
        );
        document.set_layer_visibility(hidden_layer, false).unwrap();

        let locked_layer = document
            .add_layer("Locked Parts", ColorRgb::new(40, 50, 60))
            .unwrap();
        let locked_layer_match = add_named_point(
            &mut document,
            9.0,
            ObjectAttributes::on_layer(locked_layer).with_name("BoltA"),
        );
        let locked_on_locked_layer = add_named_point(
            &mut document,
            10.0,
            ObjectAttributes::on_layer(locked_layer)
                .with_name("BoltA")
                .with_locked(true),
        );
        document.set_layer_locked(locked_layer, true).unwrap();
        document
            .add_group(Some("Team".to_owned()), [upper, peer, locked_object])
            .unwrap();
        document
            .add_group(Some("team".to_owned()), [lower])
            .unwrap();
        document
            .add_group(Some("Overlap".to_owned()), [upper, wildcard])
            .unwrap();
        let history = document.undo_label().map(str::to_owned);

        document
            .select_object(unrelated, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelName BOLT?").unwrap(),
            "Selected 3 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([unrelated, upper, lower])
        );
        assert!(!document.is_selected(wildcard));
        assert!(!document.is_selected(peer));
        assert!(!document.is_selected(hidden_object));
        assert!(!document.is_selected(locked_object));
        assert!(!document.is_selected(hidden_layer_match));
        assert!(!document.is_selected(locked_layer_match));

        registry.execute(&mut document, "SelName bolt*").unwrap();
        assert!(document.is_selected(wildcard));
        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelName \"\"").unwrap(),
            "Selected 1 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            [unrelated]
        );

        assert_eq!(
            registry.execute(&mut document, "SelGroup Team").unwrap(),
            "Selected 3 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([unrelated, upper, peer])
        );
        assert!(!document.is_selected(wildcard));
        assert_eq!(
            registry.execute(&mut document, "SelGroup team").unwrap(),
            "Selected 4 object(s)"
        );
        assert!(document.is_selected(lower));
        assert_eq!(
            registry.execute(&mut document, "SelGroup TEAM").unwrap(),
            "Selected 4 object(s)"
        );

        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "SelLayer \"Hidden Parts\"")
                .unwrap(),
            "Selected 1 object(s)"
        );
        assert!(document.layer(hidden_layer).unwrap().is_visible());
        assert!(document.is_selected(hidden_layer_match));
        assert!(!document.is_selected(hidden_on_hidden_layer));
        assert!(!document.is_selected(peer));
        assert_eq!(
            registry.execute(&mut document, "SelLayer LOCKED*").unwrap(),
            "Selected 2 object(s)"
        );
        assert!(!document.layer(locked_layer).unwrap().is_locked());
        assert!(document.is_selected(locked_layer_match));
        assert!(!document.is_selected(locked_on_locked_layer));
        assert_eq!(document.undo_label(), history.as_deref());

        let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
        for command in ["SelName", "SelLayer", "SelGroup", "SelName \"unterminated"] {
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                selection
            );
        }
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn sel_color_adds_resolved_selectable_matches_and_skips_group_members() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let default = document.current_layer_id();
        let target = ColorRgb::new(200, 10, 20);
        let target_layer = document.add_layer("Target", target).unwrap();
        let add_point = |document: &mut Document, x, attributes: ObjectAttributes| {
            document
                .add_geometry_with_attributes(
                    Geometry::Point(Point3::try_new(x, 0.0, 0.0).unwrap()),
                    attributes,
                )
                .unwrap()
        };

        let prior = add_point(&mut document, 0.0, ObjectAttributes::on_layer(default));
        let by_layer = add_point(&mut document, 1.0, ObjectAttributes::on_layer(target_layer));
        let by_object = add_point(
            &mut document,
            2.0,
            ObjectAttributes::on_layer(default).with_object_color(target),
        );
        let by_material = add_point(
            &mut document,
            3.0,
            ObjectAttributes::on_layer(target_layer)
                .with_object_color(ColorRgb::new(1, 2, 3))
                .with_color_source(ObjectColorSource::Material),
        );
        let by_parent = add_point(
            &mut document,
            4.0,
            ObjectAttributes::on_layer(target_layer)
                .with_object_color(ColorRgb::new(4, 5, 6))
                .with_color_source(ObjectColorSource::Parent),
        );
        let grouped_match = add_point(
            &mut document,
            5.0,
            ObjectAttributes::on_layer(default).with_object_color(target),
        );
        let grouped_peer = add_point(&mut document, 6.0, ObjectAttributes::on_layer(default));
        let hidden = add_point(
            &mut document,
            7.0,
            ObjectAttributes::on_layer(default)
                .with_object_color(target)
                .with_visibility(false),
        );
        let locked = add_point(
            &mut document,
            8.0,
            ObjectAttributes::on_layer(default)
                .with_object_color(target)
                .with_locked(true),
        );
        document
            .add_group(Some("Mixed".to_owned()), [grouped_match, grouped_peer])
            .unwrap();
        document
            .select_objects_direct([prior], SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);

        assert_eq!(
            registry
                .execute(&mut document, "_SelColor 200,10,20")
                .unwrap(),
            "Selected 5 object(s) with display color 200,10,20"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([prior, by_layer, by_object, by_material, by_parent])
        );
        assert!(!document.is_selected(grouped_match));
        assert!(!document.is_selected(grouped_peer));
        assert!(!document.is_selected(hidden));
        assert!(!document.is_selected(locked));
        assert_eq!(document.undo_label(), history.as_deref());

        let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
        for command in [
            "SelColor",
            "SelColor 1,2",
            "SelColor 256,0,0",
            "SelColor ByLayer",
        ] {
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                selection
            );
        }
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn duplicate_selection_commands_include_or_retain_originals_without_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let unique = document
            .add_geometry(Geometry::Point(Point3::try_new(9.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let original = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()))
            .unwrap();
        let duplicate = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()))
            .unwrap();
        let history = document.undo_label().map(str::to_owned);

        document
            .select_object(unique, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelDupAll").unwrap(),
            "Selected 3 object(s)"
        );
        assert!(document.is_selected(original));
        assert!(document.is_selected(duplicate));

        document.clear_selection();
        document
            .select_object(unique, SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelDup").unwrap(),
            "Selected 2 object(s)"
        );
        assert!(!document.is_selected(original));
        assert!(document.is_selected(duplicate));
        assert_eq!(document.undo_label(), history.as_deref());

        let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
        assert!(
            registry
                .execute(&mut document, "SelDup unexpected")
                .is_err()
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            selection
        );
    }

    #[test]
    fn type_selection_commands_add_only_visible_unlocked_matches_without_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry
            .execute(&mut document, "Layer New Hidden Markers")
            .unwrap();
        registry.execute(&mut document, "Point 9,9,9").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry
            .execute(&mut document, "Layer Hide Hidden Markers")
            .unwrap();
        registry
            .execute(&mut document, "Layer New Locked Markers")
            .unwrap();
        registry.execute(&mut document, "Point 8,8,8").unwrap();
        registry
            .execute(&mut document, "Layer Current Default")
            .unwrap();
        registry
            .execute(&mut document, "Layer Lock Locked Markers")
            .unwrap();
        registry.execute(&mut document, "Line 0,0 4,0").unwrap();
        registry.execute(&mut document, "Circle 10,0 2").unwrap();
        registry
            .execute(&mut document, "Polyline 0,0 2,0 2,2")
            .unwrap();
        registry
            .execute(&mut document, "Rectangle 20,0 24,3")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 3 30,0 31,2 33,2 34,0")
            .unwrap();
        registry
            .execute(&mut document, "SrfPt 0,10 2,10 2,12 0,12")
            .unwrap();
        document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![
                        Point3::try_new(10.0, 10.0, 0.0).unwrap(),
                        Point3::try_new(12.0, 10.0, 0.0).unwrap(),
                        Point3::try_new(10.0, 12.0, 0.0).unwrap(),
                    ],
                    vec![[0, 1, 2]],
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![
                        Point3::try_new(20.0, 10.0, 0.0).unwrap(),
                        Point3::try_new(22.0, 10.0, 0.0).unwrap(),
                        Point3::try_new(20.0, 12.0, 0.0).unwrap(),
                        Point3::try_new(20.0, 10.0, 2.0).unwrap(),
                    ],
                    vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let history = document.undo_label().map(str::to_owned);

        assert_eq!(
            registry.execute(&mut document, "SelPt").unwrap(),
            "Selected 1 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelOpenCrv").unwrap(),
            "Selected 4 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelClosedCrv").unwrap(),
            "Selected 6 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelCrv").unwrap(),
            "Selected 6 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelSrf").unwrap(),
            "Selected 7 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelMesh").unwrap(),
            "Selected 9 object(s)"
        );
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(document.objects().len(), 11);
        assert_eq!(
            document
                .objects()
                .filter(|object| document.is_selected(object.id()))
                .count(),
            9
        );

        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelOpenMesh").unwrap(),
            "Selected 1 object(s)"
        );
        assert_eq!(
            registry.execute(&mut document, "SelClosedMesh").unwrap(),
            "Selected 2 object(s)"
        );
        assert!(
            document
                .selected_objects()
                .all(|object| matches!(object.geometry(), Geometry::Mesh(_)))
        );
        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelLine").unwrap(),
            "Selected 1 object(s)"
        );
        assert!(
            document
                .selected_objects()
                .all(|object| matches!(object.geometry(), Geometry::Line(_)))
        );
        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelPolyline").unwrap(),
            "Selected 2 object(s)"
        );
        assert!(
            document
                .selected_objects()
                .all(|object| matches!(object.geometry(), Geometry::Polyline(_)))
        );
        assert!(registry.execute(&mut document, "SelMesh extra").is_err());
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn curve_shape_selection_matches_rhino_nurbs_span_and_planarity_rules() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0 3,0").unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,1 1,1 2,1 3,1")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,2 1,2 2,2 3,2 4,2")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 2 0,3 1,3 2,3")
            .unwrap();
        registry
            .execute(&mut document, "Polyline 0,4 1,4 2,4")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,5,0 1,5,0 2,6,0 3,5,1")
            .unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);

        assert_eq!(
            registry.execute(&mut document, "SelLine").unwrap(),
            "Selected 3 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([ids[0], ids[1], ids[3]])
        );

        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelPlanarCrv").unwrap(),
            "Selected 5 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([ids[0], ids[1], ids[2], ids[3], ids[4]])
        );
        assert!(matches!(
            registry.execute(&mut document, "SelPlanarCrv extra"),
            Err(CommandError::Usage("SelPlanarCrv"))
        ));
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn polyline_and_short_curve_selection_match_rhino_representation_and_boundary_rules() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 9,9").unwrap();
        registry.execute(&mut document, "Line 0,0 0.5,0").unwrap();
        registry.execute(&mut document, "Line 0,1 1,1").unwrap();
        registry.execute(&mut document, "Line 0,2 1.5,2").unwrap();
        registry
            .execute(&mut document, "Polyline 0,3 0.25,3 0.75,3")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 1 0,4 0.3,4 0.8,4")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 1 0,5 0.6,5")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,6 0.2,6 0.5,6 0.7,6")
            .unwrap();
        registry.execute(&mut document, "Line 0,7 0.4,7").unwrap();
        registry.execute(&mut document, "Line 0,8 0.4,8").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document.set_objects_visibility([ids[8]], false).unwrap();
        document.set_objects_locked([ids[9]], true).unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);

        assert_eq!(
            registry.execute(&mut document, "SelShortCrv 1").unwrap(),
            "Selected 7 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([ids[0], ids[1], ids[2], ids[4], ids[5], ids[6], ids[7]])
        );
        assert_eq!(document.undo_label(), history.as_deref());

        registry.execute(&mut document, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut document, "SelPolyline").unwrap(),
            "Selected 2 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([ids[4], ids[5]])
        );
        let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
        assert!(matches!(
            registry.execute(&mut document, "SelShortCrv"),
            Err(CommandError::Usage("SelShortCrv maximum-length"))
        ));
        assert!(matches!(
            registry.execute(&mut document, "SelShortCrv 1 extra"),
            Err(CommandError::Usage("SelShortCrv maximum-length"))
        ));
        assert!(matches!(
            registry.execute(&mut document, "SelShortCrv 0"),
            Err(CommandError::InvalidMaximumCurveLength(value)) if value == "0"
        ));
        assert!(matches!(
            registry.execute(&mut document, "SelShortCrv -1"),
            Err(CommandError::InvalidMaximumCurveLength(value)) if value == "-1"
        ));
        assert!(registry.execute(&mut document, "SelShortCrv NaN").is_err());
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            selection
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn move_and_copy_transform_the_selection_as_atomic_commands() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert!(registry.execute(&mut document, "Move 0,0 1,1").is_err());
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        let original = document.objects().next().unwrap().id();
        document
            .select_object(original, SelectionMode::Replace)
            .unwrap();

        registry
            .execute(&mut document, "Move 0,0,0 4,-2,1")
            .unwrap();
        assert_eq!(document.undo_label(), Some("Move"));
        assert!(matches!(
            document.object(original).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(5.0, 0.0, 4.0).unwrap()
        ));
        registry.execute(&mut document, "Undo").unwrap();
        assert!(matches!(
            document.object(original).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(1.0, 2.0, 3.0).unwrap()
        ));
        registry.execute(&mut document, "Redo").unwrap();

        registry.execute(&mut document, "Copy 5,0,4 8,1,4").unwrap();
        assert_eq!(document.undo_label(), Some("Copy"));
        assert_eq!(document.objects().len(), 2);
        assert!(!document.is_selected(original));
        let copy = document.selected_object_ids().next().unwrap();
        assert_ne!(copy, original);
        assert!(matches!(
            document.object(copy).unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(8.0, 1.0, 4.0).unwrap()
        ));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 1);
        assert!(document.object(original).is_some());
    }

    #[test]
    fn curve_array_items_match_rhino_selection_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let tolerance = document.tolerance();
        let first = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("First"),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(0.0, 2.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Second"),
            )
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        let path = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Rail"),
            )
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        let message = registry
            .execute(
                &mut document,
                "ArrayCrv Items=4 Orientation=_NoRotation PathName=rail",
            )
            .unwrap();
        assert!(message.contains("creating 6 copy object(s)"));
        assert_eq!(document.objects().len(), 9);
        assert_eq!(document.groups().len(), 4);
        assert_eq!(document.undo_label(), Some("ArrayCrv"));
        assert!(!document.is_selected(path));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );

        let mut first_points = document
            .objects()
            .filter(|object| object.attributes().name() == Some("First"))
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected curve-array points"),
            })
            .collect::<Vec<_>>();
        first_points.sort_by(|left, right| left.x().total_cmp(&right.x()));
        for (actual, expected_x) in
            first_points
                .into_iter()
                .zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0])
        {
            assert!(actual.is_near(Point3::try_new(expected_x, 0.0, 0.0).unwrap(), tolerance));
        }

        let copy_ids = document
            .objects()
            .map(|object| object.id())
            .filter(|id| ![first, second, path].contains(id))
            .collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.groups().len(), 1);
        registry.execute(&mut document, "Redo").unwrap();
        assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 4);
    }

    #[test]
    fn curve_array_distance_basepoint_and_closed_path_match_rhino() {
        let registry = CommandRegistry::with_builtins();

        let mut distance_document = Document::default();
        let layer = distance_document.current_layer_id();
        let tolerance = distance_document.tolerance();
        let source = distance_document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Source"),
            )
            .unwrap();
        distance_document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Rail"),
            )
            .unwrap();
        distance_document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut distance_document,
                "ArrayCrv Distance 3 Orientation NoRotation PathName Rail",
            )
            .unwrap();
        let mut distance_points = named_curve_array_points(&distance_document, "Source");
        distance_points.sort_by(|left, right| left.x().total_cmp(&right.x()));
        assert_eq!(
            distance_points,
            [0.0, 3.0, 6.0, 9.0].map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
        );

        let mut base_document = Document::default();
        let layer = base_document.current_layer_id();
        let tolerance = base_document.tolerance();
        let base_source = base_document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Source"),
            )
            .unwrap();
        base_document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Rail"),
            )
            .unwrap();
        base_document
            .select_object(base_source, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut base_document,
                "ArrayCurve 4 BasePoint=20,0,0 Orientation=NoRotation PathName=Rail",
            )
            .unwrap();
        let mut base_points = named_curve_array_points(&base_document, "Source");
        base_points.sort_by(|left, right| left.x().total_cmp(&right.x()));
        assert_eq!(base_points.len(), 5);
        for (actual, expected_x) in
            base_points
                .into_iter()
                .zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0, 20.0])
        {
            assert!(actual.is_near(Point3::try_new(expected_x, 0.0, 0.0).unwrap(), tolerance));
        }
        assert!(base_document.is_selected(base_source));

        let mut closed_document = Document::default();
        let layer = closed_document.current_layer_id();
        let tolerance = closed_document.tolerance();
        let closed_source = closed_document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(10.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Source"),
            )
            .unwrap();
        closed_document
            .add_geometry_with_attributes(
                Geometry::Circle(
                    Circle3::try_new(
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        10.0,
                        UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Rail"),
            )
            .unwrap();
        closed_document
            .select_object(closed_source, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut closed_document,
                "ArrayCrv 4 Orientation=NoRotation PathName=Rail",
            )
            .unwrap();
        let closed_points = named_curve_array_points(&closed_document, "Source");
        assert_eq!(closed_points.len(), 4);
        for expected in [
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 10.0, 0.0).unwrap(),
            Point3::try_new(-10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, -10.0, 0.0).unwrap(),
        ] {
            assert!(
                closed_points
                    .iter()
                    .any(|actual| actual.is_near(expected, tolerance))
            );
        }
    }

    fn named_curve_array_points(document: &Document, name: &str) -> Vec<Point3> {
        document
            .objects()
            .filter(|object| object.attributes().name() == Some(name))
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected named curve-array points"),
            })
            .collect()
    }

    #[test]
    fn curve_array_orientation_policies_match_planar_rhino_frames() {
        let registry = CommandRegistry::with_builtins();
        for (orientation, rotates) in [
            ("NoRotation", false),
            ("Freeform", true),
            ("Roadlike", true),
            ("Stairlike", true),
        ] {
            let mut document = Document::default();
            let layer = document.current_layer_id();
            let tolerance = document.tolerance();
            let source = document
                .add_geometry_with_attributes(
                    Geometry::Line(
                        LineSegment::try_new(
                            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                            tolerance,
                        )
                        .unwrap(),
                    ),
                    ObjectAttributes::on_layer(layer).with_name("Source"),
                )
                .unwrap();
            document
                .add_geometry_with_attributes(
                    Geometry::Arc(
                        CircularArc3::try_from_three_points(
                            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                            Point3::try_new(5.0 * 2.0_f64.sqrt(), 5.0 * 2.0_f64.sqrt(), 0.0)
                                .unwrap(),
                            Point3::try_new(0.0, 10.0, 0.0).unwrap(),
                            tolerance,
                        )
                        .unwrap(),
                    ),
                    ObjectAttributes::on_layer(layer).with_name("Rail"),
                )
                .unwrap();
            document
                .select_object(source, SelectionMode::Replace)
                .unwrap();
            registry
                .execute(
                    &mut document,
                    &format!("ArrayCrv 2 Orientation={orientation} PathName=Rail"),
                )
                .unwrap();
            let copy = document
                .objects()
                .filter(|object| object.attributes().name() == Some("Source"))
                .find(|object| object.id() != source)
                .unwrap();
            let Geometry::Line(line) = copy.geometry() else {
                panic!("expected an oriented line copy")
            };
            assert!(
                line.start()
                    .is_near(Point3::try_new(0.0, 10.0, 0.0).unwrap(), tolerance)
            );
            let expected_end = if rotates {
                Point3::try_new(0.0, 11.0, 0.0).unwrap()
            } else {
                Point3::try_new(1.0, 10.0, 0.0).unwrap()
            };
            assert!(
                line.end().is_near(expected_end, tolerance),
                "{orientation} ended at {:?}, expected {expected_end:?}",
                line.end()
            );
        }
    }

    #[test]
    fn curve_array_uses_last_selected_path_and_rejects_bad_inputs_atomically() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let tolerance = document.tolerance();
        let source = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Source"),
            )
            .unwrap();
        let path = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(4.0, 0.0, 0.0).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Rail"),
            )
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        document.select_object(path, SelectionMode::Add).unwrap();
        registry
            .execute(&mut document, "ArrayCrv 2 Orientation=NoRotation")
            .unwrap();
        assert_eq!(named_curve_array_points(&document, "Source").len(), 2);
        assert!(document.is_selected(source));
        assert!(!document.is_selected(path));

        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let object_count = document.objects().len();
        let history = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "ArrayCrv 0 PathName=Rail"),
            Err(CommandError::InvalidCurveArrayItemCount(value)) if value == "0"
        ));
        assert!(matches!(
            registry.execute(&mut document, "ArrayCrv Distance=0 PathName=Rail"),
            Err(CommandError::InvalidCurveArrayDistance(value)) if value == "0"
        ));
        assert!(matches!(
            registry.execute(&mut document, "ArrayCrv 2 PathName=Missing"),
            Err(CommandError::CurveArrayPathNotFound(name)) if name == "Missing"
        ));
        assert!(matches!(
            registry.execute(
                &mut document,
                "ArrayCrv 1000002 Orientation=NoRotation PathName=Rail"
            ),
            Err(CommandError::Geometry(
                GeometryError::TooManyCurveDivisionPoints { .. }
            ))
        ));
        assert_eq!(document.objects().len(), object_count);
        assert_eq!(document.undo_label(), history.as_deref());
        assert!(document.is_selected(source));
    }

    fn add_named_surface(document: &mut Document, surface: NurbsSurface, name: &str) -> ObjectId {
        document
            .add_geometry_with_attributes(
                Geometry::NurbsSurface(surface),
                ObjectAttributes::on_layer(document.current_layer_id()).with_name(name),
            )
            .unwrap()
    }

    fn sorted_points(mut points: Vec<Point3>) -> Vec<Point3> {
        points.sort_by(|left, right| {
            left.x()
                .total_cmp(&right.x())
                .then_with(|| left.y().total_cmp(&right.y()))
                .then_with(|| left.z().total_cmp(&right.z()))
        });
        points
    }

    #[test]
    fn surface_array_uv_matches_rhino_selection_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        let target = add_named_surface(
            &mut document,
            NurbsSurface::try_bilinear([
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(12.0, 10.0, 10.0).unwrap(),
                Point3::try_new(0.0, 10.0, 10.0).unwrap(),
            ])
            .unwrap(),
            "Target",
        );
        let message = registry
            .execute(
                &mut document,
                "ArraySrf 3 2 BasePoint=1,2,3 SurfaceName=target Mode=UV",
            )
            .unwrap();
        assert!(message.contains("creating 18 copy object(s)"));
        assert_eq!(document.objects().len(), 22);
        assert_eq!(document.groups().len(), 7);
        assert_eq!(document.undo_label(), Some("ArraySrf"));
        assert!(!document.is_selected(target));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );

        let actual_starts = sorted_points(
            document
                .objects()
                .filter(|object| {
                    object.attributes().name() == Some("x") && object.id() != originals[0]
                })
                .map(|object| match object.geometry() {
                    Geometry::Line(line) => line.start(),
                    _ => panic!("expected a surface-array line"),
                })
                .collect(),
        );
        let expected_starts = sorted_points(
            [
                [0.0, 0.0, 0.0],
                [5.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 10.0],
                [6.0, 10.0, 10.0],
                [12.0, 10.0, 10.0],
            ]
            .map(|point| Point3::try_from(point).unwrap())
            .to_vec(),
        );
        assert_eq!(actual_starts, expected_starts);

        let copy_ids = document
            .objects()
            .map(|object| object.id())
            .filter(|id| !originals.contains(id) && *id != target)
            .collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.groups().len(), 1);
        registry.execute(&mut document, "Redo").unwrap();
        assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 7);
    }

    #[test]
    fn surface_array_isocurve_evenly_divides_rational_surface_edges() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let source = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Stud"),
            )
            .unwrap();
        let middle_weight = 0.5_f64.sqrt();
        let mut controls = Vec::new();
        for z in [0.0, 10.0] {
            controls.extend([
                WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, z).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(10.0, 10.0, z).unwrap(), middle_weight)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(0.0, 10.0, z).unwrap(), 1.0).unwrap(),
            ]);
        }
        let target = add_named_surface(
            &mut document,
            NurbsSurface::try_new_rational(
                2,
                1,
                3,
                2,
                controls,
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap(),
            "Cylinder",
        );
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ArraySrf 4 1 BasePoint=1,2,3 SurfaceName=Cylinder Mode=Isocurve",
            )
            .unwrap();
        assert!(!document.is_selected(target));
        let mut copies = document
            .objects()
            .filter(|object| object.attributes().name() == Some("Stud") && object.id() != source)
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected a surface-array point"),
            })
            .collect::<Vec<_>>();
        copies.sort_by(|left, right| left.y().total_cmp(&right.y()));
        for (actual, angle) in copies.into_iter().zip([
            0.0,
            std::f64::consts::FRAC_PI_6,
            std::f64::consts::FRAC_PI_3,
            std::f64::consts::FRAC_PI_2,
        ]) {
            assert!(actual.is_near(
                Point3::try_new(10.0 * angle.cos(), 10.0 * angle.sin(), 0.0).unwrap(),
                document.tolerance()
            ));
        }
    }

    #[test]
    fn surface_array_custom_up_last_target_and_errors_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        let target = add_named_surface(
            &mut document,
            NurbsSurface::try_bilinear([
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(12.0, 10.0, 10.0).unwrap(),
                Point3::try_new(0.0, 10.0, 10.0).unwrap(),
            ])
            .unwrap(),
            "Target",
        );
        registry
            .execute(
                &mut document,
                "ArraySrf 1 1 BasePoint=1,2,3 Up=0,1,0 SurfaceName=Target",
            )
            .unwrap();
        let root_half = 0.5_f64.sqrt();
        for (name, original, expected_end) in [
            (
                "x",
                originals[0],
                Point3::try_new(0.0, root_half, root_half).unwrap(),
            ),
            (
                "y",
                originals[1],
                Point3::try_new(0.0, -root_half, root_half).unwrap(),
            ),
            ("z", originals[2], Point3::try_new(1.0, 0.0, 0.0).unwrap()),
        ] {
            let copy = orient_line(&document, name, &[original]);
            assert_eq!(copy.start(), Point3::try_new(0.0, 0.0, 0.0).unwrap());
            assert!(copy.end().is_near(expected_end, document.tolerance()));
        }
        registry.execute(&mut document, "Undo").unwrap();

        document.select_object(target, SelectionMode::Add).unwrap();
        registry
            .execute(&mut document, "ArraySrf 1 1 BasePoint=1,2,3")
            .unwrap();
        assert!(!document.is_selected(target));
        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_object(originals[0], SelectionMode::Replace)
            .unwrap();

        let object_count = document.objects().len();
        let group_count = document.groups().len();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "ArraySrf 0 1 BasePoint=1,2,3 SurfaceName=Target",
            "ArraySrf 1 1 SurfaceName=Target",
            "ArraySrf 1 1 BasePoint=1,2,3 Up=0,0,0 SurfaceName=Target",
            "ArraySrf 1 1 BasePoint=1,2,3 Mode=Bad SurfaceName=Target",
            "ArraySrf 1 1 BasePoint=1,2,3 SurfaceName=Missing",
            "ArraySrf 1 1 BasePoint=1,2,3 SurfaceName=x",
            "ArraySrf 1000001 1 BasePoint=1,2,3 SurfaceName=Target",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), object_count);
        assert_eq!(document.groups().len(), group_count);
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );
    }

    #[test]
    fn rectangular_array_matches_rhino_unit_cell_selection_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let first = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("First"),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(4.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Second"),
            )
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        let message = registry
            .execute(&mut document, "Array 3 2 2 2 -1 4 Mode=_UnitCell")
            .unwrap();
        assert!(message.contains("creating 22 copy object(s)"));
        assert_eq!(document.objects().len(), 24);
        assert_eq!(document.groups().len(), 12);
        assert_eq!(document.undo_label(), Some("Array"));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );

        let mut expected_named_points = Vec::new();
        let mut expected_groups = Vec::new();
        for z_index in 0..2 {
            for y_index in 0..2 {
                for x_index in 0..3 {
                    let translation = [
                        2.0 * f64::from(x_index),
                        -f64::from(y_index),
                        4.0 * f64::from(z_index),
                    ];
                    let group = vec![
                        [
                            1.0 + translation[0],
                            2.0 + translation[1],
                            3.0 + translation[2],
                        ],
                        [
                            4.0 + translation[0],
                            2.0 + translation[1],
                            3.0 + translation[2],
                        ],
                    ];
                    expected_named_points.push((group[0], "First".to_owned()));
                    expected_named_points.push((group[1], "Second".to_owned()));
                    expected_groups.push(group);
                }
            }
        }
        expected_named_points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        expected_groups.sort_by(|left, right| left.partial_cmp(right).unwrap());

        let mut actual_named_points = document
            .objects()
            .map(|object| {
                let Geometry::Point(point) = object.geometry() else {
                    panic!("expected rectangular-array points")
                };
                (
                    point.to_array(),
                    object.attributes().name().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        actual_named_points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        assert_eq!(actual_named_points, expected_named_points);

        let mut actual_groups = document
            .groups()
            .map(|group| {
                let mut points = group
                    .members()
                    .map(|id| match document.object(id).unwrap().geometry() {
                        Geometry::Point(point) => point.to_array(),
                        _ => panic!("expected grouped rectangular-array points"),
                    })
                    .collect::<Vec<_>>();
                points.sort_by(|left, right| left.partial_cmp(right).unwrap());
                points
            })
            .collect::<Vec<_>>();
        actual_groups.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(actual_groups, expected_groups);

        let copy_ids = document
            .objects()
            .map(|object| object.id())
            .filter(|id| ![first, second].contains(id))
            .collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 12);
    }

    #[test]
    fn rectangular_array_fill_uses_the_selected_extents() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let first = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("First"),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(4.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Second"),
            )
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        registry
            .execute(&mut document, "Array 3 2 1 10 -6 0 Mode Fill")
            .unwrap();
        assert_eq!(document.objects().len(), 12);
        assert_eq!(document.groups().len(), 6);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );

        let mut actual = document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => panic!("expected rectangular-array points"),
            })
            .collect::<Vec<_>>();
        actual.sort_by(|left, right| left.partial_cmp(right).unwrap());
        let mut expected = Vec::new();
        for y in [0.0, -6.0] {
            for x in [0.0, 3.5, 7.0] {
                expected.push([1.0 + x, 2.0 + y, 3.0]);
                expected.push([4.0 + x, 2.0 + y, 3.0]);
            }
        }
        expected.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(actual, expected);
    }

    #[test]
    fn rectangular_array_rejects_invalid_or_unbounded_output_without_partial_copies() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        document.select_all();
        let history = document.undo_label().map(str::to_owned);

        assert!(matches!(
            registry.execute(&mut document, "Array 0 2 1 1 1 0"),
            Err(CommandError::InvalidArrayDimensionCount(value)) if value == "0"
        ));
        assert!(matches!(
            registry.execute(&mut document, "Array 1000001 2 1 1 1 0"),
            Err(CommandError::TooManyArrayObjects { maximum })
                if maximum == MAX_ARRAY_OBJECTS
        ));
        assert!(matches!(
            registry.execute(&mut document, "Array 2 1 1 1 0 0 Mode=Maybe"),
            Err(CommandError::Usage(ARRAY_USAGE))
        ));
        assert!(matches!(
            registry.execute(&mut document, "Array 2 1 1 0.5 0 0 Mode=Fill"),
            Err(CommandError::ArrayFillLengthTooSmall { axis: "X", minimum })
                if minimum == 1.0
        ));
        assert!(
            registry
                .execute(&mut document, "Array 3 1 1 1e308 0 0")
                .is_err()
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn linear_array_matches_rhino_spacing_selection_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let first = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("First"),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(4.0, 2.0, 3.0).unwrap()),
                ObjectAttributes::on_layer(layer).with_name("Second"),
            )
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        let message = registry
            .execute(&mut document, "ArrayLinear 4 0,0,0 2,-1,3")
            .unwrap();
        assert!(message.contains("creating 6 copy object(s)"));
        assert_eq!(document.objects().len(), 8);
        assert_eq!(document.groups().len(), 4);
        assert_eq!(document.undo_label(), Some("ArrayLinear"));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );

        let mut named_points = document
            .objects()
            .map(|object| {
                let Geometry::Point(point) = object.geometry() else {
                    panic!("expected arrayed points")
                };
                (
                    point.to_array(),
                    object.attributes().name().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        named_points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        assert_eq!(
            named_points,
            vec![
                ([1.0, 2.0, 3.0], "First".to_owned()),
                ([3.0, 1.0, 6.0], "First".to_owned()),
                ([4.0, 2.0, 3.0], "Second".to_owned()),
                ([5.0, 0.0, 9.0], "First".to_owned()),
                ([6.0, 1.0, 6.0], "Second".to_owned()),
                ([7.0, -1.0, 12.0], "First".to_owned()),
                ([8.0, 0.0, 9.0], "Second".to_owned()),
                ([10.0, -1.0, 12.0], "Second".to_owned()),
            ]
        );
        let mut group_points = document
            .groups()
            .map(|group| {
                let mut points = group
                    .members()
                    .map(|id| match document.object(id).unwrap().geometry() {
                        Geometry::Point(point) => point.to_array(),
                        _ => panic!("expected grouped array points"),
                    })
                    .collect::<Vec<_>>();
                points.sort_by(|left, right| left.partial_cmp(right).unwrap());
                points
            })
            .collect::<Vec<_>>();
        group_points.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(
            group_points,
            vec![
                vec![[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]],
                vec![[3.0, 1.0, 6.0], [6.0, 1.0, 6.0]],
                vec![[5.0, 0.0, 9.0], [8.0, 0.0, 9.0]],
                vec![[7.0, -1.0, 12.0], [10.0, -1.0, 12.0]],
            ]
        );

        let array_ids = document
            .objects()
            .map(|object| object.id())
            .filter(|id| ![first, second].contains(id))
            .collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
        assert!(document.is_selected(first));
        assert!(document.is_selected(second));
        registry.execute(&mut document, "Redo").unwrap();
        assert!(array_ids.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 4);
    }

    #[test]
    fn linear_array_rejects_invalid_or_unbounded_output_without_partial_copies() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        document.select_all();
        let history = document.undo_label().map(str::to_owned);

        assert!(matches!(
            registry.execute(&mut document, "ArrayLinear 1 0,0,0 1,0,0"),
            Err(CommandError::InvalidArrayItemCount(value)) if value == "1"
        ));
        assert!(matches!(
            registry.execute(&mut document, "ArrayLinear 500002 0,0,0 1,0,0"),
            Err(CommandError::TooManyArrayObjects { maximum })
                if maximum == MAX_ARRAY_OBJECTS
        ));
        assert!(
            registry
                .execute(&mut document, "ArrayLinear 3 0,0,0 1e308,0,0")
                .is_err()
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn polar_array_matches_rhino_full_sweep_selection_groups_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let first = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(4.0, 1.0, 0.0).unwrap(),
                        document.tolerance(),
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("First"),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(1.0, -1.0, 2.0).unwrap(),
                        Point3::try_new(2.0, -0.5, 3.0).unwrap(),
                        document.tolerance(),
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Second"),
            )
            .unwrap();
        document
            .add_group(Some("Pair".to_owned()), [first, second])
            .unwrap();
        document
            .select_object(first, SelectionMode::Replace)
            .unwrap();

        let message = registry
            .execute(&mut document, "ArrayPolar 4 0,0,0 360")
            .unwrap();
        assert!(message.contains("creating 6 copy object(s)"));
        assert_eq!(document.objects().len(), 8);
        assert_eq!(document.groups().len(), 4);
        assert_eq!(document.undo_label(), Some("ArrayPolar"));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([first, second])
        );
        assert_eq!(
            document
                .objects()
                .filter(|object| object.attributes().name() == Some("First"))
                .count(),
            4
        );

        let mut first_lines = document
            .objects()
            .filter(|object| object.attributes().name() == Some("First"))
            .map(|object| match object.geometry() {
                Geometry::Line(line) => (line.start(), line.end()),
                _ => panic!("expected arrayed lines"),
            })
            .collect::<Vec<_>>();
        first_lines
            .sort_by(|left, right| left.0.to_array().partial_cmp(&right.0.to_array()).unwrap());
        let expected = [
            (
                Point3::try_new(-2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(-4.0, -1.0, 0.0).unwrap(),
            ),
            (
                Point3::try_new(0.0, -2.0, 0.0).unwrap(),
                Point3::try_new(1.0, -4.0, 0.0).unwrap(),
            ),
            (
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(-1.0, 4.0, 0.0).unwrap(),
            ),
            (
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(4.0, 1.0, 0.0).unwrap(),
            ),
        ];
        for ((start, end), (expected_start, expected_end)) in first_lines.into_iter().zip(expected)
        {
            assert!(start.is_near(expected_start, document.tolerance()));
            assert!(end.is_near(expected_end, document.tolerance()));
        }

        let copy_ids = document
            .objects()
            .map(|object| object.id())
            .filter(|id| ![first, second].contains(id))
            .collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
        assert!(document.is_selected(first));
        assert!(document.is_selected(second));
        registry.execute(&mut document, "Redo").unwrap();
        assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 4);
    }

    #[test]
    fn polar_array_no_rotate_uses_the_combined_selection_bounds_center() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let sources = [
            ("First", [2.0, 0.0, 0.0], [4.0, 1.0, 0.0]),
            ("Second", [1.0, -1.0, 2.0], [2.0, -0.5, 3.0]),
        ];
        let ids = sources
            .iter()
            .map(|(name, start, end)| {
                document
                    .add_geometry_with_attributes(
                        Geometry::Line(
                            LineSegment::try_new(
                                Point3::try_from(*start).unwrap(),
                                Point3::try_from(*end).unwrap(),
                                document.tolerance(),
                            )
                            .unwrap(),
                        ),
                        ObjectAttributes::on_layer(layer).with_name(*name),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document
            .add_group(Some("Pair".to_owned()), ids.iter().copied())
            .unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();

        registry
            .execute(&mut document, "ArrayPolar 4 0,0,0 180 Rotate=_No")
            .unwrap();
        let mut first_lines = document
            .objects()
            .filter(|object| object.attributes().name() == Some("First"))
            .map(|object| match object.geometry() {
                Geometry::Line(line) => (line.start(), line.end()),
                _ => panic!("expected arrayed lines"),
            })
            .collect::<Vec<_>>();
        first_lines
            .sort_by(|left, right| left.0.to_array().partial_cmp(&right.0.to_array()).unwrap());
        let root_three_over_two = 3.0_f64.sqrt() * 1.25;
        let expected_starts = [
            Point3::try_new(-3.0, 0.0, 0.0).unwrap(),
            Point3::try_new(-1.75, root_three_over_two, 0.0).unwrap(),
            Point3::try_new(0.75, root_three_over_two, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        ];
        let source_direction = Vector3::try_new(2.0, 1.0, 0.0).unwrap();
        for ((start, end), expected_start) in first_lines.into_iter().zip(expected_starts) {
            assert!(start.is_near(expected_start, document.tolerance()));
            let direction = start.vector_to(end).unwrap();
            assert!(
                direction
                    .to_array()
                    .into_iter()
                    .zip(source_direction.to_array())
                    .all(|(actual, expected)| document.tolerance().approx_eq(actual, expected))
            );
        }
        assert_eq!(document.groups().len(), 4);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            ids.into_iter().collect()
        );
    }

    #[test]
    fn polar_array_supports_multi_turn_sweeps_and_cumulative_z_offset() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 2,0,0").unwrap();
        let original = document.objects().next().unwrap().id();
        document
            .select_object(original, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "ArrayPolar 4 0,0,0 720 ZOffset 2")
            .unwrap();

        let mut points = document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected arrayed points"),
            })
            .collect::<Vec<_>>();
        points.sort_by(|left, right| left.z().partial_cmp(&right.z()).unwrap());
        let root_three = 3.0_f64.sqrt();
        let expected = [
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(-1.0, -root_three, 2.0).unwrap(),
            Point3::try_new(-1.0, root_three, 4.0).unwrap(),
            Point3::try_new(2.0, 0.0, 6.0).unwrap(),
        ];
        for (actual, expected) in points.into_iter().zip(expected) {
            assert!(actual.is_near(expected, document.tolerance()));
        }
    }

    #[test]
    fn polar_array_rejects_invalid_or_unbounded_output_without_partial_copies() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 0,0,0").unwrap();
        registry.execute(&mut document, "Point 1,0,0").unwrap();
        document.select_all();
        let history = document.undo_label().map(str::to_owned);

        assert!(matches!(
            registry.execute(&mut document, "ArrayPolar 1 0,0,0 360"),
            Err(CommandError::InvalidArrayItemCount(value)) if value == "1"
        ));
        assert!(matches!(
            registry.execute(&mut document, "ArrayPolar 3 0,0,0 0"),
            Err(CommandError::InvalidPolarArrayAngle(value)) if value == "0"
        ));
        assert!(matches!(
            registry.execute(&mut document, "ArrayPolar 500002 0,0,0 360"),
            Err(CommandError::TooManyArrayObjects { maximum })
                if maximum == MAX_ARRAY_OBJECTS
        ));
        assert!(
            registry
                .execute(&mut document, "ArrayPolar 3 0,0,0 180 ZOffset=1e308",)
                .is_err()
        );
        assert!(matches!(
            registry.execute(&mut document, "ArrayPolar 3 0,0,0 180 Rotate=No Rotate=Yes"),
            Err(CommandError::Usage(ARRAY_POLAR_USAGE))
        ));
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.selected_object_count(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    fn add_orient_triad(document: &mut Document) -> [ObjectId; 3] {
        let layer = document.current_layer_id();
        let tolerance = document.tolerance();
        let origin = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        let mut ids = Vec::new();
        for (name, end) in [
            ("x", Point3::try_new(2.0, 2.0, 3.0).unwrap()),
            ("y", Point3::try_new(1.0, 3.0, 3.0).unwrap()),
            ("z", Point3::try_new(1.0, 2.0, 4.0).unwrap()),
        ] {
            ids.push(
                document
                    .add_geometry_with_attributes(
                        Geometry::Line(LineSegment::try_new(origin, end, tolerance).unwrap()),
                        ObjectAttributes::on_layer(layer).with_name(name),
                    )
                    .unwrap(),
            );
        }
        let ids: [ObjectId; 3] = ids.try_into().unwrap();
        document.add_group(Some("Triad".to_owned()), ids).unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        ids
    }

    fn orient_line(document: &Document, name: &str, excluded: &[ObjectId]) -> LineSegment {
        document
            .objects()
            .find(|object| {
                object.attributes().name() == Some(name) && !excluded.contains(&object.id())
            })
            .map(|object| match object.geometry() {
                Geometry::Line(line) => *line,
                _ => panic!("expected an orient fixture line"),
            })
            .unwrap()
    }

    #[test]
    fn orient_matches_rhino_no_1d_and_3d_copy_scaling() {
        let registry = CommandRegistry::with_builtins();
        for (mode, expected) in [
            (
                "No",
                [
                    Point3::try_new(10.0, 0.0, 4.0).unwrap(),
                    Point3::try_new(9.0, -1.0, 4.0).unwrap(),
                    Point3::try_new(10.0, -1.0, 5.0).unwrap(),
                ],
            ),
            (
                "1D",
                [
                    Point3::try_new(10.0, 2.0, 4.0).unwrap(),
                    Point3::try_new(9.0, -1.0, 4.0).unwrap(),
                    Point3::try_new(10.0, -1.0, 5.0).unwrap(),
                ],
            ),
            (
                "3D",
                [
                    Point3::try_new(10.0, 2.0, 4.0).unwrap(),
                    Point3::try_new(7.0, -1.0, 4.0).unwrap(),
                    Point3::try_new(10.0, -1.0, 7.0).unwrap(),
                ],
            ),
        ] {
            let mut document = Document::default();
            let originals = add_orient_triad(&mut document);
            let message = registry
                .execute(
                    &mut document,
                    &format!("Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale={mode}"),
                )
                .unwrap();
            assert!(message.contains("creating 3 copy object(s)"));
            assert_eq!(document.objects().len(), 6);
            assert_eq!(document.groups().len(), 2);
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                originals.into_iter().collect()
            );
            for ((name, expected_end), original) in
                ["x", "y", "z"].into_iter().zip(expected).zip(originals)
            {
                let copy = orient_line(&document, name, &[original]);
                assert!(copy.start().is_near(
                    Point3::try_new(10.0, -1.0, 4.0).unwrap(),
                    document.tolerance()
                ));
                assert!(copy.end().is_near(expected_end, document.tolerance()));
            }
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().len(), 3);
            assert_eq!(document.groups().len(), 1);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().len(), 6);
            assert_eq!(document.groups().len(), 2);
        }
    }

    #[test]
    fn orient_defaults_to_unscaled_in_place_shortest_rotation() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        registry
            .execute(&mut document, "Orient 1,2,3 3,2,3 10,-1,4 10,5,4")
            .unwrap();
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );
        for (name, expected_end) in [
            ("x", Point3::try_new(10.0, 0.0, 4.0).unwrap()),
            ("y", Point3::try_new(9.0, -1.0, 4.0).unwrap()),
            ("z", Point3::try_new(10.0, -1.0, 5.0).unwrap()),
        ] {
            let line = orient_line(&document, name, &[]);
            assert!(line.end().is_near(expected_end, document.tolerance()));
        }
        assert_eq!(document.undo_label(), Some("Orient"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            orient_line(&document, "x", &[]),
            LineSegment::try_new(
                Point3::try_new(1.0, 2.0, 3.0).unwrap(),
                Point3::try_new(2.0, 2.0, 3.0).unwrap(),
                document.tolerance(),
            )
            .unwrap()
        );
    }

    #[test]
    fn orient_three_point_matches_rhino_frames_scale_and_atomic_errors() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        registry
            .execute(
                &mut document,
                "Orient3Pt 1,2,3 3,2,3 1,3,4 10,-1,4 10,5,4 8,-1,8 Copy=Yes Scale=Yes",
            )
            .unwrap();
        let expected = [
            ("x", Point3::try_new(10.0, 2.0, 4.0).unwrap()),
            (
                "y",
                Point3::try_new(7.153950105848459, -1.0, 4.948683298050513).unwrap(),
            ),
            (
                "z",
                Point3::try_new(10.948683298050513, -1.0, 6.846049894151541).unwrap(),
            ),
        ];
        for ((name, expected_end), original) in expected.into_iter().zip(originals) {
            let copy = orient_line(&document, name, &[original]);
            assert!(copy.end().is_near(expected_end, document.tolerance()));
        }
        assert_eq!(document.groups().len(), 2);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );

        registry.execute(&mut document, "Undo").unwrap();
        let object_count = document.objects().len();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "Orient 0,0,0 0,0,0 1,1,1 2,1,1",
            "Orient 0,0,0 1,0,0 1,1,1 1,1,1",
            "Orient3Pt 0,0,0 1,0,0 2,0,0 0,0,0 0,1,0 0,0,1",
            "Orient3Pt 0,0,0 1,0,0 0,1,0 0,0,0 0,1,0 0,2,0",
            "Orient 0,0,0 1,0,0 0,0,0 0,1,0 Copy=Yes Copy=No",
            "Orient3Pt 0,0,0 1,0,0 0,1,0 0,0,0 0,1,0 0,0,1 Scale=1D",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), object_count);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    fn orient_surface_quarter_cylinder() -> NurbsSurface {
        let middle_weight = 0.5_f64.sqrt();
        let mut controls = Vec::new();
        for z in [0.0, 10.0] {
            controls.extend([
                WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, z).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(10.0, 10.0, z).unwrap(), middle_weight)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(0.0, 10.0, z).unwrap(), 1.0).unwrap(),
            ]);
        }
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap()
    }

    fn oriented_surface_curve(document: &Document, name: &str, original: ObjectId) -> NurbsCurve {
        document
            .objects()
            .find(|object| object.id() != original && object.attributes().name() == Some(name))
            .map(|object| match object.geometry() {
                Geometry::NurbsCurve(curve) => curve.clone(),
                _ => panic!("expected a deformable surface-orient curve"),
            })
            .unwrap()
    }

    #[test]
    fn orient_on_surface_deformable_matches_rhino_splop_groups_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        let target = add_named_surface(&mut document, orient_surface_quarter_cylinder(), "Target");
        let message = registry
            .execute(
                &mut document,
                "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Copy=Yes Rigid=No SurfaceName=target",
            )
            .unwrap();
        assert!(message.contains("deformable"));
        assert!(message.contains("creating 3 copy object(s)"));
        assert_eq!(document.objects().len(), 7);
        assert_eq!(document.groups().len(), 2);
        assert_eq!(document.undo_label(), Some("OrientOnSrf"));
        assert!(!document.is_selected(target));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );
        let expected_ends = [
            Point3::try_new(8.484425274005353, 5.292875189329445, 4.0).unwrap(),
            Point3::try_new(8.973756499953724, 4.412674277525845, 5.0).unwrap(),
            Point3::try_new(9.8711321499491, 4.85394170527843, 4.0).unwrap(),
        ];
        let base = Point3::try_new(8.973756499953726, 4.412674277525846, 4.0).unwrap();
        let copy_ids = document
            .objects()
            .filter(|object| !originals.contains(&object.id()) && object.id() != target)
            .map(|object| object.id())
            .collect::<Vec<_>>();
        for (((name, original), expected_end), copy_id) in ["x", "y", "z"]
            .into_iter()
            .zip(originals)
            .zip(expected_ends)
            .zip(copy_ids.iter().copied())
        {
            let curve = oriented_surface_curve(&document, name, original);
            assert_eq!(curve.degree(), 3);
            assert_eq!(curve.control_points().len(), 4);
            assert!(
                curve
                    .evaluate(*curve.domain().start())
                    .unwrap()
                    .is_near(base, document.tolerance())
            );
            assert!(
                curve
                    .evaluate(*curve.domain().end())
                    .unwrap()
                    .is_near(expected_end, document.tolerance())
            );
            assert!(!document.is_selected(copy_id));
        }

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.groups().len(), 1);
        registry.execute(&mut document, "Redo").unwrap();
        assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
        assert_eq!(document.groups().len(), 2);
    }

    #[test]
    fn orient_on_surface_rigid_flip_constrained_normal_and_errors_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        let target = add_named_surface(&mut document, orient_surface_quarter_cylinder(), "Target");
        registry
            .execute(
                &mut document,
                "OrientOnSurface 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Scale=2 Rotation=90 Copy=Yes Rigid=Yes SurfaceName=Target",
            )
            .unwrap();
        let base = Point3::try_new(8.973756499953726, 4.412674277525846, 4.0).unwrap();
        let expected = [
            Point3::try_new(base.x(), base.y(), 6.0).unwrap(),
            Point3::try_new(9.856291355458895, 2.6179229775351006, 4.0).unwrap(),
            Point3::try_new(10.76850779994447, 5.295209133030891, 4.0).unwrap(),
        ];
        for ((name, expected_end), original) in
            ["x", "y", "z"].into_iter().zip(expected).zip(originals)
        {
            let copy = orient_line(&document, name, &[original]);
            assert!(copy.start().is_near(base, document.tolerance()));
            assert!(copy.end().is_near(expected_end, document.tolerance()));
        }
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(
                &mut document,
                "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Copy=No Rigid=No ConstrainNormal=Yes SurfaceName=Target",
            )
            .unwrap();
        let z = match document.object(originals[2]).unwrap().geometry() {
            Geometry::NurbsCurve(curve) => curve,
            _ => panic!("expected an in-place surface morph"),
        };
        assert!(z.evaluate(*z.domain().end()).unwrap().is_near(
            Point3::try_new(base.x(), base.y(), 5.0).unwrap(),
            document.tolerance()
        ));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(
                &mut document,
                "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 \
                 Copy=No Rigid=No Flip=Yes SurfaceName=Target",
            )
            .unwrap();
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.groups().len(), 1);
        let y = match document.object(originals[1]).unwrap().geometry() {
            Geometry::NurbsCurve(curve) => curve,
            _ => panic!("expected an in-place surface morph"),
        };
        assert!(y.evaluate(*y.domain().end()).unwrap().is_near(
            Point3::try_new(base.x(), base.y(), 3.0).unwrap(),
            document.tolerance()
        ));
        registry.execute(&mut document, "Undo").unwrap();

        let object_count = document.objects().len();
        let group_count = document.groups().len();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "OrientOnSrf 1,2,3 1,2,3 8.973756499953726,4.412674277525846,4 SurfaceName=Target",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Scale=0 SurfaceName=Target",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Scale=-1 SurfaceName=Target",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 SourceNormal=1,0,0 SurfaceName=Target",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 ConstrainNormal=0,0,1 SurfaceName=Target",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Rigid=Yes ConstrainNormal=Yes SurfaceName=Target",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 SurfaceName=Missing",
            "OrientOnSrf 1,2,3 2,2,3 8.973756499953726,4.412674277525846,4 Copy=Yes Copy=No SurfaceName=Target",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), object_count);
        assert_eq!(document.groups().len(), group_count);
        assert_eq!(document.undo_label(), history.as_deref());
        assert!(!document.is_selected(target));
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );
    }

    #[test]
    fn scale_rotate_and_mirror_support_numeric_and_reference_forms() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 2,1,0").unwrap();
        let object = document.objects().next().unwrap().id();
        document
            .select_object(object, SelectionMode::Replace)
            .unwrap();
        let position = |document: &Document| match document.object(object).unwrap().geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected a point"),
        };

        registry.execute(&mut document, "Scale 1,1 -1").unwrap();
        assert_eq!(position(&document), Point3::try_new(0.0, 1.0, 0.0).unwrap());
        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "Scale 1,1 2").unwrap();
        assert_eq!(position(&document), Point3::try_new(3.0, 1.0, 0.0).unwrap());
        assert_eq!(document.undo_label(), Some("Scale"));
        registry.execute(&mut document, "Undo").unwrap();
        registry
            .execute(&mut document, "Scale 1,1 2,1 3,1")
            .unwrap();
        assert_eq!(position(&document), Point3::try_new(3.0, 1.0, 0.0).unwrap());
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "Rotate 1,1 90").unwrap();
        assert!(position(&document).is_near(
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            document.tolerance()
        ));
        assert_eq!(document.undo_label(), Some("Rotate"));
        registry.execute(&mut document, "Undo").unwrap();
        registry
            .execute(&mut document, "Rotate 1,1 2,1 1,2")
            .unwrap();
        assert!(position(&document).is_near(
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            document.tolerance()
        ));
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "Mirror 0,0 0,1").unwrap();
        assert_eq!(
            position(&document),
            Point3::try_new(-2.0, 1.0, 0.0).unwrap()
        );
        assert_eq!(document.undo_label(), Some("Mirror"));
        registry.execute(&mut document, "Undo").unwrap();

        let history_label = document.undo_label().map(str::to_owned);
        assert!(registry.execute(&mut document, "Scale 1,1 0").is_err());
        assert!(registry.execute(&mut document, "Mirror 0,0 0,0").is_err());
        assert_eq!(position(&document), Point3::try_new(2.0, 1.0, 0.0).unwrap());
        assert_eq!(document.undo_label(), history_label.as_deref());
    }

    #[test]
    fn nonuniform_scale_commands_support_axes_references_flattening_and_copy() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 2,3,4").unwrap();
        let original = document.objects().next().unwrap().id();
        document
            .select_object(original, SelectionMode::Replace)
            .unwrap();
        let position = |document: &Document, id: ObjectId| {
            let Geometry::Point(point) = document.object(id).unwrap().geometry() else {
                panic!("expected a point")
            };
            *point
        };

        registry
            .execute(&mut document, "Scale1D 0,0,0 2 1,1,0")
            .unwrap();
        assert!(position(&document, original).is_near(
            Point3::try_new(4.5, 5.5, 4.0).unwrap(),
            document.tolerance()
        ));
        assert_eq!(document.undo_label(), Some("Scale1D"));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Scale1D 0,0,0 1,0,0 3,0,0")
            .unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(6.0, 3.0, 4.0).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Scale1D 0,0,0 0 1,0,0")
            .unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(0.0, 3.0, 4.0).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "Scale2D 1,1,1 2").unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(3.0, 5.0, 4.0).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Scale2D 0,0,0 2,0,10 4,0,-20")
            .unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(4.0, 6.0, 4.0).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "ScaleNU 1,1,1 2 -1 .5 Copy=Yes")
            .unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(2.0, 3.0, 4.0).unwrap()
        );
        let copy = document.objects().nth(1).unwrap().id();
        assert_eq!(
            position(&document, copy),
            Point3::try_new(3.0, -1.0, 2.5).unwrap()
        );
        assert!(document.is_selected(original));
        assert!(!document.is_selected(copy));
        assert_eq!(document.undo_label(), Some("ScaleNU"));

        registry.execute(&mut document, "Undo").unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "Scale1D 0,0,0 2 0,0,0",
            "Scale2D 0,0,0 0",
            "ScaleNU 0,0,0 1 0 1",
            "ScaleNU 0,0,0 1 2 3 Copy=Yes Copy=No",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 1);
        assert_eq!(
            position(&document, original),
            Point3::try_new(2.0, 3.0, 4.0).unwrap()
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn shear_matches_rhino_top_view_basis_angles_copy_and_atomic_errors() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 2,3,4").unwrap();
        let original = document.objects().next().unwrap().id();
        document
            .select_object(original, SelectionMode::Replace)
            .unwrap();
        let position = |document: &Document, id: ObjectId| {
            let Geometry::Point(point) = document.object(id).unwrap().geometry() else {
                panic!("expected a point")
            };
            *point
        };

        registry
            .execute(&mut document, "Shear 0,0,0 1,0,0 45")
            .unwrap();
        assert!(position(&document, original).is_near(
            Point3::try_new(2.0, 5.0, 4.0).unwrap(),
            document.tolerance()
        ));
        assert_eq!(document.undo_label(), Some("Shear"));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Shear 0,0,0 2,0,0 -30")
            .unwrap();
        assert!(position(&document, original).is_near(
            Point3::try_new(2.0, 1.845_299_461_620_748_5, 4.0).unwrap(),
            document.tolerance()
        ));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Shear 1,2,3 1,5,3 30")
            .unwrap();
        assert!(position(&document, original).is_near(
            Point3::try_new(1.422_649_730_810_374_3, 3.0, 4.0).unwrap(),
            document.tolerance()
        ));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Shear 0,0,0 2,0,0 2,2,0 Copy=Yes")
            .unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(2.0, 3.0, 4.0).unwrap()
        );
        let copy = document.objects().nth(1).unwrap().id();
        assert!(position(&document, copy).is_near(
            Point3::try_new(2.0, 5.0, 4.0).unwrap(),
            document.tolerance()
        ));
        assert!(document.is_selected(original));
        assert!(!document.is_selected(copy));
        registry.execute(&mut document, "Undo").unwrap();

        let history = document.undo_label().map(str::to_owned);
        for command in [
            "Shear 0,0,0 0,0,5 45",
            "Shear 0,0,0 1,0,0 0,0,0",
            "Shear 0,0,0 1,0,0 45 Copy=Yes Copy=No",
            "Shear 0,0,0 1,0,0 45 extra",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 1);
        assert_eq!(
            position(&document, original),
            Point3::try_new(2.0, 3.0, 4.0).unwrap()
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn project_to_cplane_matches_rhino_delete_input_groups_and_selection() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 1,2,3 4,5,6").unwrap();
        registry.execute(&mut document, "Point -2,7,-4").unwrap();
        let source_ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .add_group(
                Some("Projection pair".to_owned()),
                source_ids.iter().copied(),
            )
            .unwrap();
        document
            .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
            .unwrap();

        registry.execute(&mut document, "ProjectToCPlane").unwrap();
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.groups().len(), 2);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        let copies = document
            .objects()
            .filter(|object| !source_ids.contains(&object.id()))
            .collect::<Vec<_>>();
        assert_eq!(copies.len(), 2);
        assert!(
            copies
                .iter()
                .all(|object| !document.is_selected(object.id()))
        );
        let Geometry::Line(projected_line) = copies[0].geometry() else {
            panic!("expected a projected line")
        };
        assert_eq!(
            projected_line.start(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap()
        );
        assert_eq!(
            projected_line.end(),
            Point3::try_new(4.0, 5.0, 0.0).unwrap()
        );
        let Geometry::Point(projected_point) = copies[1].geometry() else {
            panic!("expected a projected point")
        };
        assert_eq!(*projected_point, Point3::try_new(-2.0, 7.0, 0.0).unwrap());
        assert_eq!(document.undo_label(), Some("ProjectToCPlane"));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "ProjectToCPlane DeleteInput=Yes")
            .unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        let Geometry::Line(projected_line) = document.object(source_ids[0]).unwrap().geometry()
        else {
            panic!("expected the source line identity")
        };
        assert_eq!(projected_line.start().z(), 0.0);
        assert_eq!(projected_line.end().z(), 0.0);
        let Geometry::Point(projected_point) = document.object(source_ids[1]).unwrap().geometry()
        else {
            panic!("expected the source point identity")
        };
        assert_eq!(projected_point.z(), 0.0);
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "ProjectToCPlane _Yes")
            .unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "ProjectToCPlane DeleteInput=Maybe",
            "ProjectToCPlane Other=Yes",
            "ProjectToCPlane Yes No",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn to_nurbs_matches_rhino_curve_domains_groups_selection_and_delete_input() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let z_axis = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap();
        let line = LineSegment::try_new(
            Point3::try_new(-1.0, 0.0, 2.0).unwrap(),
            Point3::try_new(5.0, 2.0, 4.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let circle = Circle3::try_new(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            2.0,
            z_axis,
            tolerance,
        )
        .unwrap();
        let arc = CircularArc3::try_from_three_points(
            Point3::try_new(3.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            Point3::try_new(-3.0, 0.0, 0.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let x_axis = UnitVector3::try_new(1.0, 0.0, 0.0, tolerance).unwrap();
        let y_axis = UnitVector3::try_new(0.0, 1.0, 0.0, tolerance).unwrap();
        let ellipse = Ellipse3::try_new(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            4.0,
            1.5,
            x_axis,
            y_axis,
            tolerance,
        )
        .unwrap();
        let polyline = Polyline3::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 1.0).unwrap(),
                Point3::try_new(2.0, 3.0, 2.0).unwrap(),
            ],
            tolerance,
        )
        .unwrap();
        let existing_nurbs = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                Point3::try_new(12.0, 0.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 2.0, 2.0],
        )
        .unwrap();
        let geometries = [
            ("line", Geometry::Line(line)),
            ("circle", Geometry::Circle(circle)),
            ("arc", Geometry::Arc(arc)),
            ("ellipse", Geometry::Ellipse(ellipse)),
            ("polyline", Geometry::Polyline(polyline.clone())),
            ("existing", Geometry::NurbsCurve(existing_nurbs.clone())),
            (
                "point",
                Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
            ),
        ];
        let layer = document.current_layer_id();
        let source_ids = geometries
            .into_iter()
            .map(|(name, geometry)| {
                document
                    .add_geometry_with_attributes(
                        geometry,
                        ObjectAttributes::on_layer(layer).with_name(name),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let group = document
            .add_group(
                Some("NURBS fixtures".to_owned()),
                source_ids.iter().copied(),
            )
            .unwrap();
        document
            .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
            .unwrap();

        registry.execute(&mut document, "ToNURBS").unwrap();
        assert_eq!(document.objects().len(), 12);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(document.group(group).unwrap().members().len(), 12);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        let copies = document
            .objects()
            .filter(|object| !source_ids.contains(&object.id()))
            .collect::<Vec<_>>();
        assert_eq!(copies.len(), 5);
        assert!(copies.iter().all(|copy| !document.is_selected(copy.id())));
        for (source, copy) in source_ids.iter().zip(&copies) {
            assert_eq!(
                document.object(*source).unwrap().attributes(),
                copy.attributes()
            );
        }

        let curves = copies
            .iter()
            .map(|object| match object.geometry() {
                Geometry::NurbsCurve(curve) => curve,
                _ => panic!("ToNURBS must create NURBS curves"),
            })
            .collect::<Vec<_>>();
        let line_length = line.length().unwrap();
        assert_eq!(curves[0].knots(), &[0.0, 0.0, line_length, line_length]);
        let circumference = circle.length().unwrap();
        assert_eq!(curves[1].domain(), 0.0..=circumference);
        assert_eq!(
            curves[1].knots(),
            &[
                0.0,
                0.0,
                0.0,
                circumference * 0.25,
                circumference * 0.25,
                circumference * 0.5,
                circumference * 0.5,
                circumference * 0.75,
                circumference * 0.75,
                circumference,
                circumference,
                circumference,
            ]
        );
        assert_eq!(curves[2].domain(), 0.0..=arc.length().unwrap());
        assert_eq!(curves[3].domain(), 0.0..=std::f64::consts::TAU);
        let first_segment = polyline.segments().next().unwrap().length().unwrap();
        let polyline_length = polyline.length().unwrap();
        assert_eq!(
            curves[4].knots(),
            &[0.0, 0.0, first_segment, polyline_length, polyline_length]
        );
        assert_eq!(document.undo_label(), Some("ToNURBS"));

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 7);
        assert_eq!(document.group(group).unwrap().members().len(), 7);
        registry
            .execute(&mut document, "ToNURBS DeleteInputObjects=Yes")
            .unwrap();
        assert_eq!(document.objects().len(), 7);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(document.group(group).unwrap().members().len(), 7);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        for id in &source_ids[..5] {
            assert!(matches!(
                document.object(*id).unwrap().geometry(),
                Geometry::NurbsCurve(_)
            ));
        }
        assert_eq!(
            document.object(source_ids[5]).unwrap().geometry(),
            &Geometry::NurbsCurve(existing_nurbs)
        );
        assert!(matches!(
            document.object(source_ids[6]).unwrap().geometry(),
            Geometry::Point(_)
        ));

        registry.execute(&mut document, "Undo").unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "ToNURBS DeleteInputObjects=Maybe",
            "ToNURBS Other=Yes",
            "ToNURBS Yes No",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 7);
        assert_eq!(document.group(group).unwrap().members().len(), 7);
        assert_eq!(document.undo_label(), history.as_deref());

        document
            .select_objects_direct([source_ids[5], source_ids[6]], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ToNURBS"),
            Err(CommandError::NoConvertibleNurbsCurves)
        ));
        assert_eq!(document.objects().len(), 7);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn extrude_curve_matches_rhino_domains_attributes_groups_and_delete_input() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let output_layer = document.current_layer_id();
        let input_layer = document
            .add_layer("Profiles", ColorRgb::new(91, 92, 93))
            .unwrap();
        let line = LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let polyline = Polyline3::try_new(
            vec![
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
                Point3::try_new(2.0, 1.0, 0.0).unwrap(),
                Point3::try_new(2.0, 4.0, 0.0).unwrap(),
            ],
            tolerance,
        )
        .unwrap();
        let circle = Circle3::try_new(
            Point3::try_new(8.0, 0.0, 0.0).unwrap(),
            2.0,
            UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
            tolerance,
        )
        .unwrap();
        let existing_nurbs = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(12.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(13.0, 2.0, 0.0).unwrap(), 0.75).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(15.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let geometries = [
            ("line", Geometry::Line(line)),
            ("polyline", Geometry::Polyline(polyline.clone())),
            ("circle", Geometry::Circle(circle)),
            ("nurbs", Geometry::NurbsCurve(existing_nurbs.clone())),
            (
                "point",
                Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
            ),
        ];
        let source_ids = geometries
            .into_iter()
            .map(|(name, geometry)| {
                document
                    .add_geometry_with_attributes(
                        geometry,
                        ObjectAttributes::on_layer(input_layer)
                            .with_name(name)
                            .with_object_color(ColorRgb::new(12, 34, 56)),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let group = document
            .add_group(
                Some("Extrusion profiles".to_owned()),
                source_ids.iter().copied(),
            )
            .unwrap();
        document
            .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
            .unwrap();

        registry
            .execute(&mut document, "ExtrudeCrv 5 Output=Surface Solid=No")
            .unwrap();
        assert_eq!(document.objects().len(), 9);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(document.group(group).unwrap().members().len(), 5);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        let outputs = document
            .objects()
            .filter(|object| !source_ids.contains(&object.id()))
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 4);
        for output in &outputs {
            assert!(!document.is_selected(output.id()));
            assert_eq!(output.attributes().layer_id(), output_layer);
            assert_eq!(output.attributes().name(), None);
            assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
            assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
        }
        let surfaces = outputs
            .iter()
            .map(|object| match object.geometry() {
                Geometry::NurbsSurface(surface) => surface,
                _ => panic!("ExtrudeCrv must create NURBS surfaces"),
            })
            .collect::<Vec<_>>();
        assert_eq!(surfaces[0].knots_u(), &[0.0, 0.0, 4.0, 4.0]);
        assert_eq!(surfaces[0].knots_v(), &[0.0, 0.0, 5.0, 5.0]);
        assert_eq!(
            surfaces[0].evaluate(0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap()
        );
        assert_eq!(
            surfaces[0].evaluate(4.0, 5.0).unwrap(),
            Point3::try_new(4.0, 0.0, 5.0).unwrap()
        );
        assert_eq!(surfaces[1].knots_u(), &[0.0, 0.0, 2.0, 5.0, 5.0]);
        assert_eq!(surfaces[2].domain_u(), 0.0..=circle.length().unwrap());
        assert!(surfaces[2].is_rational());
        assert_eq!(surfaces[3].knots_u(), existing_nurbs.knots());
        assert_eq!(surfaces[3].domain_u(), 2.0..=7.0);
        assert_eq!(document.undo_label(), Some("ExtrudeCrv"));

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.group(group).unwrap().members().len(), 5);
        registry
            .execute(
                &mut document,
                "ExtrudeCrv 0,0,0 0,3,4 BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.groups().len(), 1);
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            vec![source_ids[4]]
        );
        assert!(
            source_ids[..4]
                .iter()
                .all(|id| document.object(*id).is_none())
        );
        assert!(document.is_selected(source_ids[4]));
        let both_sides_outputs = document
            .objects()
            .filter(|object| object.id() != source_ids[4])
            .collect::<Vec<_>>();
        assert_eq!(both_sides_outputs.len(), 4);
        assert!(
            both_sides_outputs
                .iter()
                .all(|object| !document.is_selected(object.id()))
        );
        let Geometry::NurbsSurface(both_sides_line) = both_sides_outputs[0].geometry() else {
            panic!("expected an extruded line surface")
        };
        assert_eq!(both_sides_line.domain_v(), 0.0..=10.0);
        assert_eq!(
            both_sides_line.evaluate(0.0, 0.0).unwrap(),
            Point3::try_new(0.0, -3.0, -4.0).unwrap()
        );
        assert_eq!(
            both_sides_line.evaluate(0.0, 10.0).unwrap(),
            Point3::try_new(0.0, 3.0, 4.0).unwrap()
        );

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.group(group).unwrap().members().len(), 5);
        document
            .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "ExtrudeCrv 0",
            "ExtrudeCrv 0,0,0 0,0,0",
            "ExtrudeCrv 5 BothSides=Yes BothSides=No",
            "ExtrudeCrv 5 DeleteInput=Maybe",
            "ExtrudeCrv 5 Output=SubD",
            "ExtrudeCrv 5 Solid=Yes",
            "ExtrudeCrv 5 extra",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.group(group).unwrap().members().len(), 5);
        assert_eq!(document.undo_label(), history.as_deref());

        document
            .select_objects_direct([source_ids[4]], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtrudeCrv 5"),
            Err(CommandError::NoExtrudableCurves)
        ));
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn extrude_curve_along_curve_matches_rhino_sum_surface_and_path_retention() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let output_layer = document.current_layer_id();
        let input_layer = document
            .add_layer("Profiles", ColorRgb::new(91, 92, 93))
            .unwrap();
        let line = LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let rational_profile = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(4.0, 1.0, 0.0).unwrap(), 0.75).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(3.0, 1.0, 0.0).unwrap(), 1.0).unwrap(),
            ],
            vec![3.0, 3.0, 3.0, 8.0, 8.0, 8.0],
        )
        .unwrap();
        let path = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(11.0, 2.0, 1.0).unwrap(), 0.5).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(12.0, 3.0, 4.0).unwrap(), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let source_ids = [
            document
                .add_geometry_with_attributes(
                    Geometry::Line(line),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("line")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::NurbsCurve(rational_profile.clone()),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("rational")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                    ObjectAttributes::on_layer(input_layer).with_name("point"),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::NurbsCurve(path.clone()),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("Rail")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
        ];
        let group = document
            .add_group(Some("Along-curve inputs".to_owned()), source_ids)
            .unwrap();
        document
            .select_objects_direct(source_ids[..3].iter().copied(), SelectionMode::Replace)
            .unwrap();

        registry
            .execute(
                &mut document,
                "ExtrudeCrvAlongCrv PathName=Rail Output=Surface Solid=No DeleteInput=No SplitAtTangents=No",
            )
            .unwrap();
        assert_eq!(document.objects().len(), 6);
        assert_eq!(document.group(group).unwrap().members().len(), 4);
        assert!(source_ids[..3].iter().all(|id| document.is_selected(*id)));
        assert!(!document.is_selected(source_ids[3]));
        let outputs = document
            .objects()
            .filter(|object| !source_ids.contains(&object.id()))
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        for output in &outputs {
            assert!(!document.is_selected(output.id()));
            assert_eq!(output.attributes().layer_id(), output_layer);
            assert_eq!(output.attributes().name(), None);
            assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
            assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
        }
        let surfaces = outputs
            .iter()
            .map(|object| match object.geometry() {
                Geometry::NurbsSurface(surface) => surface,
                _ => panic!("ExtrudeCrvAlongCrv must create NURBS surfaces"),
            })
            .collect::<Vec<_>>();
        let line_surface = surfaces
            .iter()
            .find(|surface| surface.control_point_count_u() == 2)
            .unwrap();
        assert_eq!(line_surface.degree_u(), 1);
        assert_eq!(line_surface.degree_v(), path.degree());
        assert_eq!(line_surface.knots_u(), &[0.0, 0.0, 2.0, 2.0]);
        assert_eq!(line_surface.knots_v(), path.knots());
        assert_eq!(
            line_surface.control_point(0, 0).unwrap().point(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap()
        );
        assert_eq!(
            line_surface.control_point(1, 1).unwrap().point(),
            Point3::try_new(1.0, 4.0, 1.0).unwrap()
        );
        assert_eq!(line_surface.control_point(1, 1).unwrap().weight(), 0.5);
        let rational_surface = surfaces
            .iter()
            .find(|surface| surface.control_point_count_u() == 3)
            .unwrap();
        assert_eq!(rational_surface.knots_u(), rational_profile.knots());
        assert_eq!(rational_surface.knots_v(), path.knots());
        assert_eq!(
            rational_surface.control_point(1, 1).unwrap().point(),
            Point3::try_new(5.0, 3.0, 1.0).unwrap()
        );
        assert_eq!(
            rational_surface.control_point(1, 1).unwrap().weight(),
            0.375
        );
        assert_eq!(document.undo_label(), Some("ExtrudeCrvAlongCrv"));

        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_objects_direct(source_ids[..3].iter().copied(), SelectionMode::Replace)
            .unwrap();
        document
            .select_objects_direct([source_ids[3]], SelectionMode::Add)
            .unwrap();
        registry
            .execute(&mut document, "ExtrudeCrvAlongCrv DeleteInput=Yes")
            .unwrap();
        assert_eq!(document.objects().len(), 4);
        let remaining_group_members = document.group(group).unwrap().members().collect::<Vec<_>>();
        assert_eq!(remaining_group_members.len(), 2);
        assert!(remaining_group_members.contains(&source_ids[2]));
        assert!(remaining_group_members.contains(&source_ids[3]));
        assert!(document.object(source_ids[0]).is_none());
        assert!(document.object(source_ids[1]).is_none());
        assert!(document.is_selected(source_ids[2]));
        assert!(!document.is_selected(source_ids[3]));
        assert!(
            document
                .objects()
                .filter(|object| ![source_ids[2], source_ids[3]].contains(&object.id()))
                .all(|object| !document.is_selected(object.id()))
        );

        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_objects_direct(source_ids[..3].iter().copied(), SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "ExtrudeCrvAlongCrv PathName=Missing",
            "ExtrudeCrvAlongCrv PathName=point",
            "ExtrudeCrvAlongCrv PathName=Rail DeleteInput=Maybe",
            "ExtrudeCrvAlongCrv PathName=Rail Output=SubD",
            "ExtrudeCrvAlongCrv PathName=Rail Solid=Yes",
            "ExtrudeCrvAlongCrv PathName=Rail SplitAtTangents=Maybe",
            "ExtrudeCrvAlongCrv PathName=Rail extra",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.group(group).unwrap().members().len(), 4);
        assert_eq!(document.undo_label(), history.as_deref());

        document
            .select_objects_direct([source_ids[3]], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtrudeCrvAlongCrv"),
            Err(CommandError::CurveAlongCurveProfilesRequired)
        ));
        document
            .select_objects_direct([source_ids[2]], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtrudeCrvAlongCrv PathName=Rail"),
            Err(CommandError::NoCurveAlongCurveProfiles)
        ));
        assert_eq!(document.objects().len(), 4);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn extrude_curve_to_point_matches_rhino_nurbs_form_and_document_behavior() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let output_layer = document.current_layer_id();
        let input_layer = document
            .add_layer("Profiles", ColorRgb::new(91, 92, 93))
            .unwrap();
        let line = LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let rational_curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(12.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(13.0, 2.0, 0.0).unwrap(), 0.75).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(15.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let source_ids = [
            document
                .add_geometry_with_attributes(
                    Geometry::Line(line),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("line")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::NurbsCurve(rational_curve.clone()),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("rational")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                    ObjectAttributes::on_layer(input_layer).with_name("point"),
                )
                .unwrap(),
        ];
        let group = document
            .add_group(Some("Apex profiles".to_owned()), source_ids)
            .unwrap();
        document
            .select_objects_direct(source_ids, SelectionMode::Replace)
            .unwrap();

        registry
            .execute(
                &mut document,
                "ExtrudeCrvToPoint 1,2,5 Output=Surface Solid=No",
            )
            .unwrap();
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.group(group).unwrap().members().len(), 3);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        let outputs = document
            .objects()
            .filter(|object| !source_ids.contains(&object.id()))
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        for output in &outputs {
            assert!(!document.is_selected(output.id()));
            assert_eq!(output.attributes().layer_id(), output_layer);
            assert_eq!(output.attributes().name(), None);
            assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
            assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
        }
        let surfaces = outputs
            .iter()
            .map(|object| match object.geometry() {
                Geometry::NurbsSurface(surface) => surface,
                _ => panic!("ExtrudeCrvToPoint must create NURBS surfaces"),
            })
            .collect::<Vec<_>>();
        let apex = Point3::try_new(1.0, 2.0, 5.0).unwrap();
        let line_apex_distance = Point3::try_new(0.0, 0.0, 0.0)
            .unwrap()
            .distance_to(apex)
            .unwrap();
        assert_eq!(surfaces[0].degree_u(), 1);
        assert_eq!(surfaces[0].degree_v(), 1);
        assert_eq!(surfaces[0].control_point_count_u(), 2);
        assert_eq!(surfaces[0].control_point_count_v(), 2);
        assert_eq!(
            surfaces[0].knots_u(),
            &[0.0, 0.0, line_apex_distance, line_apex_distance]
        );
        assert_eq!(surfaces[0].knots_v(), &[0.0, 0.0, 4.0, 4.0]);
        assert_eq!(
            surfaces[0].evaluate(0.0, 4.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap()
        );
        assert_eq!(surfaces[0].evaluate(line_apex_distance, 2.0).unwrap(), apex);
        assert_eq!(surfaces[1].degree_u(), 1);
        assert_eq!(surfaces[1].degree_v(), rational_curve.degree());
        assert_eq!(surfaces[1].knots_v(), rational_curve.knots());
        assert_eq!(surfaces[1].domain_v(), rational_curve.domain());
        for (index, control) in rational_curve.control_points().iter().enumerate() {
            assert_eq!(surfaces[1].control_point(0, index), Some(*control));
            let apex_control = surfaces[1].control_point(1, index).unwrap();
            assert_eq!(apex_control.point(), apex);
            assert_eq!(apex_control.weight(), control.weight());
        }
        assert_eq!(document.undo_label(), Some("ExtrudeCrvToPoint"));

        registry.execute(&mut document, "Undo").unwrap();
        registry
            .execute(&mut document, "ExtrudeCrvToPoint 1 2 5 DeleteInput=Yes")
            .unwrap();
        assert_eq!(document.objects().len(), 3);
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            vec![source_ids[2]]
        );
        assert!(document.object(source_ids[0]).is_none());
        assert!(document.object(source_ids[1]).is_none());
        assert!(document.is_selected(source_ids[2]));
        assert!(
            document
                .objects()
                .filter(|object| object.id() != source_ids[2])
                .all(|object| !document.is_selected(object.id()))
        );

        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_objects_direct(source_ids, SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "ExtrudeCrvToPoint",
            "ExtrudeCrvToPoint 0,0,0",
            "ExtrudeCrvToPoint 1,2,5 DeleteInput=Maybe",
            "ExtrudeCrvToPoint 1,2,5 DeleteInput=Yes DeleteInput=No",
            "ExtrudeCrvToPoint 1,2,5 Output=SubD",
            "ExtrudeCrvToPoint 1,2,5 Solid=Yes",
            "ExtrudeCrvToPoint 1,2,5 extra",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.group(group).unwrap().members().len(), 3);
        assert_eq!(document.undo_label(), history.as_deref());

        document
            .select_objects_direct([source_ids[2]], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "ExtrudeCrvToPoint 1,2,5"),
            Err(CommandError::NoExtrudableCurves)
        ));
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn revolve_matches_rhino_exact_surface_attributes_groups_and_delete_input() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let output_layer = document.current_layer_id();
        let input_layer = document
            .add_layer("Profiles", ColorRgb::new(91, 92, 93))
            .unwrap();
        let line = LineSegment::try_new(
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 3.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let rational_curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 1.5).unwrap(), 0.75).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 3.0).unwrap(), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let source_ids = [
            document
                .add_geometry_with_attributes(
                    Geometry::Line(line),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("line")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::NurbsCurve(rational_curve.clone()),
                    ObjectAttributes::on_layer(input_layer)
                        .with_name("rational")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap(),
            document
                .add_geometry_with_attributes(
                    Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                    ObjectAttributes::on_layer(input_layer).with_name("point"),
                )
                .unwrap(),
        ];
        let group = document
            .add_group(Some("Revolve profiles".to_owned()), source_ids)
            .unwrap();
        document
            .select_objects_direct(source_ids, SelectionMode::Replace)
            .unwrap();

        registry
            .execute(
                &mut document,
                "Revolve 0,0,0 0,0,1 FullCircle=Yes Output=Surface Deformable=No SplitAtTangents=No",
            )
            .unwrap();
        assert_eq!(document.objects().len(), 5);
        assert_eq!(document.group(group).unwrap().members().len(), 3);
        assert!(source_ids.iter().all(|id| document.is_selected(*id)));
        let outputs = document
            .objects()
            .filter(|object| !source_ids.contains(&object.id()))
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        for output in &outputs {
            assert!(!document.is_selected(output.id()));
            assert_eq!(output.attributes().layer_id(), output_layer);
            assert_eq!(output.attributes().name(), None);
            assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
            assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
        }
        let surfaces = outputs
            .iter()
            .map(|object| match object.geometry() {
                Geometry::NurbsSurface(surface) => surface,
                _ => panic!("Revolve must create NURBS surfaces"),
            })
            .collect::<Vec<_>>();
        assert_eq!(surfaces[0].degree_u(), 2);
        assert_eq!(surfaces[0].degree_v(), 1);
        assert_eq!(surfaces[0].control_point_count_u(), 9);
        assert_eq!(surfaces[0].domain_u(), 0.0..=4.0 * std::f64::consts::PI);
        assert_eq!(surfaces[0].domain_v(), 0.0..=3.0);
        assert_eq!(
            surfaces[0].control_point(8, 0),
            surfaces[0].control_point(0, 0)
        );
        assert_eq!(surfaces[1].degree_v(), rational_curve.degree());
        assert_eq!(surfaces[1].knots_v(), rational_curve.knots());
        assert_eq!(surfaces[1].domain_u(), 0.0..=8.0 * std::f64::consts::PI);
        assert_eq!(
            surfaces[1].control_point(1, 1).unwrap().weight(),
            0.75 * 0.5_f64.sqrt()
        );
        assert_eq!(document.undo_label(), Some("Revolve"));

        registry.execute(&mut document, "Undo").unwrap();
        registry
            .execute(
                &mut document,
                "Revolve 0 0 0 0 0 1 120 StartAngle=30 DeleteInput=Yes SplitAtTangents=Yes",
            )
            .unwrap();
        assert_eq!(document.objects().len(), 3);
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            vec![source_ids[2]]
        );
        assert!(document.object(source_ids[0]).is_none());
        assert!(document.object(source_ids[1]).is_none());
        assert!(document.is_selected(source_ids[2]));
        let partial_outputs = document
            .objects()
            .filter(|object| object.id() != source_ids[2])
            .collect::<Vec<_>>();
        assert_eq!(partial_outputs.len(), 2);
        assert!(
            partial_outputs
                .iter()
                .all(|object| !document.is_selected(object.id()))
        );
        let Geometry::NurbsSurface(partial_line) = partial_outputs[0].geometry() else {
            panic!("expected a partially revolved line surface")
        };
        assert_eq!(partial_line.control_point_count_u(), 5);
        assert_eq!(partial_line.domain_u(), 0.0..=2.0 * 120.0_f64.to_radians());
        assert!(partial_line.evaluate(0.0, 0.0).unwrap().is_near(
            Point3::try_new(3.0_f64.sqrt(), 1.0, 0.0).unwrap(),
            tolerance
        ));
        assert!(
            partial_line
                .evaluate(*partial_line.domain_u().end(), 3.0)
                .unwrap()
                .is_near(
                    Point3::try_new(-3.0_f64.sqrt(), 1.0, 3.0).unwrap(),
                    tolerance
                )
        );

        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_objects_direct(source_ids, SelectionMode::Replace)
            .unwrap();
        let history = document.undo_label().map(str::to_owned);
        for command in [
            "Revolve",
            "Revolve 0,0,0 0,0,0 90",
            "Revolve 0,0,0 0,0,1 0",
            "Revolve 0,0,0 0,0,1 361",
            "Revolve 0,0,0 0,0,1 90 FullCircle=Yes",
            "Revolve 0,0,0 0,0,1 FullCircle=Maybe",
            "Revolve 0,0,0 0,0,1 90 DeleteInput=Yes DeleteInput=No",
            "Revolve 0,0,0 0,0,1 90 Output=SubD",
            "Revolve 0,0,0 0,0,1 90 Deformable=Yes",
            "Revolve 0,0,0 0,0,1 90 SplitAtTangents=Maybe",
            "Revolve 0,0,0 0,0,1 90 extra",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.group(group).unwrap().members().len(), 3);
        assert_eq!(document.undo_label(), history.as_deref());

        document
            .select_objects_direct([source_ids[2]], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "Revolve 0,0,0 0,0,1 90"),
            Err(CommandError::NoRevolvableCurves)
        ));
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    #[test]
    fn rotate_three_dimensional_and_transform_copy_options_are_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 2,0,1").unwrap();
        let original = document.objects().next().unwrap().id();
        document
            .select_object(original, SelectionMode::Replace)
            .unwrap();
        let position = |document: &Document, id: ObjectId| {
            let Geometry::Point(point) = document.object(id).unwrap().geometry() else {
                panic!("expected a point")
            };
            *point
        };

        registry
            .execute(&mut document, "Rotate3D 0,0,0 0,0,2 90")
            .unwrap();
        assert!(position(&document, original).is_near(
            Point3::try_new(0.0, 2.0, 1.0).unwrap(),
            document.tolerance()
        ));
        assert_eq!(document.undo_label(), Some("Rotate3D"));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Rotate3D 0,0,0 0,0,2 1,0,5 0,1,-2 Copy=Yes")
            .unwrap();
        assert_eq!(
            position(&document, original),
            Point3::try_new(2.0, 0.0, 1.0).unwrap()
        );
        let copy = document.objects().nth(1).unwrap().id();
        assert!(position(&document, copy).is_near(
            Point3::try_new(0.0, 2.0, 1.0).unwrap(),
            document.tolerance()
        ));
        assert!(document.is_selected(original));
        assert!(!document.is_selected(copy));
        registry.execute(&mut document, "Undo").unwrap();

        registry
            .execute(&mut document, "Rotate 0,0 90 Copy=Yes")
            .unwrap();
        assert_eq!(document.objects().len(), 2);
        registry.execute(&mut document, "Undo").unwrap();
        registry
            .execute(&mut document, "Mirror 0,0 0,1 Copy=Yes")
            .unwrap();
        let mirrored = document.objects().nth(1).unwrap().id();
        assert_eq!(
            position(&document, mirrored),
            Point3::try_new(-2.0, 0.0, 1.0).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();

        let history = document.undo_label().map(str::to_owned);
        for command in [
            "Rotate3D 0,0,0 0,0,0 90",
            "Rotate3D 0,0,0 0,0,2 0,0,1 1,0,0",
            "Rotate3D 0,0,0 0,0,2 90 Copy=Yes Copy=No",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
        }
        assert_eq!(document.objects().len(), 1);
        assert_eq!(
            position(&document, original),
            Point3::try_new(2.0, 0.0, 1.0).unwrap()
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }

    struct MutateThenFailCommand;

    impl Command for MutateThenFailCommand {
        fn name(&self) -> &'static str {
            "MutateThenFail"
        }

        fn run(
            &self,
            document: &mut Document,
            _arguments: &[&str],
        ) -> Result<String, CommandError> {
            document.add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 9.0).unwrap()))?;
            Err(CommandError::Usage("this command always fails"))
        }
    }

    #[test]
    fn failed_command_rolls_back_mutations_atomically() {
        let mut registry = CommandRegistry::default();
        registry.register(MutateThenFailCommand).unwrap();
        let mut document = Document::default();
        assert!(registry.execute(&mut document, "MutateThenFail").is_err());
        assert_eq!(document.objects().len(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn imports_and_exports_stl_through_commands() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-source model.stl",
            std::process::id()
        ));
        let output = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-output model.stl",
            std::process::id()
        ));
        fs::write(
            &source,
            "solid test\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid test\n",
        )
        .unwrap();

        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, &format!("ImportStl {}", source.display()))
            .unwrap();
        assert!(matches!(
            document.objects().next().unwrap().geometry(),
            Geometry::Mesh(_)
        ));
        registry
            .execute(
                &mut document,
                &format!("ExportStl Binary {}", output.display()),
            )
            .unwrap();
        let exported = read_stl_file(&output, document.tolerance()).unwrap();
        assert_eq!(exported.triangles().len(), 1);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 0);
        fs::remove_file(source).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn imports_and_exports_step_without_polluting_undo_history() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        use monstertruck::modeling::{BoundingBox, Point3 as TruckPoint3, primitive};
        use monstertruck::step::save::{
            CompleteStepDisplay, StepHeaderDescriptor, StepModel as TruckStepModel,
        };

        let cube: monstertruck::modeling::Solid = primitive::cuboid(BoundingBox::from_iter([
            TruckPoint3::new(0.0, 0.0, 0.0),
            TruckPoint3::new(2.0, 3.0, 4.0),
        ]));
        let compressed = cube.compress();
        let step = CompleteStepDisplay::new(
            TruckStepModel::from(&compressed),
            StepHeaderDescriptor {
                organization_system: "Viboceros command test".to_owned(),
                ..Default::default()
            },
        )
        .to_string();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-command model.step",
            std::process::id()
        ));
        let output = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-command output.step",
            std::process::id()
        ));
        fs::write(&path, step).unwrap();

        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let message = registry
            .execute(&mut document, &format!("ImportStep {}", path.display()))
            .unwrap();
        assert!(message.contains("1 STEP mesh object"));
        assert!(message.contains("12 triangles"));
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.undo_label(), Some("ImportStep"));
        let Geometry::Mesh(mesh) = document.objects().next().unwrap().geometry() else {
            panic!("expected imported STEP mesh")
        };
        assert_eq!(mesh.bounds().max(), Point3::try_new(2.0, 3.0, 4.0).unwrap());

        let export_message = registry
            .execute(&mut document, &format!("ExportStep {}", output.display()))
            .unwrap();
        assert!(export_message.contains("12 triangles"));
        let exported = read_step_file(&output, document.tolerance()).unwrap();
        assert_eq!(exported.objects.len(), 1);
        assert_eq!(exported.objects[0].mesh.triangles().len(), 12);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 0);
        fs::remove_file(path).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn imports_and_exports_3dm_with_layers_and_groups_as_one_undo_step() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-command model.3dm",
            std::process::id()
        ));

        let registry = CommandRegistry::with_builtins();
        let mut source = Document::default();
        registry
            .execute(&mut source, "Layer New Reference")
            .unwrap();
        registry
            .execute(&mut source, "Layer Color 12,34,56 Reference")
            .unwrap();
        registry.execute(&mut source, "Point 1,2,3").unwrap();
        registry
            .execute(&mut source, "Layer Current Default")
            .unwrap();
        registry.execute(&mut source, "Line 0,0,0 4,0,0").unwrap();
        registry.execute(&mut source, "Circle 6,0,0 2").unwrap();
        registry
            .execute(&mut source, "Arc 9,0,0 10,1,0 11,0,0")
            .unwrap();
        registry
            .execute(&mut source, "Ellipse 15,0,0 18,0,0 16,2,0")
            .unwrap();
        registry
            .execute(&mut source, "Rectangle 12,0,0 14,2,0")
            .unwrap();
        registry.execute(&mut source, "Polygon 5 20,0,0 2").unwrap();
        registry
            .execute(&mut source, "SrfPt 0,0,0 2,0,0 2,2,1 0,2,1")
            .unwrap();
        let point_cloud_locations = vec![
            Point3::try_new(-3.0, 2.0, 7.0).unwrap(),
            Point3::try_new(8.0, -1.0, 4.0).unwrap(),
            Point3::try_new(-3.0, 2.0, 7.0).unwrap(),
        ];
        source
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(point_cloud_locations.clone()).unwrap(),
            ))
            .unwrap();
        let source_ids = source
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        source
            .set_objects_color(
                [source_ids[0], source_ids[1]],
                Some(ColorRgb::new(201, 45, 67)),
            )
            .unwrap();
        source
            .select_object(source_ids[1], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut source, "Hide").unwrap();
        source
            .select_object(source_ids[0], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut source, "Lock").unwrap();
        registry
            .execute(&mut source, "Layer Hide Reference")
            .unwrap();
        registry
            .execute(&mut source, "Layer Lock Reference")
            .unwrap();
        source
            .add_group(
                Some("Assembly".to_owned()),
                [source_ids[0], source_ids[1], source_ids[8]],
            )
            .unwrap();
        source
            .add_group(
                Some("Inspection".to_owned()),
                [source_ids[1], source_ids[8]],
            )
            .unwrap();
        source
            .add_group(None, [source_ids[2], source_ids[8]])
            .unwrap();
        source
            .add_empty_group(Some("Empty Fixture".to_owned()))
            .unwrap();
        let export_message = registry
            .execute(&mut source, &format!("Export3dm {}", path.display()))
            .unwrap();
        assert!(export_message.contains("9 objects in 4 groups"));

        let mut imported = Document::default();
        let message = registry
            .execute(&mut imported, &format!("Import3dm {}", path.display()))
            .unwrap();
        assert!(message.contains("9 objects in 4 groups"));
        assert!(message.contains("0 unsupported objects skipped"));
        assert_eq!(imported.objects().len(), 9);
        let imported_ids = imported
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        assert_eq!(imported.groups().len(), 4);
        assert_eq!(
            imported
                .group_by_name("Assembly")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([imported_ids[0], imported_ids[1], imported_ids[8]])
        );
        assert_eq!(
            imported
                .group_by_name("Inspection")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([imported_ids[1], imported_ids[8]])
        );
        assert_eq!(
            imported
                .group_by_name("Group01")
                .unwrap()
                .members()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([imported_ids[2], imported_ids[8]])
        );
        assert_eq!(
            imported
                .group_by_name("Empty Fixture")
                .unwrap()
                .members()
                .len(),
            0
        );
        let imported_cloud = imported
            .objects()
            .find_map(|object| match object.geometry() {
                Geometry::PointCloud(cloud) => Some(cloud),
                _ => None,
            })
            .unwrap();
        assert_eq!(imported_cloud.points(), point_cloud_locations);
        assert!(
            imported
                .objects()
                .any(|object| matches!(object.geometry(), Geometry::NurbsSurface(_)))
        );
        assert_eq!(
            imported
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::NurbsCurve(_)))
                .count(),
            3
        );
        assert_eq!(
            imported
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::Polyline(_)))
                .count(),
            2
        );
        assert_eq!(imported.layers().len(), 3);
        let reference = imported.layer_by_name("Reference").unwrap();
        assert_eq!(reference.color(), ColorRgb::new(12, 34, 56));
        assert!(!reference.is_visible());
        assert!(reference.is_locked());
        assert!(imported.layer_by_name("Default (Imported 1)").is_some());
        let point = imported
            .objects()
            .find(|object| matches!(object.geometry(), Geometry::Point(_)))
            .unwrap();
        assert!(point.attributes().is_visible());
        assert!(point.attributes().is_locked());
        let line = imported
            .objects()
            .find(|object| matches!(object.geometry(), Geometry::Line(_)))
            .unwrap();
        assert!(!line.attributes().is_visible());
        assert!(!line.attributes().is_locked());
        for id in [imported_ids[0], imported_ids[1]] {
            let attributes = imported.object(id).unwrap().attributes();
            assert_eq!(attributes.color_source(), ObjectColorSource::Object);
            assert_eq!(attributes.object_color(), ColorRgb::new(201, 45, 67));
        }

        assert_eq!(imported.undo_label(), Some("Import3dm"));
        registry
            .execute(&mut imported, &format!("Import3dm {}", path.display()))
            .unwrap();
        assert_eq!(imported.objects().len(), 18);
        assert!(imported.group_by_name("Assembly (Imported 1)").is_some());
        assert!(imported.group_by_name("Inspection (Imported 1)").is_some());
        assert!(imported.group_by_name("Group01 (Imported 1)").is_some());
        assert!(
            imported
                .group_by_name("Empty Fixture (Imported 1)")
                .is_some()
        );
        registry.execute(&mut imported, "Undo").unwrap();
        assert_eq!(imported.objects().len(), 9);
        assert_eq!(imported.groups().len(), 4);
        registry.execute(&mut imported, "Undo").unwrap();
        assert_eq!(imported.objects().len(), 0);
        assert_eq!(imported.groups().len(), 0);
        assert_eq!(imported.layers().len(), 1);
        fs::remove_file(path).unwrap();
    }
}
