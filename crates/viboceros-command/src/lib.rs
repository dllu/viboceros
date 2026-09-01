//! Extensible command registry and the first model-editing commands.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, GroupId, LayerId, ObjectAttributes, ObjectId,
    SelectionMode,
};
use viboceros_geometry::{
    AffineTransform3, Circle3, CircularArc3, CurveRef, Ellipse3, GeometryError, LineSegment,
    MAX_CURVE_DIVISION_POINTS, MAX_REGULAR_POLYGON_SIDES, MeshFaceExtraction, NurbsCurve,
    NurbsSurface, Point3, Polyline3, PolylineClosure, Real, Tolerance, TriangleMesh, UnitVector3,
    Vector3, join_polylines,
};
use viboceros_io::{
    StepError, StlError, StlFormat, ThreeDmError, ThreeDmGeometry, ThreeDmLayer, ThreeDmModel,
    ThreeDmObject, read_3dm_file, read_step_file, read_stl_file, write_3dm_file, write_step_file,
    write_stl_file,
};

const SURFACE_EXPORT_SAMPLES_PER_SPAN: usize = 16;
const MAX_EXTRACTED_POINT_OBJECTS: usize = 1_000_000;

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
            .register(ControlPointCurveCommand)
            .expect("unique built-in command");
        registry
            .register(SrfPtCommand)
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
            .register(ScaleCommand)
            .expect("unique built-in command");
        registry
            .register(RotateCommand)
            .expect("unique built-in command");
        registry
            .register(MirrorCommand)
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

const EXTRACT_PT_USAGE: &str = "ExtractPt [OutputLayer=Input|Current] [Output=Points]";

struct ExtractPtCommand;

impl Command for ExtractPtCommand {
    fn name(&self) -> &'static str {
        "ExtractPt"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let output_layer = parse_extract_point_arguments(arguments)?;
        let selected = document
            .objects()
            .filter(|object| document.is_selected(object.id()))
            .map(|object| (object.geometry().clone(), object.attributes().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let current_layer = document.current_layer_id();
        let mut output = Vec::new();
        let mut source_with_points = 0;
        for (geometry, attributes) in &selected {
            let points = geometry.extract_point_locations()?;
            if points.is_empty() {
                continue;
            }
            source_with_points += 1;
            let output_count = output.len().checked_add(points.len()).ok_or(
                CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINT_OBJECTS,
                },
            )?;
            if output_count > MAX_EXTRACTED_POINT_OBJECTS {
                return Err(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINT_OBJECTS,
                });
            }
            output
                .try_reserve(points.len())
                .map_err(|_| CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINT_OBJECTS,
                })?;
            let attributes = match output_layer {
                ExtractPointOutputLayer::Input => attributes.clone(),
                ExtractPointOutputLayer::Current => attributes.clone().with_layer(current_layer),
            };
            output.extend(points.into_iter().map(|point| (point, attributes.clone())));
        }
        if output.is_empty() {
            return Err(CommandError::NoExtractablePoints);
        }

        let mut ids = Vec::with_capacity(output.len());
        for (point, attributes) in output {
            ids.push(document.add_geometry_with_attributes(Geometry::Point(point), attributes)?);
        }
        replace_selection(document, ids.iter().copied())?;
        Ok(format!(
            "Extracted {} point(s) from {source_with_points} of {} selected object(s)",
            ids.len(),
            selected.len()
        ))
    }
}

#[derive(Clone, Copy)]
enum ExtractPointOutputLayer {
    Input,
    Current,
}

