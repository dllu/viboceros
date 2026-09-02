use std::collections::{BTreeSet, VecDeque};

use eframe::egui::{self, RichText};
use viboceros_command::{CommandRegistry, MAX_CURVE_COMMAND_DEGREE};
use viboceros_document::{Document, DocumentError, suggested_layer_color};
use viboceros_geometry::{
    CircularArc3, ControlPointCurveClosure, Ellipse3, Frame3, MAX_REGULAR_POLYGON_SIDES, Point3,
    Tolerance,
};

use crate::sidebar::{DocumentSidebar, SidebarAction};
use crate::viewport::{DisplayMode, DraftingInput, SelectionClick, Viewport, ViewportOutput};

const MAX_LOG_ENTRIES: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveScaleKind {
    Uniform,
    OneDimensional,
    TwoDimensional,
}

impl InteractiveScaleKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Uniform => "Scale",
            Self::OneDimensional => "Scale1D",
            Self::TwoDimensional => "Scale2D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum InteractiveCommand {
    Point,
    Line {
        start: Option<Point3>,
    },
    Circle {
        center: Option<Point3>,
    },
    Sphere {
        center: Option<Point3>,
    },
    Ellipsoid {
        points: [Option<Point3>; 3],
    },
    Arc {
        points: [Option<Point3>; 2],
    },
    Ellipse {
        center: Option<Point3>,
        first_axis: Option<Point3>,
    },
    Polyline,
    Curve {
        degree: usize,
        closure: ControlPointCurveClosure,
    },
    InterpCrv,
    Rectangle {
        first: Option<Point3>,
    },
    Polygon {
        side_count: usize,
        center: Option<Point3>,
    },
    SrfPt {
        corners: [Option<Point3>; 3],
    },
    Move {
        start: Option<Point3>,
    },
    Copy {
        start: Option<Point3>,
    },
    ArrayLinear {
        item_count: usize,
        start: Option<Point3>,
    },
    Array {
        counts: [usize; 3],
        fill: bool,
        z_distance: f64,
        start: Option<Point3>,
    },
    ArrayPolar {
        item_count: usize,
        fill_angle_degrees: f64,
        rotate: bool,
        z_offset: f64,
    },
    Scale {
        kind: InteractiveScaleKind,
        center: Option<Point3>,
        reference: Option<Point3>,
    },
    Rotate {
        center: Option<Point3>,
        reference: Option<Point3>,
    },
    Rotate3D {
        points: [Option<Point3>; 3],
    },
    Mirror {
        start: Option<Point3>,
    },
    Shear {
        origin: Option<Point3>,
        reference: Option<Point3>,
    },
    ExtrudeCurve {
        base: Option<Point3>,
        both_sides: bool,
        delete_input: bool,
    },
    ExtrudeCurveToPoint {
        delete_input: bool,
        solid: bool,
    },
    Revolve {
        axis_start: Option<Point3>,
        start_angle_degrees: f64,
        sweep_degrees: f64,
        delete_input: bool,
    },
}

impl InteractiveCommand {
    const fn name(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Line { .. } => "Line",
            Self::Circle { .. } => "Circle",
            Self::Sphere { .. } => "Sphere",
            Self::Ellipsoid { .. } => "Ellipsoid",
            Self::Arc { .. } => "Arc",
            Self::Ellipse { .. } => "Ellipse",
            Self::Polyline => "Polyline",
            Self::Curve { .. } => "Curve",
            Self::InterpCrv => "InterpCrv",
            Self::Rectangle { .. } => "Rectangle",
            Self::Polygon { .. } => "Polygon",
            Self::SrfPt { .. } => "SrfPt",
            Self::Move { .. } => "Move",
            Self::Copy { .. } => "Copy",
            Self::ArrayLinear { .. } => "ArrayLinear",
            Self::Array { .. } => "Array",
            Self::ArrayPolar { .. } => "ArrayPolar",
            Self::Scale { kind, .. } => kind.name(),
            Self::Rotate { .. } => "Rotate",
            Self::Rotate3D { .. } => "Rotate3D",
            Self::Mirror { .. } => "Mirror",
            Self::Shear { .. } => "Shear",
            Self::ExtrudeCurve { .. } => "ExtrudeCrv",
            Self::ExtrudeCurveToPoint { .. } => "ExtrudeCrvToPoint",
            Self::Revolve { .. } => "Revolve",
        }
    }

    const fn prompt(self) -> &'static str {
        match self {
            Self::Point => "Point: pick a location in the viewport (Esc to cancel)",
            Self::Line { start: None } => {
                "Line: pick the start point in the viewport (Esc to cancel)"
            }
            Self::Line { start: Some(_) } => {
                "Line: pick the end point in the viewport (Esc to cancel)"
            }
            Self::Circle { center: None } => {
                "Circle: pick the center in the viewport (Esc to cancel)"
            }
            Self::Circle { center: Some(_) } => {
                "Circle: pick a point on the circle in the viewport (Esc to cancel)"
            }
            Self::Sphere { center: None } => {
                "Sphere: pick the center in the viewport (Esc to cancel)"
            }
            Self::Sphere { center: Some(_) } => {
                "Sphere: pick a point on the sphere in the viewport (Esc to cancel)"
            }
            Self::Ellipsoid { points } => match points {
                [None, _, _] => "Ellipsoid: pick the center in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "Ellipsoid: pick the end of the first axis in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "Ellipsoid: pick the second-axis radius in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Ellipsoid: pick the third-axis radius in the viewport (Esc to cancel)"
                }
            },
            Self::Arc { points } => match points {
                [None, _] => "Arc: pick the start point in the viewport (Esc to cancel)",
                [Some(_), None] => "Arc: pick a point on the arc in the viewport (Esc to cancel)",
                [Some(_), Some(_)] => "Arc: pick the end point in the viewport (Esc to cancel)",
            },
            Self::Ellipse { center: None, .. } => {
                "Ellipse: pick the center in the viewport (Esc to cancel)"
            }
            Self::Ellipse {
                center: Some(_),
                first_axis: None,
            } => "Ellipse: pick the end of the first axis in the viewport (Esc to cancel)",
            Self::Ellipse {
                first_axis: Some(_),
                ..
            } => "Ellipse: pick the second-axis radius in the viewport (Esc to cancel)",
            Self::Polyline => "Polyline: pick vertices; press Enter to finish (Esc to cancel)",
            Self::Curve { .. } => {
                "Curve: pick control points; press Enter to finish (Esc to cancel)"
            }
            Self::InterpCrv => {
                "InterpCrv: pick points on the curve; press Enter to finish (Esc to cancel)"
            }
            Self::Rectangle { first: None } => {
                "Rectangle: pick the first corner in the viewport (Esc to cancel)"
            }
            Self::Rectangle { first: Some(_) } => {
                "Rectangle: pick the opposite corner in the viewport (Esc to cancel)"
            }
            Self::Polygon { center: None, .. } => {
                "Polygon: pick the center in the viewport (Esc to cancel)"
            }
            Self::Polygon {
                center: Some(_), ..
            } => "Polygon: pick the first vertex in the viewport (Esc to cancel)",
            Self::SrfPt { corners } => match corners {
                [None, _, _] => "SrfPt: pick the first corner in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "SrfPt: pick the second corner in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "SrfPt: pick the third corner in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "SrfPt: pick the fourth corner in the viewport (Esc to cancel)"
                }
            },
            Self::Move { start: None } => {
                "Move: pick the base point in the viewport (Esc to cancel)"
            }
            Self::Move { start: Some(_) } => {
                "Move: pick the destination point in the viewport (Esc to cancel)"
            }
            Self::Copy { start: None } => {
                "Copy: pick the base point in the viewport (Esc to cancel)"
            }
            Self::Copy { start: Some(_) } => {
                "Copy: pick the destination point in the viewport (Esc to cancel)"
            }
            Self::ArrayLinear { start: None, .. } => {
                "ArrayLinear: pick the first reference point in the viewport (Esc to cancel)"
            }
            Self::ArrayLinear { start: Some(_), .. } => {
                "ArrayLinear: pick the spacing point in the viewport (Esc to cancel)"
            }
            Self::Array { start: None, .. } => {
                "Array: pick the first cell corner in the viewport (Esc to cancel)"
            }
            Self::Array {
                fill: false,
                start: Some(_),
                ..
            } => "Array: pick the opposite UnitCell corner in the viewport (Esc to cancel)",
            Self::Array {
                fill: true,
                start: Some(_),
                ..
            } => "Array: pick the opposite Fill corner in the viewport (Esc to cancel)",
            Self::ArrayPolar { .. } => {
                "ArrayPolar: pick the array center in the viewport (Esc to cancel)"
            }
            Self::Scale {
                kind, center: None, ..
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the center point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the origin in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the center point in the viewport (Esc to cancel)"
                }
            },
            Self::Scale {
                kind,
                center: Some(_),
                reference: None,
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the reference point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the reference point and direction (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the reference point in the viewport (Esc to cancel)"
                }
            },
            Self::Scale {
                kind,
                reference: Some(_),
                ..
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the target point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the target point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the target point in the viewport (Esc to cancel)"
                }
            },
            Self::Rotate { center: None, .. } => {
                "Rotate: pick the center point in the viewport (Esc to cancel)"
            }
            Self::Rotate {
                center: Some(_),
                reference: None,
            } => "Rotate: pick the reference point in the viewport (Esc to cancel)",
            Self::Rotate {
                reference: Some(_), ..
            } => "Rotate: pick the target point in the viewport (Esc to cancel)",
            Self::Rotate3D { points } => match points {
                [None, _, _] => "Rotate3D: pick the axis start in the viewport (Esc to cancel)",
                [Some(_), None, _] => "Rotate3D: pick the axis end in the viewport (Esc to cancel)",
                [Some(_), Some(_), None] => {
                    "Rotate3D: pick the reference point in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Rotate3D: pick the target point in the viewport (Esc to cancel)"
                }
            },
            Self::Mirror { start: None } => {
                "Mirror: pick the first axis point in the viewport (Esc to cancel)"
            }
            Self::Mirror { start: Some(_) } => {
                "Mirror: pick the second axis point in the viewport (Esc to cancel)"
            }
            Self::Shear { origin: None, .. } => {
                "Shear: pick the fixed origin in the viewport (Esc to cancel)"
            }
            Self::Shear {
                origin: Some(_),
                reference: None,
            } => "Shear: pick the reference direction in the viewport (Esc to cancel)",
            Self::Shear {
                reference: Some(_), ..
            } => "Shear: pick the target angle in the viewport (Esc to cancel)",
            Self::ExtrudeCurve { base: None, .. } => {
                "ExtrudeCrv: pick the direction base point in the viewport (Esc to cancel)"
            }
            Self::ExtrudeCurve { base: Some(_), .. } => {
                "ExtrudeCrv: pick the direction target point in the viewport (Esc to cancel)"
            }
            Self::ExtrudeCurveToPoint { .. } => {
                "ExtrudeCrvToPoint: pick the apex in the viewport (Esc to cancel)"
            }
            Self::Revolve {
                axis_start: None, ..
            } => "Revolve: pick the axis start in the viewport (Esc to cancel)",
            Self::Revolve {
                axis_start: Some(_),
                ..
            } => "Revolve: pick the axis end in the viewport (Esc to cancel)",
        }
    }

    const fn anchor(self) -> Option<Point3> {
        match self {
            Self::Point
            | Self::Line { start: None }
            | Self::Circle { center: None }
            | Self::Sphere { center: None }
            | Self::Ellipsoid {
                points: [None, _, _],
            }
            | Self::Arc { points: [None, _] }
            | Self::Ellipse { center: None, .. }
            | Self::Polyline
            | Self::Curve { .. }
            | Self::InterpCrv
            | Self::Rectangle { first: None }
            | Self::Polygon { center: None, .. }
            | Self::SrfPt {
                corners: [None, _, _],
            }
            | Self::Move { start: None }
            | Self::Copy { start: None }
            | Self::ArrayLinear { start: None, .. }
            | Self::Array { start: None, .. }
            | Self::ArrayPolar { .. }
            | Self::Scale { center: None, .. }
            | Self::Rotate { center: None, .. }
            | Self::Rotate3D {
                points: [None, _, _],
            }
            | Self::Mirror { start: None }
            | Self::Shear { origin: None, .. }
            | Self::ExtrudeCurve { base: None, .. }
            | Self::ExtrudeCurveToPoint { .. }
            | Self::Revolve {
                axis_start: None, ..
            } => None,
            Self::Line { start }
            | Self::Circle { center: start }
            | Self::Sphere { center: start }
            | Self::Rectangle { first: start }
            | Self::Move { start }
            | Self::Copy { start }
            | Self::ArrayLinear { start, .. }
            | Self::Array { start, .. }
            | Self::Mirror { start }
            | Self::ExtrudeCurve { base: start, .. }
            | Self::Revolve {
                axis_start: start, ..
            } => start,
            Self::Ellipse { center, .. }
            | Self::Polygon { center, .. }
            | Self::Ellipsoid {
                points: [center, _, _],
            } => center,
            Self::Arc {
                points: [_, Some(point)],
            }
            | Self::Arc {
                points: [Some(point), None],
            } => Some(point),
            Self::SrfPt {
                corners: [_, _, Some(corner)],
            }
            | Self::SrfPt {
                corners: [_, Some(corner), None],
            }
            | Self::SrfPt {
                corners: [Some(corner), None, None],
            } => Some(corner),
            Self::Scale {
                center: Some(center),
                ..
            }
            | Self::Rotate {
                center: Some(center),
                ..
            }
            | Self::Shear {
                origin: Some(center),
                ..
            } => Some(center),
            Self::Rotate3D {
                points: [Some(start), _, _],
            } => Some(start),
        }
    }

    const fn reference(self) -> Option<Point3> {
        match self {
            Self::Scale { reference, .. }
            | Self::Rotate { reference, .. }
            | Self::Shear { reference, .. } => reference,
            Self::Rotate3D {
                points: [_, _, reference],
            } => reference,
            Self::Ellipse { first_axis, .. } => first_axis,
            Self::Ellipsoid { points } => match points {
                [_, _, Some(second_axis)] => Some(second_axis),
                [_, Some(first_axis), None] => Some(first_axis),
                _ => None,
            },
            _ => None,
        }
    }

    const fn collects_curve_points(self) -> bool {
        matches!(self, Self::Polyline | Self::Curve { .. } | Self::InterpCrv)
    }
}

