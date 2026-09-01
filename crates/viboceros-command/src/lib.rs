//! Extensible command registry and the first model-editing commands.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;
use viboceros_document::{
    ColorRgb, Document, DocumentError, Geometry, LayerId, ObjectAttributes, ObjectId, SelectionMode,
};
use viboceros_geometry::{
    AffineTransform3, Circle3, CircularArc3, CurveRef, Ellipse3, GeometryError, LineSegment,
    MAX_CURVE_DIVISION_POINTS, MAX_REGULAR_POLYGON_SIDES, NurbsCurve, NurbsSurface, Point3,
    Polyline3, PolylineClosure, Real, Tolerance, TriangleMesh, UnitVector3, Vector3,
    join_polylines,
};
use viboceros_io::{
    StepError, StlError, StlFormat, ThreeDmError, ThreeDmGeometry, ThreeDmLayer, ThreeDmModel,
    ThreeDmObject, read_3dm_file, read_step_file, read_stl_file, write_3dm_file, write_step_file,
    write_stl_file,
};

const SURFACE_EXPORT_SAMPLES_PER_SPAN: usize = 16;

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
            .register(SelAllCommand)
            .expect("unique built-in command");
        registry
            .register(SelNoneCommand)
            .expect("unique built-in command");
        registry
            .register(InvertCommand)
            .expect("unique built-in command");
        for (name, filter) in [
            ("SelCrv", GeometrySelectionFilter::Curve),
            ("SelOpenCrv", GeometrySelectionFilter::OpenCurve),
            ("SelClosedCrv", GeometrySelectionFilter::ClosedCurve),
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
            .register(LengthCommand)
            .expect("unique built-in command");
        registry
            .register(AreaCommand)
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
            .register(CloseCrvCommand)
            .expect("unique built-in command");
        registry
            .register(FlipCommand)
            .expect("unique built-in command");
        registry
            .register(GroupCommand)
            .expect("unique built-in command");
        registry
            .register(UngroupCommand)
            .expect("unique built-in command");
        registry
            .register(DeleteCommand)
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
        let matches = document
            .objects()
            .filter(|object| document.is_object_selectable(object.id()))
            .map(|object| {
                Ok(self
                    .filter
                    .matches(object.geometry())?
                    .then_some(object.id()))
            })
            .collect::<Result<Vec<Option<ObjectId>>, GeometryError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        for id in matches {
            document.select_object(id, SelectionMode::Add)?;
        }
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
    Line,
    Polyline,
    Point,
    Surface,
    Mesh,
    OpenMesh,
    ClosedMesh,
}

impl GeometrySelectionFilter {
    fn matches(self, geometry: &Geometry) -> Result<bool, GeometryError> {
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
            Self::Line => matches!(geometry, Geometry::Line(_)),
            Self::Polyline => matches!(geometry, Geometry::Polyline(_)),
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
        let (count, total) =
            selected_measurement(document, |geometry, tolerance| match geometry {
                Geometry::Line(line) => Ok(line.length()?),
                Geometry::Circle(circle) => Ok(circle.length()?),
                Geometry::Arc(arc) => Ok(arc.length()?),
                Geometry::Ellipse(ellipse) => Ok(ellipse.length(tolerance)?),
                Geometry::Polyline(polyline) => Ok(polyline.length()?),
                Geometry::NurbsCurve(curve) => Ok(curve.length(tolerance)?),
                _ => Err(CommandError::UnsupportedLengthGeometry),
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
                    _ => return Err(CommandError::UnsupportedFlipGeometry),
                };
                Ok((id, reversed))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let count = document.replace_object_geometries(replacements)?;
        Ok(format!("Flipped {count} curve(s)"))
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
        let name = (!name_arguments.is_empty()).then(|| name_arguments.join(" "));
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
        let id = document.add_group(name.clone(), members)?;
        Ok(match name {
            Some(name) => format!("Created group '{name}' {id} with {member_count} object(s)"),
            None => format!("Created group {id} with {member_count} object(s)"),
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
    document.clear_selection();
    for id in ids {
        document.select_object(id, SelectionMode::Add)?;
    }
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

    #[error("Divide supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedDivideGeometry,

    #[error("the requested division creates no point objects")]
    NoCurveDivisionPoints,

    #[error("Flip supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedFlipGeometry,

    #[error(
        "CrvStart and CrvEnd support selected lines, analytic curves, polylines, and NURBS curves only"
    )]
    UnsupportedCurveEndpointGeometry,

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
            "Commands: Arc, Area, Circle, Clear, CloseCrv, ControlPointCurve, Copy, CrvEnd, CrvStart, Delete, Divide, Ellipse, Explode, Export3dm, ExportStep, ExportStl, Flip, Group, Import3dm, ImportStep, ImportStl, Invert, Join, Layer, Length, Line, Mirror, Move, Point, Polygon, Polyline, Rectangle, Redo, Rotate, Scale, SelAll, SelClosedCrv, SelClosedMesh, SelCrv, SelLine, SelMesh, SelNone, SelOpenCrv, SelOpenMesh, SelPolyline, SelPt, SelSrf, SrfPt, Undo, Ungroup"
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
            "Flipped 2 curve(s)"
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
        let group = document.group_by_name("pair").unwrap();
        assert_eq!(group.members().len(), 2);
        assert!(registry.execute(&mut document, "Group All PAIR").is_err());
        assert_eq!(document.groups().len(), 1);

        registry.execute(&mut document, "Ungroup Pair").unwrap();
        assert_eq!(document.groups().len(), 0);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.group_by_name("Pair").unwrap().members().len(), 2);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.groups().len(), 0);
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

        assert_eq!(imported.undo_label(), Some("Import3dm"));
        registry.execute(&mut imported, "Undo").unwrap();
        assert_eq!(imported.objects().len(), 0);
        assert_eq!(imported.layers().len(), 1);
        fs::remove_file(path).unwrap();
    }
}