fn parse_extract_point_arguments(
    arguments: &[&str],
) -> Result<ExtractPointOutputLayer, CommandError> {
    let mut output_layer = ExtractPointOutputLayer::Input;
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
                return Err(CommandError::UnsupportedExtractPointCloudOutput);
            }
            if !value.eq_ignore_ascii_case("Points") {
                return Err(CommandError::Usage(EXTRACT_PT_USAGE));
            }
            output_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_PT_USAGE));
        }
        index += consumed;
    }
    Ok(output_layer)
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
    for object in document
        .objects()
        .filter(|object| document.is_selected(object.id()))
    {
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
            .objects()
            .filter(|object| document.is_selected(object.id()))
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
                    polyline.segments().collect::<Vec<_>>(),
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
        let line_count = exploded
            .iter()
            .map(|(_, segments, _)| segments.len())
            .sum::<usize>();
        for (id, _, _) in &exploded {
            document.delete_object(*id)?;
        }
        let mut result_ids = Vec::with_capacity(line_count);
        for (_, segments, attributes) in exploded {
            for segment in segments {
                result_ids.push(
                    document.add_geometry_with_attributes(
                        Geometry::Line(segment),
                        attributes.clone(),
                    )?,
                );
            }
        }
        replace_selection(
            document,
            unchanged_ids.into_iter().chain(result_ids.iter().copied()),
        )?;
        Ok(format!(
            "Exploded {} polyline(s) into {line_count} line(s); {} object(s) unchanged",
            exploded_ids.len(),
            selected.len() - exploded_ids.len()
        ))
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

struct ScaleCommand;

impl Command for ScaleCommand {
    fn name(&self) -> &'static str {
        "Scale"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (center, consumed) = parse_point(arguments)?;
        let remaining = &arguments[consumed..];
        let factor = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_nonzero_scale(remaining[0])?
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                "Scale center factor | center reference target",
            )?;
            scale_factor_from_reference(center, reference, target, document.tolerance())?
        };
        let transform = AffineTransform3::try_uniform_scale(center, factor)?;
        let count = document.transform_objects(selected, transform)?;
        Ok(format!("Scaled {count} object(s) by {factor:.6}"))
    }
}

struct RotateCommand;

impl Command for RotateCommand {
    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let (center, consumed) = parse_point(arguments)?;
        let remaining = &arguments[consumed..];
        let angle_radians = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?.to_radians()
        } else {
            let (reference, reference_consumed) = parse_point(remaining)?;
            let (target, target_consumed) = parse_point(&remaining[reference_consumed..])?;
            require_consumed(
                remaining,
                reference_consumed + target_consumed,
                "Rotate center degrees | center reference target",
            )?;
            top_view_angle(center, reference, target, document.tolerance())?
        };
        let axis = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance())?;
        let transform = AffineTransform3::try_rotation(center, axis, angle_radians)?;
        let count = document.transform_objects(selected, transform)?;
        Ok(format!(
            "Rotated {count} object(s) by {:.6} degrees",
            angle_radians.to_degrees()
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
        let (axis_start, consumed) = parse_point(arguments)?;
        let (axis_end, end_consumed) = parse_point(&arguments[consumed..])?;
        require_consumed(
            arguments,
            consumed + end_consumed,
            "Mirror axisStart axisEnd",
        )?;
        let normal = top_view_mirror_normal(axis_start, axis_end, document.tolerance())?;
        let transform = AffineTransform3::try_reflection(axis_start, normal)?;
        let count = document.transform_objects(selected, transform)?;
        Ok(format!("Mirrored {count} object(s)"))
    }
}