pub struct VibocerosApp {
    document: Document,
    commands: CommandRegistry,
    command_input: String,
    command_log: VecDeque<String>,
    viewport: Viewport,
    osnap: bool,
    smart_track: bool,
    active_command: Option<InteractiveCommand>,
    curve_points: Vec<Point3>,
    sidebar: DocumentSidebar,
}

impl VibocerosApp {
    pub fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        creation_context
            .egui_ctx
            .set_visuals(egui::Visuals::light());
        let mut command_log = VecDeque::new();
        command_log.push_back("Viboceros ready — enter Help for commands.".to_owned());
        Self {
            document: Document::default(),
            commands: CommandRegistry::with_builtins(),
            command_input: String::new(),
            command_log,
            viewport: Viewport::default(),
            osnap: true,
            smart_track: true,
            active_command: None,
            curve_points: Vec::new(),
            sidebar: DocumentSidebar::default(),
        }
    }

    fn run_command(&mut self) {
        let input = self.command_input.trim().to_owned();
        if input.is_empty() {
            if self
                .active_command
                .is_some_and(InteractiveCommand::collects_curve_points)
            {
                self.finish_interactive_curve();
            }
            return;
        }
        self.command_input.clear();
        if self.try_start_interactive_command(&input) {
            return;
        }
        self.execute_command(&input);
    }

    fn execute_command(&mut self, input: &str) {
        self.cancel_interactive_command(false);
        self.push_log(format!("> {input}"));
        match self.commands.execute(&mut self.document, input) {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn push_log(&mut self, message: String) {
        if self.command_log.len() == MAX_LOG_ENTRIES {
            self.command_log.pop_front();
        }
        self.command_log.push_back(message);
    }

    fn try_start_interactive_command(&mut self, input: &str) -> bool {
        let mut tokens = input.split_whitespace();
        let Some(name) = tokens.next() else {
            return false;
        };
        let arguments = tokens.collect::<Vec<_>>();
        let normalized = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        let command = if normalized == "curve" {
            let mut degree = 3;
            let mut closure = ControlPointCurveClosure::Open;
            let mut degree_seen = false;
            let mut close_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Degree") && !degree_seen {
                    let Ok(parsed) = value.parse::<usize>() else {
                        return false;
                    };
                    degree = parsed.clamp(1, MAX_CURVE_COMMAND_DEGREE);
                    degree_seen = true;
                } else if name.eq_ignore_ascii_case("Close") && !close_seen {
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
                            return false;
                        };
                    close_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::Curve { degree, closure }
        } else if normalized == "extrudecrv" {
            let mut both_sides = false;
            let mut delete_input = false;
            let mut both_sides_seen = false;
            let mut delete_input_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let parsed = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    return false;
                };
                if name.eq_ignore_ascii_case("BothSides") && !both_sides_seen {
                    both_sides = parsed;
                    both_sides_seen = true;
                } else if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = parsed;
                    delete_input_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides,
                delete_input,
            }
        } else if normalized == "extrudecrvtopoint" {
            let mut delete_input = false;
            let mut solid = false;
            let mut delete_input_seen = false;
            let mut solid_seen = false;
            let mut output_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                let yes_no = if value.eq_ignore_ascii_case("Yes") {
                    true
                } else if value.eq_ignore_ascii_case("No") {
                    false
                } else {
                    if name.eq_ignore_ascii_case("Output")
                        && !output_seen
                        && value.eq_ignore_ascii_case("Surface")
                    {
                        output_seen = true;
                        continue;
                    }
                    return false;
                };
                if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = yes_no;
                    delete_input_seen = true;
                } else if name.eq_ignore_ascii_case("Solid") && !solid_seen {
                    solid = yes_no;
                    solid_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ExtrudeCurveToPoint {
                delete_input,
                solid,
            }
        } else if normalized == "revolve" {
            let mut start_angle_degrees = 0.0;
            let mut sweep_degrees = 360.0;
            let mut delete_input = false;
            let mut start_angle_seen = false;
            let mut sweep_seen = false;
            let mut delete_input_seen = false;
            for option in arguments {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches(['_', '-']);
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("StartAngle") && !start_angle_seen {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() {
                        return false;
                    }
                    start_angle_degrees = angle;
                    start_angle_seen = true;
                } else if matches!(name.to_ascii_lowercase().as_str(), "angle" | "sweepangle")
                    && !sweep_seen
                {
                    let Ok(angle) = value.parse::<f64>() else {
                        return false;
                    };
                    if !angle.is_finite() || angle == 0.0 || angle.abs() > 360.0 {
                        return false;
                    }
                    sweep_degrees = angle;
                    sweep_seen = true;
                } else if name.eq_ignore_ascii_case("DeleteInput") && !delete_input_seen {
                    delete_input = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return false;
                    };
                    delete_input_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            }
        } else if matches!(normalized.as_str(), "polygon" | "poly") {
            let side_count = match arguments.as_slice() {
                [] => 4,
                [text] => match text.parse::<usize>() {
                    Ok(side_count) if (3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) => {
                        side_count
                    }
                    _ => return false,
                },
                _ => return false,
            };
            InteractiveCommand::Polygon {
                side_count,
                center: None,
            }
        } else if matches!(normalized.as_str(), "array" | "arrayrectangular") {
            if arguments.len() < 2 {
                return false;
            }
            let Ok(x_count) = arguments[0].parse::<usize>() else {
                return false;
            };
            let Ok(y_count) = arguments[1].parse::<usize>() else {
                return false;
            };
            if x_count == 0 || y_count == 0 {
                return false;
            }
            let mut z_count = 1;
            let mut remaining = &arguments[2..];
            if let Some(z_count_text) = remaining.first()
                && !z_count_text.contains('=')
            {
                let Ok(parsed) = z_count_text.parse::<usize>() else {
                    return false;
                };
                if parsed == 0 {
                    return false;
                }
                z_count = parsed;
                remaining = &remaining[1..];
            }
            let mut fill = false;
            let mut z_distance = 0.0;
            let mut mode_seen = false;
            let mut z_distance_seen = false;
            for option in remaining {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches('_');
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Mode") && !mode_seen {
                    if value.eq_ignore_ascii_case("UnitCell") {
                        fill = false;
                    } else if value.eq_ignore_ascii_case("Fill") {
                        fill = true;
                    } else {
                        return false;
                    }
                    mode_seen = true;
                } else if name.eq_ignore_ascii_case("ZDistance") && !z_distance_seen {
                    let Ok(distance) = value.parse::<f64>() else {
                        return false;
                    };
                    if !distance.is_finite() {
                        return false;
                    }
                    z_distance = distance;
                    z_distance_seen = true;
                } else {
                    return false;
                }
            }
            if z_count > 1 && !z_distance_seen {
                return false;
            }
            InteractiveCommand::Array {
                counts: [x_count, y_count, z_count],
                fill,
                z_distance,
                start: None,
            }
        } else if normalized == "arraylinear" {
            let [item_count] = arguments.as_slice() else {
                return false;
            };
            let Ok(item_count) = item_count.parse::<usize>() else {
                return false;
            };
            if item_count < 2 {
                return false;
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: None,
            }
        } else if normalized == "arraypolar" {
            let Some(item_count_text) = arguments.first() else {
                return false;
            };
            let Ok(item_count) = item_count_text.parse::<usize>() else {
                return false;
            };
            if item_count < 2 {
                return false;
            }
            let mut fill_angle_degrees = 360.0;
            let mut rotate = true;
            let mut z_offset = 0.0;
            let mut rotate_seen = false;
            let mut z_offset_seen = false;
            let mut remaining = &arguments[1..];
            if let Some(angle_text) = remaining.first()
                && !angle_text.contains('=')
            {
                let Ok(angle) = angle_text.parse::<f64>() else {
                    return false;
                };
                if !angle.is_finite() || angle == 0.0 {
                    return false;
                }
                fill_angle_degrees = angle;
                remaining = &remaining[1..];
            }
            for option in remaining {
                let Some((name, value)) = option.split_once('=') else {
                    return false;
                };
                let name = name.trim_start_matches('_');
                let value = value.trim_start_matches('_');
                if name.eq_ignore_ascii_case("Rotate") && !rotate_seen {
                    if value.eq_ignore_ascii_case("Yes") {
                        rotate = true;
                    } else if value.eq_ignore_ascii_case("No") {
                        rotate = false;
                    } else {
                        return false;
                    }
                    rotate_seen = true;
                } else if name.eq_ignore_ascii_case("ZOffset") && !z_offset_seen {
                    let Ok(offset) = value.parse::<f64>() else {
                        return false;
                    };
                    if !offset.is_finite() {
                        return false;
                    }
                    z_offset = offset;
                    z_offset_seen = true;
                } else {
                    return false;
                }
            }
            InteractiveCommand::ArrayPolar {
                item_count,
                fill_angle_degrees,
                rotate,
                z_offset,
            }
        } else {
            if !arguments.is_empty() {
                return false;
            }
            match normalized.as_str() {
                "point" | "pt" => InteractiveCommand::Point,
                "line" | "l" => InteractiveCommand::Line { start: None },
                "circle" | "c" => InteractiveCommand::Circle { center: None },
                "sphere" | "sph" => InteractiveCommand::Sphere { center: None },
                "ellipsoid" => InteractiveCommand::Ellipsoid { points: [None; 3] },
                "arc" | "a" => InteractiveCommand::Arc { points: [None; 2] },
                "ellipse" | "ell" => InteractiveCommand::Ellipse {
                    center: None,
                    first_axis: None,
                },
                "polyline" | "pline" => InteractiveCommand::Polyline,
                "interpcrv" | "interpcurve" => InteractiveCommand::InterpCrv,
                "rectangle" | "rect" => InteractiveCommand::Rectangle { first: None },
                "srfpt" | "surfacefromcorners" => InteractiveCommand::SrfPt { corners: [None; 3] },
                "move" | "m" => InteractiveCommand::Move { start: None },
                "copy" => InteractiveCommand::Copy { start: None },
                "scale" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::Uniform,
                    center: None,
                    reference: None,
                },
                "scale1d" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::OneDimensional,
                    center: None,
                    reference: None,
                },
                "scale2d" => InteractiveCommand::Scale {
                    kind: InteractiveScaleKind::TwoDimensional,
                    center: None,
                    reference: None,
                },
                "rotate" => InteractiveCommand::Rotate {
                    center: None,
                    reference: None,
                },
                "rotate3d" => InteractiveCommand::Rotate3D { points: [None; 3] },
                "mirror" => InteractiveCommand::Mirror { start: None },
                "shear" => InteractiveCommand::Shear {
                    origin: None,
                    reference: None,
                },
                _ => return false,
            }
        };

        self.cancel_interactive_command(true);
        self.push_log(format!("> {input}"));
        if matches!(
            command,
            InteractiveCommand::Move { .. }
                | InteractiveCommand::Copy { .. }
                | InteractiveCommand::Array { .. }
                | InteractiveCommand::ArrayLinear { .. }
                | InteractiveCommand::ArrayPolar { .. }
                | InteractiveCommand::Scale { .. }
                | InteractiveCommand::Rotate { .. }
                | InteractiveCommand::Rotate3D { .. }
                | InteractiveCommand::Mirror { .. }
                | InteractiveCommand::Shear { .. }
                | InteractiveCommand::ExtrudeCurve { .. }
                | InteractiveCommand::ExtrudeCurveToPoint { .. }
                | InteractiveCommand::Revolve { .. }
        ) && self.document.selected_object_count() == 0
        {
            self.push_log("Error: no objects are selected".to_owned());
            return true;
        }
        self.push_log(command.prompt().to_owned());
        self.active_command = Some(command);
        true
    }

    fn cancel_interactive_command(&mut self, announce: bool) {
        let command = self.active_command.take();
        self.curve_points.clear();
        if let Some(command) = command
            && announce
        {
            self.push_log(format!("Cancelled {}", command.name()));
        }
    }

    fn accept_drafting_point(&mut self, point: Point3) {
        let Some(command) = self.active_command else {
            return;
        };
        match command {
            InteractiveCommand::Point => {
                self.active_command = None;
                self.execute_command(&format!("Point {}", format_model_point(point)));
            }
            InteractiveCommand::Line { start: None } => {
                self.active_command = Some(InteractiveCommand::Line { start: Some(point) });
                self.push_log(format!("Start: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Line { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Line { start: Some(start) } => {
                if start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: line end must differ from its start".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Line {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Circle { center: None } => {
                let command = InteractiveCommand::Circle {
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Circle {
                center: Some(center),
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log("Error: circle point must differ from its center".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Circle {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Sphere { center: None } => {
                let command = InteractiveCommand::Sphere {
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Sphere {
                center: Some(center),
            } => {
                if center.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: sphere point must differ from its center".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Sphere {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Ellipsoid { mut points } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1 {
                    let Some(center) = points[0] else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    if center.is_near(point, self.document.tolerance()) {
                        self.push_log(
                            "Error: ellipsoid axis point must differ from its center".to_owned(),
                        );
                        return;
                    }
                } else if point_count == 2 {
                    let [Some(center), Some(first_axis), _] = points else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    if let Err(error) = Frame3::try_from_points(
                        center,
                        first_axis,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return;
                    }
                }

                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Ellipsoid { points };
                    self.active_command = Some(command);
                    let label = ["Center", "First axis", "Second axis"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(center), Some(first_axis), Some(second_axis)] = points else {
                        self.push_log("Error: ellipsoid point state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    if center.is_near(point, self.document.tolerance()) {
                        self.push_log(
                            "Error: ellipsoid third-axis radius must be positive".to_owned(),
                        );
                        return;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Ellipsoid {} {} {} {}",
                        format_model_point(center),
                        format_model_point(first_axis),
                        format_model_point(second_axis),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Arc { mut points } => {
                let point_count = points.iter().flatten().count();
                if let Some(previous) = points.iter().flatten().next_back()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: consecutive arc points must differ".to_owned());
                    return;
                }
                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Arc { points };
                    self.active_command = Some(command);
                    let label = if point_count == 0 { "Start" } else { "Through" };
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(start), Some(through)] = points else {
                        self.push_log("Error: arc point state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    if let Err(error) = CircularArc3::try_from_three_points(
                        start,
                        through,
                        point,
                        self.document.tolerance(),
                    ) {
                        self.push_log(format!("Error: {error}"));
                        return;
                    }
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Arc {} {} {}",
                        format_model_point(start),
                        format_model_point(through),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Ellipse { center: None, .. } => {
                let command = InteractiveCommand::Ellipse {
                    center: Some(point),
                    first_axis: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: None,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log(
                        "Error: ellipse axis point must differ from its center".to_owned(),
                    );
                    return;
                }
                let command = InteractiveCommand::Ellipse {
                    center: Some(center),
                    first_axis: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First axis: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: Some(first_axis),
            } => {
                if let Err(error) = Ellipse3::try_from_three_points(
                    center,
                    first_axis,
                    point,
                    self.document.tolerance(),
                ) {
                    self.push_log(format!("Error: {error}"));
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Ellipse {} {} {}",
                    format_model_point(center),
                    format_model_point(first_axis),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Polyline => {
                if let Some(previous) = self.curve_points.last()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: adjacent polyline vertices must differ".to_owned());
                    return;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Vertex {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Curve { .. } => {
                if let Some(previous) = self.curve_points.last()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: adjacent curve control points must differ".to_owned());
                    return;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Control point {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::InterpCrv => {
                if let Some(previous) = self.curve_points.last()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log(
                        "Error: adjacent curve interpolation points must differ".to_owned(),
                    );
                    return;
                }
                self.curve_points.push(point);
                self.push_log(format!(
                    "Point {}: {}",
                    self.curve_points.len(),
                    format_model_point(point)
                ));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rectangle { first: None } => {
                let command = InteractiveCommand::Rectangle { first: Some(point) };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rectangle { first: Some(first) } => {
                let tolerance = self.document.tolerance().absolute();
                if (first.x() - point.x()).abs() <= tolerance
                    || (first.y() - point.y()).abs() <= tolerance
                {
                    self.push_log(
                        "Error: rectangle width and height must both exceed model tolerance"
                            .to_owned(),
                    );
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Rectangle {} {}",
                    format_model_point(first),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Polygon {
                side_count,
                center: None,
            } => {
                let command = InteractiveCommand::Polygon {
                    side_count,
                    center: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Polygon {
                side_count,
                center: Some(center),
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log("Error: polygon vertex must differ from its center".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Polygon {side_count} {} {}",
                    format_model_point(center),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::SrfPt { mut corners } => {
                let corner_count = corners.iter().flatten().count();
                if let Some(previous) = corners.iter().flatten().next_back()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: adjacent surface corners must differ".to_owned());
                    return;
                }
                if corner_count < corners.len() {
                    corners[corner_count] = Some(point);
                    let command = InteractiveCommand::SrfPt { corners };
                    self.active_command = Some(command);
                    self.push_log(format!(
                        "Corner {}: {}",
                        corner_count + 1,
                        format_model_point(point)
                    ));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(first), Some(second), Some(third)] = corners else {
                        self.push_log("Error: surface corner state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "SrfPt {} {} {} {}",
                        format_model_point(first),
                        format_model_point(second),
                        format_model_point(third),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Move { start: None } => {
                self.active_command = Some(InteractiveCommand::Move { start: Some(point) });
                self.push_log(format!("Base: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Move { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Copy { start: None } => {
                self.active_command = Some(InteractiveCommand::Copy { start: Some(point) });
                self.push_log(format!("Base: {}", format_model_point(point)));
                self.push_log(
                    InteractiveCommand::Copy { start: Some(point) }
                        .prompt()
                        .to_owned(),
                );
            }
            InteractiveCommand::Move { start: Some(start) } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Move {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Copy { start: Some(start) } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Copy {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Array {
                counts,
                fill,
                z_distance,
                start: None,
            } => {
                let command = InteractiveCommand::Array {
                    counts,
                    fill,
                    z_distance,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First corner: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Array {
                counts,
                fill,
                z_distance,
                start: Some(start),
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "Array {} {} {} {} {} {} Mode={}",
                    counts[0],
                    counts[1],
                    counts[2],
                    point.x() - start.x(),
                    point.y() - start.y(),
                    z_distance,
                    if fill { "Fill" } else { "UnitCell" }
                ));
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: None,
            } => {
                let command = InteractiveCommand::ArrayLinear {
                    item_count,
                    start: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("First reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ArrayLinear {
                item_count,
                start: Some(start),
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ArrayLinear {item_count} {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ArrayPolar {
                item_count,
                fill_angle_degrees,
                rotate,
                z_offset,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ArrayPolar {item_count} {} {fill_angle_degrees} Rotate={} ZOffset={z_offset}",
                    format_model_point(point),
                    if rotate { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::Scale {
                kind, center: None, ..
            } => {
                let command = InteractiveCommand::Scale {
                    kind,
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: None,
            } => {
                if center.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: scale reference must differ from its center".to_owned());
                    return;
                }
                let command = InteractiveCommand::Scale {
                    kind,
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: Some(reference),
            } => {
                if kind != InteractiveScaleKind::OneDimensional
                    && center.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: scale target must differ from its center".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "{} {} {} {}",
                    kind.name(),
                    format_model_point(center),
                    format_model_point(reference),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Rotate { center: None, .. } => {
                let command = InteractiveCommand::Rotate {
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: None,
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log("Error: rotate reference must differ from its center".to_owned());
                    return;
                }
                let command = InteractiveCommand::Rotate {
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: Some(reference),
            } => {
                if same_top_point(center, point, self.document.tolerance()) {
                    self.push_log("Error: rotate target must differ from its center".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Rotate {} {} {}",
                    format_model_point(center),
                    format_model_point(reference),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Rotate3D { mut points } => {
                let point_count = points.iter().flatten().count();
                if point_count == 1
                    && points[0]
                        .is_some_and(|start| start.is_near(point, self.document.tolerance()))
                {
                    self.push_log("Error: rotation axis points must differ".to_owned());
                    return;
                }
                if point_count >= 2 {
                    let [Some(axis_start), Some(axis_end), _] = points else {
                        self.push_log("Error: Rotate3D point state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    if point_is_near_axis(axis_start, axis_end, point, self.document.tolerance()) {
                        self.push_log(
                            "Error: Rotate3D reference points must lie off the axis".to_owned(),
                        );
                        return;
                    }
                }
                if point_count < points.len() {
                    points[point_count] = Some(point);
                    let command = InteractiveCommand::Rotate3D { points };
                    self.active_command = Some(command);
                    let label = ["Axis start", "Axis end", "Reference"][point_count];
                    self.push_log(format!("{label}: {}", format_model_point(point)));
                    self.push_log(command.prompt().to_owned());
                } else {
                    let [Some(axis_start), Some(axis_end), Some(reference)] = points else {
                        self.push_log("Error: Rotate3D point state is inconsistent".to_owned());
                        self.active_command = None;
                        return;
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "Rotate3D {} {} {} {}",
                        format_model_point(axis_start),
                        format_model_point(axis_end),
                        format_model_point(reference),
                        format_model_point(point)
                    ));
                }
            }
            InteractiveCommand::Mirror { start: None } => {
                let command = InteractiveCommand::Mirror { start: Some(point) };
                self.active_command = Some(command);
                self.push_log(format!("Axis start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Mirror { start: Some(start) } => {
                if same_top_point(start, point, self.document.tolerance()) {
                    self.push_log("Error: mirror axis points must differ".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Mirror {} {}",
                    format_model_point(start),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::Shear { origin: None, .. } => {
                let command = InteractiveCommand::Shear {
                    origin: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Origin: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: None,
            } => {
                if same_top_point(origin, point, self.document.tolerance()) {
                    self.push_log("Error: shear reference must differ from its origin".to_owned());
                    return;
                }
                let command = InteractiveCommand::Shear {
                    origin: Some(origin),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: Some(reference),
            } => {
                if same_top_point(origin, point, self.document.tolerance()) {
                    self.push_log("Error: shear target must differ from its origin".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Shear {} {} {}",
                    format_model_point(origin),
                    format_model_point(reference),
                    format_model_point(point)
                ));
            }
            InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides,
                delete_input,
            } => {
                let command = InteractiveCommand::ExtrudeCurve {
                    base: Some(point),
                    both_sides,
                    delete_input,
                };
                self.active_command = Some(command);
                self.push_log(format!("Direction base: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::ExtrudeCurve {
                base: Some(base),
                both_sides,
                delete_input,
            } => {
                if base.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: extrusion direction points must differ".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtrudeCrv {} {} BothSides={} DeleteInput={}",
                    format_model_point(base),
                    format_model_point(point),
                    if both_sides { "Yes" } else { "No" },
                    if delete_input { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::ExtrudeCurveToPoint {
                delete_input,
                solid,
            } => {
                self.active_command = None;
                self.execute_command(&format!(
                    "ExtrudeCrvToPoint {} DeleteInput={} Solid={}",
                    format_model_point(point),
                    if delete_input { "Yes" } else { "No" },
                    if solid { "Yes" } else { "No" }
                ));
            }
            InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            } => {
                let command = InteractiveCommand::Revolve {
                    axis_start: Some(point),
                    start_angle_degrees,
                    sweep_degrees,
                    delete_input,
                };
                self.active_command = Some(command);
                self.push_log(format!("Axis start: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Revolve {
                axis_start: Some(axis_start),
                start_angle_degrees,
                sweep_degrees,
                delete_input,
            } => {
                if axis_start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: revolve axis points must differ".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Revolve {} {} {} StartAngle={} DeleteInput={}",
                    format_model_point(axis_start),
                    format_model_point(point),
                    sweep_degrees,
                    start_angle_degrees,
                    if delete_input { "Yes" } else { "No" }
                ));
            }
        }
    }

    fn finish_interactive_curve(&mut self) {
        let Some(command) = self
            .active_command
            .filter(|command| command.collects_curve_points())
        else {
            return;
        };
        let minimum_point_count = match command {
            InteractiveCommand::Curve {
                closure: ControlPointCurveClosure::Smooth | ControlPointCurveClosure::Sharp,
                ..
            } => 3,
            _ => 2,
        };
        if self.curve_points.len() < minimum_point_count {
            self.push_log(format!(
                "Error: {} requires at least {minimum_point_count} points",
                command.name(),
            ));
            return;
        }
        let points = std::mem::take(&mut self.curve_points);
        self.active_command = None;
        let arguments = points
            .into_iter()
            .map(format_model_point)
            .collect::<Vec<_>>()
            .join(" ");
        let input = match command {
            InteractiveCommand::Curve { degree, closure } => {
                let closure = match closure {
                    ControlPointCurveClosure::Open => "Open",
                    ControlPointCurveClosure::Smooth => "Smooth",
                    ControlPointCurveClosure::Sharp => "Sharp",
                };
                format!("Curve {arguments} Degree={degree} Close={closure}")
            }
            _ => format!("{} {arguments}", command.name()),
        };
        self.execute_command(&input);
    }

    fn apply_selection_click(&mut self, click: SelectionClick) {
        match click.object_id {
            Some(id) => match self.document.select_object(id, click.mode) {
                Ok(count) => self.push_log(format!("Selected {count} object(s)")),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            None if click.mode == viboceros_document::SelectionMode::Replace => {
                let count = self.document.clear_selection();
                if count > 0 {
                    self.push_log(format!("Deselected {count} object(s)"));
                }
            }
            None => {}
        }
    }

    fn show_toolbar(&mut self, root: &mut egui::Ui) {
        let can_undo = self.document.can_undo();
        let can_redo = self.document.can_redo();
        let undo_tooltip = self.document.undo_label().map_or_else(
            || "Nothing to undo".to_owned(),
            |label| format!("Undo {label}"),
        );
        let redo_tooltip = self.document.redo_label().map_or_else(
            || "Nothing to redo".to_owned(),
            |label| format!("Redo {label}"),
        );
        let mut undo_clicked = false;
        let mut redo_clicked = false;
        let mut select_all_clicked = false;
        let mut select_none_clicked = false;
        let mut select_last_clicked = false;
        let mut select_previous_clicked = false;
        let mut delete_clicked = false;
        let mut hide_clicked = false;
        let mut show_clicked = false;
        let mut lock_clicked = false;
        let mut unlock_clicked = false;
        let mut hide_swap_clicked = false;
        let mut lock_swap_clicked = false;
        let mut isolate_clicked = false;
        let mut unisolate_clicked = false;
        let mut isolate_lock_clicked = false;
        let mut unisolate_lock_clicked = false;
        let mut join_clicked = false;
        let mut explode_clicked = false;
        let mut flip_clicked = false;
        let mut unify_mesh_normals_clicked = false;
        let mut combine_mesh_vertices_clicked = false;
        let mut cull_unused_mesh_vertices_clicked = false;
        let mut split_disjoint_mesh_clicked = false;
        let mut extract_non_manifold_clicked = false;
        let mut extract_duplicate_faces_clicked = false;
        let mut close_curve_clicked = false;
        let mut curve_start_clicked = false;
        let mut curve_end_clicked = false;
        let mut extract_points_clicked = false;
        let mut length_clicked = false;
        let mut area_clicked = false;
        let mut volume_clicked = false;
        let mut bounding_box_clicked = false;
        let mut move_clicked = false;
        let mut copy_clicked = false;
        let mut scale_clicked = false;
        let mut rotate_clicked = false;
        let mut mirror_clicked = false;
        let mut shear_clicked = false;
        let mut project_to_cplane_clicked = false;
        let mut to_nurbs_clicked = false;
        let mut planar_surface_clicked = false;
        let mut sphere_clicked = false;
        let mut ellipsoid_clicked = false;
        let mut extrude_curve_clicked = false;
        let mut extrude_curve_to_point_clicked = false;
        let mut extrude_curve_along_curve_clicked = false;
        let mut revolve_clicked = false;
        let selected = self.document.selected_object_count();
        let selectable_last_changed = self.document.selectable_last_changed_object_count();
        let selectable_previous = self.document.selectable_previous_object_count();
        let swappable_layers = self
            .document
            .layers()
            .filter(|layer| layer.is_visible() && !layer.is_locked())
            .map(|layer| layer.id())
            .collect::<BTreeSet<_>>();
        let (hidden, locked, hide_swappable, lock_swappable) = self.document.objects().fold(
            (0, 0, 0, 0),
            |(hidden, locked, hide_swappable, lock_swappable), object| {
                let attributes = object.attributes();
                let layer_is_eligible = swappable_layers.contains(&attributes.layer_id());
                (
                    hidden + usize::from(!attributes.is_visible()),
                    locked + usize::from(attributes.is_locked()),
                    hide_swappable + usize::from(layer_is_eligible && !attributes.is_locked()),
                    lock_swappable + usize::from(layer_is_eligible && attributes.is_visible()),
                )
            },
        );
        let isolated_hidden = self.document.isolated_hidden_object_count();
        let isolated_locked = self.document.isolated_locked_object_count();
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Viboceros");
                ui.separator();
                undo_clicked = ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .on_hover_text(undo_tooltip)
                    .clicked();
                redo_clicked = ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .on_hover_text(redo_tooltip)
                    .clicked();
                ui.separator();
                select_all_clicked = ui
                    .button("Select All")
                    .on_hover_text("Select every visible, unlocked object")
                    .clicked();
                select_none_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Clear Selection"))
                    .clicked();
                select_last_clicked = ui
                    .add_enabled(selectable_last_changed > 0, egui::Button::new("Select Last"))
                    .on_hover_text("Replace the selection with the last changed objects")
                    .clicked();
                select_previous_clicked = ui
                    .add_enabled(selectable_previous > 0, egui::Button::new("Select Previous"))
                    .on_hover_text("Swap the current and previous selections")
                    .clicked();
                delete_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Delete"))
                    .on_hover_text("Delete selected objects")
                    .clicked();
                hide_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Hide"))
                    .on_hover_text("Hide selected objects")
                    .clicked();
                show_clicked = ui
                    .add_enabled(hidden > 0, egui::Button::new("Show"))
                    .on_hover_text("Show all hidden objects")
                    .clicked();
                lock_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Lock"))
                    .on_hover_text("Lock selected objects while keeping them available to osnap")
                    .clicked();
                unlock_clicked = ui
                    .add_enabled(locked > 0, egui::Button::new("Unlock"))
                    .on_hover_text("Unlock all object-level locks")
                    .clicked();
                hide_swap_clicked = ui
                    .add_enabled(hide_swappable > 0, egui::Button::new("Swap Hidden"))
                    .on_hover_text("Swap normal and hidden objects on visible, unlocked layers")
                    .clicked();
                lock_swap_clicked = ui
                    .add_enabled(lock_swappable > 0, egui::Button::new("Swap Locked"))
                    .on_hover_text("Swap normal and locked objects on visible, unlocked layers")
                    .clicked();
                isolate_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Isolate"))
                    .on_hover_text("Hide ordinary objects outside the selection")
                    .clicked();
                unisolate_clicked = ui
                    .add_enabled(isolated_hidden > 0, egui::Button::new("Unisolate"))
                    .on_hover_text("Show only objects hidden by Isolate")
                    .clicked();
                isolate_lock_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Isolate Lock"))
                    .on_hover_text("Lock ordinary objects outside the selection")
                    .clicked();
                unisolate_lock_clicked = ui
                    .add_enabled(isolated_locked > 0, egui::Button::new("Unisolate Lock"))
                    .on_hover_text("Unlock only objects locked by Isolate Lock")
                    .clicked();
                join_clicked = ui
                    .add_enabled(selected > 1, egui::Button::new("Join"))
                    .on_hover_text("Join selected endpoint-connected lines and polylines")
                    .clicked();
                explode_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Explode"))
                    .on_hover_text("Explode selected polylines into line segments")
                    .clicked();
                flip_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Flip"))
                    .on_hover_text("Flip selected curve directions or mesh face windings")
                    .clicked();
                unify_mesh_normals_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Unify Normals"))
                    .on_hover_text("Make selected mesh face windings consistent")
                    .clicked();
                combine_mesh_vertices_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Combine Vertices"))
                    .on_hover_text("Merge exactly identical selected-mesh vertices")
                    .clicked();
                cull_unused_mesh_vertices_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Cull Unused"))
                    .on_hover_text("Remove unreferenced vertices from selected meshes")
                    .clicked();
                split_disjoint_mesh_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Split Pieces"))
                    .on_hover_text("Separate edge-disconnected parts of selected meshes")
                    .clicked();
                extract_non_manifold_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Extract Non-Manifold"))
                    .on_hover_text("Extract faces around non-manifold mesh edges")
                    .clicked();
                extract_duplicate_faces_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Extract Duplicates"))
                    .on_hover_text("Extract duplicate faces from selected meshes")
                    .clicked();
                close_curve_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Close"))
                    .on_hover_text("Close selected polylines")
                    .clicked();
                curve_start_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Start Pt"))
                    .on_hover_text("Place points at selected curve starts")
                    .clicked();
                curve_end_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("End Pt"))
                    .on_hover_text("Place points at selected curve ends")
                    .clicked();
                extract_points_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Extract Pts"))
                    .on_hover_text("Duplicate defining points and raw mesh vertices")
                    .clicked();
                length_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Length"))
                    .on_hover_text("Measure selected curve length")
                    .clicked();
                area_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Area"))
                    .on_hover_text("Measure selected planar, surface, B-rep, or mesh area")
                    .clicked();
                volume_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Volume"))
                    .on_hover_text("Measure signed volume of selected closed meshes or B-reps")
                    .clicked();
                bounding_box_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Bounding Box"))
                    .on_hover_text("Create one World-coordinate enclosure for the selection")
                    .clicked();
                move_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Move"))
                    .on_hover_text("Move selected objects using two viewport points")
                    .clicked();
                copy_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Copy"))
                    .on_hover_text("Copy selected objects using two viewport points")
                    .clicked();
                scale_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Scale"))
                    .on_hover_text("Scale selected objects using three viewport points")
                    .clicked();
                rotate_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Rotate"))
                    .on_hover_text("Rotate selected objects using three viewport points")
                    .clicked();
                mirror_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Mirror"))
                    .on_hover_text("Mirror selected objects across a two-point top-view axis")
                    .clicked();
                shear_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Shear"))
                    .on_hover_text("Shear selected objects using three top-view points")
                    .clicked();
                project_to_cplane_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Project CPlane"))
                    .on_hover_text(
                        "Create flattened copies of selected objects on the construction plane",
                    )
                    .clicked();
                to_nurbs_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("To NURBS"))
                    .on_hover_text("Create exact NURBS copies of supported selected curves")
                    .clicked();
                planar_surface_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Planar Surface"))
                    .on_hover_text("Create exact trimmed planar faces from selected closed curves")
                    .clicked();
                sphere_clicked = ui
                    .button("Sphere")
                    .on_hover_text("Create an exact NURBS sphere from two viewport points")
                    .clicked();
                ellipsoid_clicked = ui
                    .button("Ellipsoid")
                    .on_hover_text("Create an exact NURBS ellipsoid from four viewport points")
                    .clicked();
                extrude_curve_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Extrude Curve"))
                    .on_hover_text("Extrude selected curves along a picked direction")
                    .clicked();
                extrude_curve_to_point_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Extrude to Point"))
                    .on_hover_text("Extrude selected curves to a picked apex")
                    .clicked();
                extrude_curve_along_curve_clicked = ui
                    .add_enabled(selected > 1, egui::Button::new("Extrude Along"))
                    .on_hover_text(
                        "Extrude selected profiles along the last-selected curve path",
                    )
                    .clicked();
                revolve_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Revolve"))
                    .on_hover_text("Revolve selected curves around a picked axis")
                    .clicked();
                ui.label(format!("{selected} selected"));
                ui.separator();
                ui.label("Display:");
                ui.selectable_value(
                    &mut self.viewport.display_mode,
                    DisplayMode::Wireframe,
                    "Wireframe",
                );
                ui.selectable_value(
                    &mut self.viewport.display_mode,
                    DisplayMode::Shaded,
                    "Shaded",
                );
                ui.selectable_value(
                    &mut self.viewport.display_mode,
                    DisplayMode::Ghosted,
                    "Ghosted",
                );
                ui.separator();
                ui.toggle_value(&mut self.osnap, "Osnap")
                    .on_hover_text(
                        "Snap to visible Point, End, Mid, Center, and Quad features, including locked geometry",
                    );
                ui.toggle_value(&mut self.smart_track, "SmartTrack")
                    .on_hover_text("Track horizontally or vertically from the last picked point");
            });
        });
        if undo_clicked {
            self.execute_command("Undo");
        } else if redo_clicked {
            self.execute_command("Redo");
        } else if select_all_clicked {
            self.execute_command("SelAll");
        } else if select_none_clicked {
            self.execute_command("SelNone");
        } else if select_last_clicked {
            self.execute_command("SelLast");
        } else if select_previous_clicked {
            self.execute_command("SelPrev");
        } else if delete_clicked {
            self.execute_command("Delete");
        } else if hide_clicked {
            self.execute_command("Hide");
        } else if show_clicked {
            self.execute_command("Show");
        } else if lock_clicked {
            self.execute_command("Lock");
        } else if unlock_clicked {
            self.execute_command("Unlock");
        } else if hide_swap_clicked {
            self.execute_command("HideSwap");
        } else if lock_swap_clicked {
            self.execute_command("LockSwap");
        } else if isolate_clicked {
            self.execute_command("Isolate");
        } else if unisolate_clicked {
            self.execute_command("Unisolate");
        } else if isolate_lock_clicked {
            self.execute_command("IsolateLock");
        } else if unisolate_lock_clicked {
            self.execute_command("UnisolateLock");
        } else if join_clicked {
            self.execute_command("Join");
        } else if explode_clicked {
            self.execute_command("Explode");
        } else if flip_clicked {
            self.execute_command("Flip");
        } else if unify_mesh_normals_clicked {
            self.execute_command("UnifyMeshNormals");
        } else if combine_mesh_vertices_clicked {
            self.execute_command("CombineIdenticalMeshVertices");
        } else if cull_unused_mesh_vertices_clicked {
            self.execute_command("CullUnusedMeshVertices");
        } else if split_disjoint_mesh_clicked {
            self.execute_command("SplitDisjointMesh");
        } else if extract_non_manifold_clicked {
            self.execute_command("ExtractNonManifoldMeshEdges");
        } else if extract_duplicate_faces_clicked {
            self.execute_command("ExtractDuplicateMeshFaces");
        } else if close_curve_clicked {
            self.execute_command("CloseCrv");
        } else if curve_start_clicked {
            self.execute_command("CrvStart");
        } else if curve_end_clicked {
            self.execute_command("CrvEnd");
        } else if extract_points_clicked {
            self.execute_command("ExtractPt");
        } else if length_clicked {
            self.execute_command("Length");
        } else if area_clicked {
            self.execute_command("Area");
        } else if volume_clicked {
            self.execute_command("Volume");
        } else if bounding_box_clicked {
            self.execute_command("BoundingBox");
        } else if move_clicked {
            self.try_start_interactive_command("Move");
        } else if copy_clicked {
            self.try_start_interactive_command("Copy");
        } else if scale_clicked {
            self.try_start_interactive_command("Scale");
        } else if rotate_clicked {
            self.try_start_interactive_command("Rotate");
        } else if mirror_clicked {
            self.try_start_interactive_command("Mirror");
        } else if shear_clicked {
            self.try_start_interactive_command("Shear");
        } else if project_to_cplane_clicked {
            self.execute_command("ProjectToCPlane");
        } else if to_nurbs_clicked {
            self.execute_command("ToNURBS");
        } else if planar_surface_clicked {
            self.execute_command("PlanarSrf");
        } else if sphere_clicked {
            self.try_start_interactive_command("Sphere");
        } else if ellipsoid_clicked {
            self.try_start_interactive_command("Ellipsoid");
        } else if extrude_curve_clicked {
            self.try_start_interactive_command("ExtrudeCrv");
        } else if extrude_curve_to_point_clicked {
            self.try_start_interactive_command("ExtrudeCrvToPoint");
        } else if extrude_curve_along_curve_clicked {
            self.execute_command("ExtrudeCrvAlongCrv");
        } else if revolve_clicked {
            self.try_start_interactive_command("Revolve");
        }
    }

    fn show_layers(&mut self, root: &mut egui::Ui) {
        for action in self.sidebar.show(root, &self.document) {
            self.apply_sidebar_action(action);
        }
    }

    fn apply_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::AddLayer { name } => {
                let color = suggested_layer_color(self.document.layers().len());
                let result =
                    edit_document_transaction(&mut self.document, "Add layer", |document| {
                        let id = document.add_layer(&name, color)?;
                        document.set_current_layer(id)?;
                        Ok(id)
                    });
                match result {
                    Ok(_) => {
                        self.sidebar.clear_new_layer_name();
                        self.push_log(format!("Created current layer '{name}'"));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::EditLayer {
                id,
                old_name,
                name,
                color,
            } => {
                let result =
                    edit_document_transaction(&mut self.document, "Edit layer", |document| {
                        let renamed = document.rename_layer(id, &name)?;
                        let recolored = document.set_layer_color(id, color)?;
                        Ok(renamed || recolored)
                    });
                match result {
                    Ok(_) => {
                        self.sidebar.close_layer_editor(id);
                        self.push_log(format!(
                            "Updated layer '{old_name}' as '{name}' with color {},{},{}",
                            color.red, color.green, color.blue
                        ));
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::DeleteLayer { id, name } => match self.document.delete_layer(id) {
                Ok(()) => {
                    self.sidebar.close_layer_editor(id);
                    self.push_log(format!("Deleted layer '{name}'"));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            SidebarAction::SetCurrent { id, name } => match self.document.set_current_layer(id) {
                Ok(()) => self.push_log(format!("Current layer is '{name}'")),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            SidebarAction::SetVisibility { id, name, visible } => {
                match self.document.set_layer_visibility(id, visible) {
                    Ok(_) => self.push_log(format!(
                        "Layer '{name}' is {}",
                        if visible { "visible" } else { "hidden" }
                    )),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::SetLocked { id, name, locked } => {
                match self.document.set_layer_locked(id, locked) {
                    Ok(_) => self.push_log(format!(
                        "Layer '{name}' is {}",
                        if locked { "locked" } else { "unlocked" }
                    )),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            SidebarAction::RemoveGroup { id, name } => match self.document.remove_group(id) {
                Ok(members) => {
                    self.push_log(format!("Removed group '{name}' ({members} object(s))"));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
        }
    }

    fn show_command_line(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("command_line")
            .resizable(true)
            .default_size(120.0)
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(82.0)
                    .show(ui, |ui| {
                        for line in &self.command_log {
                            ui.label(line);
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    let label = self
                        .active_command
                        .map_or("Command", InteractiveCommand::name);
                    ui.label(RichText::new(format!("{label}:")).strong());
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.command_input)
                            .desired_width(f32::INFINITY)
                            .hint_text(if self.active_command.is_some() {
                                if self
                                    .active_command
                                    .is_some_and(InteractiveCommand::collects_curve_points)
                                {
                                    "Pick curve points; press Enter to finish or Esc to cancel"
                                } else {
                                    "Pick in the viewport or press Esc"
                                }
                            } else {
                                "Point 0,0,0 | Line 0,0,0 10,5,0"
                            }),
                    );
                    if response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    {
                        self.run_command();
                        response.request_focus();
                    }
                });
            });
    }
}

impl eframe::App for VibocerosApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.active_command.is_some() {
                self.cancel_interactive_command(true);
            } else {
                let count = self.document.clear_selection();
                if count > 0 {
                    self.push_log(format!("Deselected {count} object(s)"));
                }
            }
        }
        if self
            .active_command
            .is_some_and(InteractiveCommand::collects_curve_points)
            && self.command_input.trim().is_empty()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Enter))
        {
            self.finish_interactive_curve();
        }
        if self.active_command.is_none()
            && self.document.selected_object_count() > 0
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Delete))
        {
            self.execute_command("Delete");
        }
        self.show_toolbar(ui);
        self.show_layers(ui);
        self.show_command_line(ui);
        let drafting = DraftingInput {
            active: self.active_command.is_some(),
            osnap: self.osnap,
            smart_track: self.smart_track,
            anchor: if self
                .active_command
                .is_some_and(InteractiveCommand::collects_curve_points)
            {
                self.curve_points.last().copied()
            } else {
                self.active_command.and_then(InteractiveCommand::anchor)
            },
            reference: self.active_command.and_then(InteractiveCommand::reference),
        };
        let mut viewport_output = ViewportOutput::default();
        egui::CentralPanel::default().show(ui, |ui| {
            viewport_output = self
                .viewport
                .show(ui, &self.document, drafting, &self.curve_points);
        });
        if viewport_output.cancelled {
            self.cancel_interactive_command(true);
        } else if let Some(point) = viewport_output.picked_point {
            self.accept_drafting_point(point);
        } else if let Some(click) = viewport_output.selection_click {
            self.apply_selection_click(click);
        }
    }
}

fn edit_document_transaction<T>(
    document: &mut Document,
    label: &'static str,
    edit: impl FnOnce(&mut Document) -> Result<T, DocumentError>,
) -> Result<T, DocumentError> {
    document.begin_transaction(label)?;
    match edit(document) {
        Ok(value) => {
            document.commit_transaction()?;
            Ok(value)
        }
        Err(error) => {
            document.rollback_transaction()?;
            Err(error)
        }
    }
}

fn format_model_point(point: Point3) -> String {
    format!("{},{},{}", point.x(), point.y(), point.z())
}

fn same_top_point(left: Point3, right: Point3, tolerance: Tolerance) -> bool {
    (left.x() - right.x()).hypot(left.y() - right.y()) <= tolerance.absolute()
}

fn point_is_near_axis(
    axis_start: Point3,
    axis_end: Point3,
    point: Point3,
    tolerance: Tolerance,
) -> bool {
    axis_start
        .vector_to(axis_end)
        .and_then(|axis| axis.normalized(tolerance))
        .and_then(|axis| axis_start.vector_to(point)?.cross(axis.as_vector()))
        .and_then(|perpendicular| perpendicular.length())
        .map_or(true, |distance| distance <= tolerance.absolute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::{ColorRgb, Geometry};

    fn test_app() -> VibocerosApp {
        VibocerosApp {
            document: Document::default(),
            commands: CommandRegistry::with_builtins(),
            command_input: String::new(),
            command_log: VecDeque::new(),
            viewport: Viewport::default(),
            osnap: true,
            smart_track: true,
            active_command: None,
            curve_points: Vec::new(),
            sidebar: DocumentSidebar::default(),
        }
    }

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn interactive_line_uses_the_transactional_command_path() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("_Line"));
        app.accept_drafting_point(point(1.0, 2.0, 3.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line {
                start: Some(point(1.0, 2.0, 3.0))
            })
        );

        app.accept_drafting_point(point(4.0, 6.0, 3.0));
        assert_eq!(app.active_command, None);
        let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactively created line")
        };
        assert_eq!(line.start(), point(1.0, 2.0, 3.0));
        assert_eq!(line.end(), point(4.0, 6.0, 3.0));
        assert_eq!(app.document.undo_label(), Some("Line"));
    }

    #[test]
    fn a_degenerate_second_pick_keeps_line_active() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("L"));
        let start = point(2.0, 3.0, 0.0);
        app.accept_drafting_point(start);
        app.accept_drafting_point(start);

        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line { start: Some(start) })
        );
        assert_eq!(app.document.objects().len(), 0);
        assert!(app.command_log.back().unwrap().contains("line end"));
    }

    #[test]
    fn interactive_circle_and_arc_reject_degenerate_picks_and_use_history() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Circle"));
        let center = point(0.0, 0.0, 2.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(point(0.0, 0.0, 9.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Circle {
                center: Some(center)
            })
        );
        app.accept_drafting_point(point(2.0, 0.0, 2.0));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Circle(circle) if circle.center() == center && circle.radius() == 2.0
        ));
        assert_eq!(app.document.undo_label(), Some("Circle"));

        assert!(app.try_start_interactive_command("A"));
        app.accept_drafting_point(point(5.0, 0.0, 0.0));
        app.accept_drafting_point(point(6.0, 0.0, 0.0));
        app.accept_drafting_point(point(7.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Arc {
                points: [Some(point(5.0, 0.0, 0.0)), Some(point(6.0, 0.0, 0.0))]
            })
        );
        assert_eq!(app.document.objects().len(), 1);
        app.accept_drafting_point(point(6.0, 1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().nth(1).unwrap().geometry(),
            Geometry::Arc(_)
        ));
        assert_eq!(app.document.undo_label(), Some("Arc"));
    }

    #[test]
    fn interactive_sphere_uses_center_and_radius_point_transactionally() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Sph"));
        let center = point(1.0, 2.0, 3.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Sphere {
                center: Some(center)
            })
        );
        assert_eq!(app.document.objects().len(), 0);
        assert!(app.command_log.back().unwrap().contains("sphere point"));

        app.accept_drafting_point(point(4.0, 2.0, 3.0));
        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created NURBS sphere")
        };
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 5);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(4.0, 2.0, 3.0));
        assert_eq!(app.document.undo_label(), Some("Sphere"));
    }

    #[test]
    fn interactive_ellipsoid_validates_each_axis_and_uses_command_history() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Ellipsoid"));
        let center = point(1.0, 2.0, 3.0);
        let first_axis = point(3.0, 2.0, 3.0);
        let second_axis = point(1.0, 5.0, 3.0);

        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Ellipsoid {
                points: [Some(center), None, None]
            })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(first_axis);
        let command = app.active_command.unwrap();
        assert_eq!(command.anchor(), Some(center));
        assert_eq!(command.reference(), Some(first_axis));
        app.accept_drafting_point(point(5.0, 2.0, 3.0));
        assert_eq!(app.active_command, Some(command));
        assert!(app.command_log.back().unwrap().contains("coordinate frame"));

        app.accept_drafting_point(second_axis);
        let command = app.active_command.unwrap();
        assert_eq!(command.anchor(), Some(center));
        assert_eq!(command.reference(), Some(second_axis));
        app.accept_drafting_point(center);
        assert_eq!(app.active_command, Some(command));
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(1.0, 2.0, 7.0));
        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created NURBS ellipsoid")
        };
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 5);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), first_axis);
        assert_eq!(
            surface.evaluate(std::f64::consts::FRAC_PI_2, 0.0).unwrap(),
            second_axis
        );
        assert_eq!(
            surface.evaluate(0.0, std::f64::consts::FRAC_PI_2).unwrap(),
            point(1.0, 2.0, 7.0)
        );
        assert_eq!(app.document.undo_label(), Some("Ellipsoid"));
    }

    #[test]
    fn interactive_rectangle_keeps_a_degenerate_second_pick_active() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Rect"));
        let first = point(-2.0, -1.0, 3.0);
        app.accept_drafting_point(first);
        app.accept_drafting_point(point(-2.0, 4.0, 3.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Rectangle { first: Some(first) })
        );
        assert_eq!(app.document.objects().len(), 0);

        app.accept_drafting_point(point(5.0, 4.0, 9.0));
        assert_eq!(app.active_command, None);
        let Geometry::Polyline(rectangle) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactive rectangle polyline")
        };
        assert!(rectangle.is_closed());
        assert_eq!(rectangle.segment_count(), 4);
        assert!(
            rectangle
                .vertices()
                .iter()
                .all(|vertex| vertex.z() == first.z())
        );
        assert_eq!(app.document.undo_label(), Some("Rectangle"));
    }

    #[test]
    fn interactive_ellipse_and_polygon_validate_each_pick() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Ell"));
        let center = point(0.0, 0.0, 2.0);
        let first_axis = point(4.0, 0.0, 2.0);
        app.accept_drafting_point(center);
        app.accept_drafting_point(center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: None,
            })
        );
        app.accept_drafting_point(first_axis);
        app.accept_drafting_point(point(2.0, 0.0, 2.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Ellipse {
                center: Some(center),
                first_axis: Some(first_axis),
            })
        );
        app.accept_drafting_point(point(0.0, 3.0, 2.0));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Ellipse(ellipse)
                if ellipse.radius_x() == 4.0 && ellipse.radius_y() == 3.0
        ));
        assert_eq!(app.document.undo_label(), Some("Ellipse"));

        assert!(app.try_start_interactive_command("Polygon 6"));
        let polygon_center = point(10.0, 10.0, 5.0);
        app.accept_drafting_point(polygon_center);
        app.accept_drafting_point(polygon_center);
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Polygon {
                side_count: 6,
                center: Some(polygon_center),
            })
        );
        app.accept_drafting_point(point(12.0, 10.0, 5.0));
        assert_eq!(app.active_command, None);
        let Geometry::Polyline(polygon) = app.document.objects().nth(1).unwrap().geometry() else {
            panic!("expected an interactive polygon")
        };
        assert!(polygon.is_closed());
        assert_eq!(polygon.segment_count(), 6);
        assert_eq!(app.document.undo_label(), Some("Polygon"));

        assert!(!app.try_start_interactive_command("Polygon 2"));
        assert!(app.try_start_interactive_command("Polygon"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Polygon {
                side_count: 4,
                center: None,
            })
        );
    }

    #[test]
    fn interactive_polyline_collects_until_enter_and_cancel_discards_vertices() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("PLine"));
        app.run_command();
        assert_eq!(app.active_command, Some(InteractiveCommand::Polyline));
        assert_eq!(app.document.objects().len(), 0);

        let first = point(0.0, 0.0, 1.0);
        let second = point(3.0, 0.0, 1.0);
        let third = point(3.0, 2.0, 1.0);
        app.accept_drafting_point(first);
        app.accept_drafting_point(first);
        assert_eq!(app.curve_points, vec![first]);
        app.accept_drafting_point(second);
        app.accept_drafting_point(third);
        app.accept_drafting_point(first);
        assert_eq!(app.curve_points.last(), Some(&first));

        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive polyline")
        };
        assert!(polyline.is_closed());
        assert_eq!(polyline.segment_count(), 3);
        assert_eq!(app.document.undo_label(), Some("Polyline"));

        assert!(app.try_start_interactive_command("Polyline"));
        app.accept_drafting_point(point(5.0, 5.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().len(), 1);
    }

    #[test]
    fn interactive_curve_collects_control_points_and_preserves_degree_option() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("Curve Degree=5"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: 5,
                closure: ControlPointCurveClosure::Open,
            })
        );

        let controls = [
            point(0.0, 0.0, 0.0),
            point(2.0, 3.0, 0.0),
            point(10.0, 0.0, 0.0),
        ];
        app.accept_drafting_point(controls[0]);
        app.accept_drafting_point(controls[0]);
        assert_eq!(app.curve_points, vec![controls[0]]);
        for control in &controls[1..] {
            app.accept_drafting_point(*control);
        }
        app.run_command();

        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive control-point curve")
        };
        assert_eq!(curve.degree(), 2);
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            controls
        );
        assert_eq!(app.document.undo_label(), Some("Curve"));

        assert!(app.try_start_interactive_command("Curve Close=Smooth Degree=3"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: 3,
                closure: ControlPointCurveClosure::Smooth,
            })
        );
        let periodic_controls = [
            point(20.0, 0.0, 0.0),
            point(22.0, 3.0, 0.0),
            point(25.0, 0.0, 0.0),
            point(22.0, -2.0, 0.0),
        ];
        for control in &periodic_controls[..2] {
            app.accept_drafting_point(*control);
        }
        app.run_command();
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                closure: ControlPointCurveClosure::Smooth,
                ..
            })
        ));
        assert_eq!(app.document.objects().len(), 1);
        for control in &periodic_controls[2..] {
            app.accept_drafting_point(*control);
        }
        app.run_command();
        let Geometry::NurbsCurve(periodic) = app.document.objects().nth(1).unwrap().geometry()
        else {
            panic!("expected an interactive periodic control-point curve")
        };
        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());

        assert!(app.try_start_interactive_command("Curve Degree=15"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: 11,
                closure: ControlPointCurveClosure::Open,
            })
        );
        app.accept_drafting_point(point(20.0, 0.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().len(), 2);
    }

    #[test]
    fn interactive_interp_crv_collects_points_until_enter() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("InterpCurve"));
        assert_eq!(app.active_command, Some(InteractiveCommand::InterpCrv));

        let points = [
            point(0.0, 0.0, 0.0),
            point(1.0, 2.0, 0.0),
            point(4.0, -1.0, 0.0),
            point(6.0, 0.0, 0.0),
        ];
        app.accept_drafting_point(points[0]);
        app.accept_drafting_point(points[0]);
        assert_eq!(app.curve_points, vec![points[0]]);
        for point in &points[1..] {
            app.accept_drafting_point(*point);
        }
        app.run_command();

        assert_eq!(app.active_command, None);
        assert!(app.curve_points.is_empty());
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive interpolated curve")
        };
        let mut parameter = 0.0;
        for (index, expected) in points.into_iter().enumerate() {
            assert!(
                curve
                    .evaluate(parameter)
                    .unwrap()
                    .is_near(expected, Tolerance::DEFAULT)
            );
            if let Some(next) = points.get(index + 1) {
                parameter += expected.distance_to(*next).unwrap();
            }
        }
        assert_eq!(app.document.undo_label(), Some("InterpCrv"));

        assert!(app.try_start_interactive_command("InterpCrv"));
        app.accept_drafting_point(point(10.0, 10.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.curve_points.is_empty());
        assert_eq!(app.document.objects().len(), 1);
    }

    #[test]
    fn interactive_srfpt_collects_four_corners_and_uses_command_history() {
        let mut app = test_app();
        assert!(app.try_start_interactive_command("SurfaceFromCorners"));
        let corners = [
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
        ];
        for corner in corners[..3].iter().copied() {
            app.accept_drafting_point(corner);
            assert!(matches!(
                app.active_command,
                Some(InteractiveCommand::SrfPt { .. })
            ));
        }
        app.accept_drafting_point(corners[3]);
        assert_eq!(app.active_command, None);
        let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry()
        else {
            panic!("expected an interactively created NURBS surface")
        };
        assert_eq!(surface.evaluate(0.5, 0.5).unwrap(), point(2.0, 1.5, 0.0));
        assert_eq!(app.document.undo_label(), Some("SrfPt"));
    }

    #[test]
    fn coordinate_commands_still_bypass_interactive_mode() {
        let mut app = test_app();
        app.command_input = "Point 7,8,9".to_owned();
        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().next().unwrap().geometry(),
            Geometry::Point(point) if *point == Point3::try_new(7.0, 8.0, 9.0).unwrap()
        ));

        app.command_input = "Curve 0,0 1,1".to_owned();
        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().nth(1).unwrap().geometry(),
            Geometry::NurbsCurve(curve) if curve.degree() == 1
        ));
    }

    #[test]
    fn interactive_move_and_copy_use_the_selected_objects() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("Move"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Move {
                start: Some(point(0.0, 0.0, 0.0))
            })
        );
        app.accept_drafting_point(point(3.0, -1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("Move"));
        assert!(matches!(
            app.document.object(original).unwrap().geometry(),
            Geometry::Point(position) if *position == point(4.0, 1.0, 0.0)
        ));

        assert!(app.try_start_interactive_command("Copy"));
        app.accept_drafting_point(point(4.0, 1.0, 0.0));
        app.accept_drafting_point(point(6.0, 4.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Copy"));
        assert_eq!(app.document.objects().len(), 2);
        let copy = app.document.selected_object_ids().next().unwrap();
        assert_ne!(copy, original);
        assert!(matches!(
            app.document.object(copy).unwrap().geometry(),
            Geometry::Point(position) if *position == point(6.0, 4.0, 0.0)
        ));
    }

    #[test]
    fn interactive_linear_array_uses_a_count_and_two_reference_points() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ArrayLinear 3"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ArrayLinear {
                item_count: 3,
                start: Some(point(0.0, 0.0, 0.0)),
            })
        );
        app.accept_drafting_point(point(2.0, -1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("ArrayLinear"));
        assert!(app.document.is_selected(original));
        let mut locations = app
            .document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => panic!("expected arrayed points"),
            })
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(
            locations,
            vec![[1.0, 2.0, 0.0], [3.0, 1.0, 0.0], [5.0, 0.0, 0.0]]
        );

        assert!(!app.try_start_interactive_command("ArrayLinear"));
        assert!(!app.try_start_interactive_command("ArrayLinear 1"));
    }

    #[test]
    fn interactive_rectangular_array_uses_counts_options_and_two_corners() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("Array 2 2 2 Mode=UnitCell ZDistance=4"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Array {
                counts: [2, 2, 2],
                fill: false,
                z_distance: 4.0,
                start: None,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Array {
                counts: [2, 2, 2],
                fill: false,
                z_distance: 4.0,
                start: Some(point(0.0, 0.0, 0.0)),
            })
        );
        app.accept_drafting_point(point(2.0, -1.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("Array"));
        assert!(app.document.is_selected(original));
        let mut locations = app
            .document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => panic!("expected rectangular-array points"),
            })
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| left.partial_cmp(right).unwrap());
        let mut expected = Vec::new();
        for z in [0.0, 4.0] {
            for y in [0.0, -1.0] {
                for x in [0.0, 2.0] {
                    expected.push([1.0 + x, 2.0 + y, z]);
                }
            }
        }
        expected.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(locations, expected);

        assert!(!app.try_start_interactive_command("Array"));
        assert!(!app.try_start_interactive_command("Array 0 2"));
        assert!(!app.try_start_interactive_command("Array 2 2 2"));
        assert!(!app.try_start_interactive_command("Array 2 2 Mode=Maybe"));
        assert!(!app.try_start_interactive_command("Array 3 2 2 2 -1 4"));
    }

    #[test]
    fn interactive_polar_array_uses_a_count_angle_options_and_center() {
        let mut app = test_app();
        let original = app
            .document
            .add_geometry(Geometry::Point(point(2.0, 0.0, 0.0)))
            .unwrap();
        app.document
            .select_object(original, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ArrayPolar 4 180 Rotate=No ZOffset=2"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ArrayPolar {
                item_count: 4,
                fill_angle_degrees: 180.0,
                rotate: false,
                z_offset: 2.0,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.undo_label(), Some("ArrayPolar"));
        assert!(app.document.is_selected(original));
        let mut locations = app
            .document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("expected arrayed points"),
            })
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| left.z().partial_cmp(&right.z()).unwrap());
        let root_three = 3.0_f64.sqrt();
        let expected = [
            point(2.0, 0.0, 0.0),
            point(1.0, root_three, 2.0),
            point(-1.0, root_three, 4.0),
            point(-2.0, 0.0, 6.0),
        ];
        for (actual, expected) in locations.into_iter().zip(expected) {
            assert!(actual.is_near(expected, app.document.tolerance()));
        }

        assert!(!app.try_start_interactive_command("ArrayPolar"));
        assert!(!app.try_start_interactive_command("ArrayPolar 1"));
        assert!(!app.try_start_interactive_command("ArrayPolar 4 0"));
        assert!(!app.try_start_interactive_command("ArrayPolar 4 Rotate=Maybe"));
    }

    #[test]
    fn interactive_transforms_require_a_selection() {
        let mut app = test_app();
        for command in [
            "M",
            "Copy",
            "Array 3 2",
            "ArrayLinear 3",
            "ArrayPolar 4",
            "Scale",
            "Scale1D",
            "Scale2D",
            "Rotate",
            "Rotate3D",
            "Mirror",
            "Shear",
            "ExtrudeCrv",
            "ExtrudeCrvToPoint",
            "Revolve",
        ] {
            assert!(app.try_start_interactive_command(command));
            assert_eq!(app.active_command, None);
            assert!(app.command_log.back().unwrap().contains("no objects"));
        }
    }

    #[test]
    fn interactive_curve_extrusion_uses_two_direction_points_and_options() {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ExtrudeCrv BothSides=Yes DeleteInput=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurve {
                base: None,
                both_sides: true,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurve {
                base: Some(point(0.0, 0.0, 0.0)),
                both_sides: true,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 2.0, 0.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        let output = app.document.objects().nth(1).unwrap();
        assert!(!app.document.is_selected(output.id()));
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("expected an extruded NURBS surface")
        };
        assert_eq!(surface.domain_v(), 0.0..=4.0);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(0.0, -2.0, 0.0));
        assert_eq!(surface.evaluate(0.0, 4.0).unwrap(), point(0.0, 2.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrv"));
        assert!(!app.try_start_interactive_command("ExtrudeCrv BothSides=Maybe"));
    }

    #[test]
    fn interactive_curve_to_point_extrusion_uses_one_apex_pick() {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command("ExtrudeCrvToPoint DeleteInput=No"));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurveToPoint {
                delete_input: false,
                solid: false,
            })
        );
        app.accept_drafting_point(point(1.0, 2.0, 5.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        let output = app.document.objects().nth(1).unwrap();
        assert!(!app.document.is_selected(output.id()));
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("expected a curve-to-point NURBS surface")
        };
        assert_eq!(surface.degree_u(), 1);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.domain_v(), 0.0..=4.0);
        let apex = point(1.0, 2.0, 5.0);
        assert_eq!(
            surface.evaluate(*surface.domain_u().end(), 2.0).unwrap(),
            apex
        );
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
        assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint DeleteInput=Maybe"));
    }

    #[test]
    fn interactive_curve_to_point_preserves_solid_option_through_apex_pick() {
        let mut app = test_app();
        let rectangle = viboceros_geometry::Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            app.document.tolerance(),
        )
        .unwrap();
        let source = app
            .document
            .add_geometry(Geometry::Polyline(rectangle))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(app.try_start_interactive_command(
            "ExtrudeCrvToPoint Solid=Yes Output=Surface DeleteInput=No"
        ));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeCurveToPoint {
                delete_input: false,
                solid: true,
            })
        );
        app.accept_drafting_point(point(1.0, 2.0, 6.0));
        assert_eq!(app.active_command, None);
        let output = app.document.objects().nth(1).unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("expected an interactive capped apex B-rep")
        };
        assert!(brep.is_solid());
        assert!((brep.signed_volume(app.document.tolerance()).unwrap() - 12.0).abs() < 1.0e-11);
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
        assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint Solid=Yes Solid=No"));
        assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint Output=SubD"));
    }

    #[test]
    fn interactive_revolve_uses_two_axis_picks_and_angle_options() {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(2.0, 0.0, 0.0),
                    point(2.0, 0.0, 3.0),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();

        assert!(
            app.try_start_interactive_command("Revolve Angle=120 StartAngle=30 DeleteInput=No")
        );
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Revolve {
                axis_start: None,
                start_angle_degrees: 30.0,
                sweep_degrees: 120.0,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Revolve {
                axis_start: Some(point(0.0, 0.0, 0.0)),
                start_angle_degrees: 30.0,
                sweep_degrees: 120.0,
                delete_input: false,
            })
        );
        app.accept_drafting_point(point(0.0, 0.0, 1.0));
        assert_eq!(app.active_command, None);
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        let output = app.document.objects().nth(1).unwrap();
        assert!(!app.document.is_selected(output.id()));
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("expected a revolved NURBS surface")
        };
        assert_eq!(surface.control_point_count_u(), 5);
        assert_eq!(surface.domain_u(), 0.0..=2.0 * 120.0_f64.to_radians());
        assert!(
            surface
                .evaluate(0.0, 0.0)
                .unwrap()
                .is_near(point(3.0_f64.sqrt(), 1.0, 0.0), app.document.tolerance())
        );
        assert_eq!(app.document.undo_label(), Some("Revolve"));
        assert!(!app.try_start_interactive_command("Revolve Angle=0"));
        assert!(!app.try_start_interactive_command("Revolve Angle=361"));
        assert!(!app.try_start_interactive_command("Revolve DeleteInput=Maybe"));
    }

    #[test]
    fn interactive_scale_rotate_and_mirror_use_reference_points() {
        let mut app = test_app();
        let object = app
            .document
            .add_geometry(Geometry::Point(point(2.0, 1.0, 0.0)))
            .unwrap();
        app.document
            .select_object(object, viboceros_document::SelectionMode::Replace)
            .unwrap();
        let position = |app: &VibocerosApp| match app.document.object(object).unwrap().geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected a point"),
        };

        assert!(app.try_start_interactive_command("Scale"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Scale {
                kind: InteractiveScaleKind::Uniform,
                center: Some(point(1.0, 1.0, 0.0)),
                reference: None,
            })
        );
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(3.0, 1.0, 0.0));
        assert_eq!(position(&app), point(3.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Scale1D"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(1.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert_eq!(position(&app), point(0.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale1D"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Scale2D"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(3.0, 1.0, 0.0));
        assert_eq!(position(&app), point(3.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale2D"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Rotate"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(1.0, 2.0, 0.0));
        assert!(position(&app).is_near(point(1.0, 2.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Rotate"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Rotate3D"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Rotate3D {
                points: [Some(_), None, None]
            })
        ));
        app.accept_drafting_point(point(0.0, 0.0, 1.0));
        app.accept_drafting_point(point(0.0, 0.0, 2.0));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Rotate3D {
                points: [Some(_), Some(_), None]
            })
        ));
        app.accept_drafting_point(point(1.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 1.0, 0.0));
        assert!(position(&app).is_near(point(-1.0, 2.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Rotate3D"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Shear"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 1.0));
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Shear {
                origin: Some(point(0.0, 0.0, 0.0)),
                reference: None,
            })
        );
        app.accept_drafting_point(point(1.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 0.0, 2.0));
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::Shear {
                origin: Some(_),
                reference: Some(_),
            })
        ));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        assert!(position(&app).is_near(point(2.0, 3.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Shear"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Mirror"));
        app.accept_drafting_point(point(0.0, 0.0, 0.0));
        app.accept_drafting_point(point(0.0, 1.0, 0.0));
        assert_eq!(position(&app), point(-2.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Mirror"));
    }

    #[test]
    fn viewport_clicks_select_and_empty_clicks_clear() {
        let mut app = test_app();
        let object = app
            .document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.apply_selection_click(SelectionClick {
            object_id: Some(object),
            mode: viboceros_document::SelectionMode::Replace,
        });
        assert!(app.document.is_selected(object));

        app.apply_selection_click(SelectionClick {
            object_id: None,
            mode: viboceros_document::SelectionMode::Add,
        });
        assert!(app.document.is_selected(object));
        app.apply_selection_click(SelectionClick {
            object_id: None,
            mode: viboceros_document::SelectionMode::Replace,
        });
        assert_eq!(app.document.selected_object_count(), 0);
    }

    #[test]
    fn layer_sidebar_crud_is_atomic_and_protects_nonempty_layers() {
        let mut app = test_app();
        let default = app.document.current_layer_id();
        app.sidebar.set_new_layer_name("Construction");
        app.apply_sidebar_action(SidebarAction::AddLayer {
            name: "Construction".to_owned(),
        });
        let construction = app.document.layer_by_name("Construction").unwrap().id();
        assert_eq!(app.document.current_layer_id(), construction);
        assert_eq!(
            app.document.layer(construction).unwrap().color(),
            suggested_layer_color(1)
        );
        assert!(app.sidebar.new_layer_name().is_empty());
        assert_eq!(app.document.undo_label(), Some("Add layer"));

        app.document.undo().unwrap();
        assert!(app.document.layer(construction).is_none());
        assert_eq!(app.document.current_layer_id(), default);
        app.document.redo().unwrap();
        assert_eq!(app.document.current_layer_id(), construction);

        let edited_color = ColorRgb::new(12, 34, 56);
        app.apply_sidebar_action(SidebarAction::EditLayer {
            id: construction,
            old_name: "Construction".to_owned(),
            name: "Reference".to_owned(),
            color: edited_color,
        });
        let edited = app.document.layer(construction).unwrap();
        assert_eq!(edited.name(), "Reference");
        assert_eq!(edited.color(), edited_color);
        assert_eq!(app.document.undo_label(), Some("Edit layer"));

        app.document.undo().unwrap();
        let original = app.document.layer(construction).unwrap();
        assert_eq!(original.name(), "Construction");
        assert_eq!(original.color(), suggested_layer_color(1));
        app.document.redo().unwrap();
        let edited = app.document.layer(construction).unwrap();
        assert_eq!(edited.name(), "Reference");
        assert_eq!(edited.color(), edited_color);

        app.document
            .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
            .unwrap();
        app.apply_sidebar_action(SidebarAction::SetCurrent {
            id: default,
            name: "Default".to_owned(),
        });
        app.apply_sidebar_action(SidebarAction::DeleteLayer {
            id: construction,
            name: "Reference".to_owned(),
        });
        assert!(app.document.layer(construction).is_some());
        assert!(app.command_log.back().unwrap().contains("contains objects"));

        app.apply_sidebar_action(SidebarAction::AddLayer {
            name: "Empty".to_owned(),
        });
        let empty = app.document.layer_by_name("Empty").unwrap().id();
        app.apply_sidebar_action(SidebarAction::SetCurrent {
            id: default,
            name: "Default".to_owned(),
        });
        app.apply_sidebar_action(SidebarAction::DeleteLayer {
            id: empty,
            name: "Empty".to_owned(),
        });
        assert!(app.document.layer(empty).is_none());
        assert_eq!(app.document.undo_label(), Some("Delete layer"));
        app.document.undo().unwrap();
        assert_eq!(app.document.layer(empty).unwrap().name(), "Empty");
    }
}