fn create_current_layer(
    document: &mut Document,
    name_arguments: &[&str],
) -> Result<String, CommandError> {
    let name = joined_argument(name_arguments, "Layer [New] name")?;
    let color = layer_color(document.layers().len());
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

fn scale_factor_from_reference(
    center: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
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

        for object in model.objects {
            let layer_id = imported_layers[object.layer_index];
            let mut attributes = ObjectAttributes::on_layer(layer_id)
                .with_visibility(object.visible)
                .with_locked(object.locked);
            if let Some(name) = object.name {
                attributes = attributes.with_name(name);
            }
            document.add_geometry_with_attributes(
                document_geometry_from_3dm(object.geometry, document.tolerance()),
                attributes,
            )?;
        }

        for (source, id) in model.layers.iter().zip(imported_layers) {
            document.set_layer_visibility(id, source.visible)?;
            document.set_layer_locked(id, source.locked)?;
        }

        Ok(format!(
            "Imported {object_count} objects on {layer_count} layers from '{path}' ({unsupported} unsupported objects skipped)"
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
        let layer_count = model.layers.len();
        write_3dm_file(&path, &model)?;
        Ok(format!(
            "Exported {object_count} objects on {layer_count} layers to '{path}'"
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
    let objects = document
        .objects()
        .map(|object| {
            Ok(ThreeDmObject {
                geometry: geometry_to_3dm(object.geometry())?,
                layer_index: layer_indices[&object.attributes().layer_id()],
                name: object.attributes().name().map(str::to_owned),
                visible: object.attributes().is_visible(),
                locked: object.attributes().is_locked(),
            })
        })
        .collect::<Result<_, GeometryError>>()?;
    Ok(ThreeDmModel::new(layers, objects))
}

fn geometry_to_3dm(geometry: &Geometry) -> Result<ThreeDmGeometry, GeometryError> {
    Ok(match geometry {
        Geometry::Point(point) => ThreeDmGeometry::Point(*point),
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

fn layer_color(index: usize) -> ColorRgb {
    const PALETTE: [ColorRgb; 6] = [
        ColorRgb::new(40, 110, 220),
        ColorRgb::new(220, 75, 60),
        ColorRgb::new(30, 155, 85),
        ColorRgb::new(190, 125, 20),
        ColorRgb::new(145, 80, 190),
        ColorRgb::new(20, 155, 165),
    ];
    PALETTE[index % PALETTE.len()]
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

    #[error("none of the selected objects is an explodable polyline")]
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

    #[error("ExtractPt point-cloud output is not available yet")]
    UnsupportedExtractPointCloudOutput,

    #[error("ExtractPt would create more than {maximum} point objects")]
    TooManyExtractedPoints { maximum: usize },

    #[error(
        "CloseCrv currently supports line and polyline inputs only; open arcs and NURBS curves require polycurve support"
    )]
    UnsupportedCloseCurveGeometry,

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
            "Commands: Arc, Area, ChangeLayer, Circle, Clear, CloseCrv, CombineIdenticalMeshVertices, ControlPointCurve, Copy, CopyToLayer, CrvEnd, CrvStart, CullUnusedMeshVertices, Delete, Divide, Ellipse, Explode, Export3dm, ExportStep, ExportStl, ExtractDuplicateMeshFaces, ExtractNonManifoldMeshEdges, ExtractPt, Flip, Group, Hide, HideSwap, Import3dm, ImportStep, ImportStl, Invert, Isolate, IsolateLock, Join, Layer, Length, Line, Lock, LockSwap, Mirror, Move, Point, Polygon, Polyline, Rectangle, Redo, Rotate, Scale, SelAll, SelClosedCrv, SelClosedMesh, SelCrv, SelDup, SelDupAll, SelGroup, SelLast, SelLayer, SelLine, SelMesh, SelName, SelNone, SelOpenCrv, SelOpenMesh, SelPlanarCrv, SelPolyline, SelPrev, SelPt, SelShortCrv, SelSrf, SetObjectName, Show, SplitDisjointMesh, SrfPt, Undo, Ungroup, UnifyMeshNormals, Unisolate, UnisolateLock, Unlock, Volume"
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

        let object_count = document.objects().len();
        assert!(matches!(
            registry.execute(&mut document, "ExtractPt Output=PointCloud"),
            Err(CommandError::UnsupportedExtractPointCloudOutput)
        ));
        assert_eq!(document.objects().len(), object_count);

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
    fn imports_and_exports_3dm_with_layers_as_one_undo_step() {
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
        let source_ids = source
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
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
        registry
            .execute(&mut source, &format!("Export3dm {}", path.display()))
            .unwrap();

        let mut imported = Document::default();
        let message = registry
            .execute(&mut imported, &format!("Import3dm {}", path.display()))
            .unwrap();
        assert!(message.contains("8 objects"));
        assert!(message.contains("0 unsupported objects skipped"));
        assert_eq!(imported.objects().len(), 8);
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

        assert_eq!(imported.undo_label(), Some("Import3dm"));
        registry.execute(&mut imported, "Undo").unwrap();
        assert_eq!(imported.objects().len(), 0);
        assert_eq!(imported.layers().len(), 1);
        fs::remove_file(path).unwrap();
    }
}
