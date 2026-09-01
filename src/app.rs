use std::collections::VecDeque;

use eframe::egui::{self, Color32, RichText};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, GroupId, LayerId};
use viboceros_geometry::{CircularArc3, Ellipse3, MAX_REGULAR_POLYGON_SIDES, Point3, Tolerance};

use crate::viewport::{DisplayMode, DraftingInput, SelectionClick, Viewport, ViewportOutput};

const MAX_LOG_ENTRIES: usize = 100;

enum SidebarAction {
    SetCurrent {
        id: LayerId,
        name: String,
    },
    SetVisibility {
        id: LayerId,
        name: String,
        visible: bool,
    },
    SetLocked {
        id: LayerId,
        name: String,
        locked: bool,
    },
    RemoveGroup {
        id: GroupId,
        name: String,
    },
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
    Arc {
        points: [Option<Point3>; 2],
    },
    Ellipse {
        center: Option<Point3>,
        first_axis: Option<Point3>,
    },
    Polyline,
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
    Scale {
        center: Option<Point3>,
        reference: Option<Point3>,
    },
    Rotate {
        center: Option<Point3>,
        reference: Option<Point3>,
    },
    Mirror {
        start: Option<Point3>,
    },
}

impl InteractiveCommand {
    const fn name(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Line { .. } => "Line",
            Self::Circle { .. } => "Circle",
            Self::Arc { .. } => "Arc",
            Self::Ellipse { .. } => "Ellipse",
            Self::Polyline => "Polyline",
            Self::Rectangle { .. } => "Rectangle",
            Self::Polygon { .. } => "Polygon",
            Self::SrfPt { .. } => "SrfPt",
            Self::Move { .. } => "Move",
            Self::Copy { .. } => "Copy",
            Self::Scale { .. } => "Scale",
            Self::Rotate { .. } => "Rotate",
            Self::Mirror { .. } => "Mirror",
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
            Self::Scale { center: None, .. } => {
                "Scale: pick the center point in the viewport (Esc to cancel)"
            }
            Self::Scale {
                center: Some(_),
                reference: None,
            } => "Scale: pick the reference point in the viewport (Esc to cancel)",
            Self::Scale {
                reference: Some(_), ..
            } => "Scale: pick the target point in the viewport (Esc to cancel)",
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
            Self::Mirror { start: None } => {
                "Mirror: pick the first axis point in the viewport (Esc to cancel)"
            }
            Self::Mirror { start: Some(_) } => {
                "Mirror: pick the second axis point in the viewport (Esc to cancel)"
            }
        }
    }

    const fn anchor(self) -> Option<Point3> {
        match self {
            Self::Point
            | Self::Line { start: None }
            | Self::Circle { center: None }
            | Self::Arc { points: [None, _] }
            | Self::Ellipse { center: None, .. }
            | Self::Polyline
            | Self::Rectangle { first: None }
            | Self::Polygon { center: None, .. }
            | Self::SrfPt {
                corners: [None, _, _],
            }
            | Self::Move { start: None }
            | Self::Copy { start: None }
            | Self::Scale { center: None, .. }
            | Self::Rotate { center: None, .. }
            | Self::Mirror { start: None } => None,
            Self::Line { start }
            | Self::Circle { center: start }
            | Self::Rectangle { first: start }
            | Self::Move { start }
            | Self::Copy { start }
            | Self::Mirror { start } => start,
            Self::Ellipse { center, .. } | Self::Polygon { center, .. } => center,
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
            } => Some(center),
        }
    }

    const fn reference(self) -> Option<Point3> {
        match self {
            Self::Scale { reference, .. } | Self::Rotate { reference, .. } => reference,
            Self::Ellipse { first_axis, .. } => first_axis,
            _ => None,
        }
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
    polyline_points: Vec<Point3>,
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
            polyline_points: Vec::new(),
        }
    }

    fn run_command(&mut self) {
        let input = self.command_input.trim().to_owned();
        if input.is_empty() {
            if self.active_command == Some(InteractiveCommand::Polyline) {
                self.finish_interactive_polyline();
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
        let command = if matches!(normalized.as_str(), "polygon" | "poly") {
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
        } else {
            if !arguments.is_empty() {
                return false;
            }
            match normalized.as_str() {
                "point" | "pt" => InteractiveCommand::Point,
                "line" | "l" => InteractiveCommand::Line { start: None },
                "circle" | "c" => InteractiveCommand::Circle { center: None },
                "arc" | "a" => InteractiveCommand::Arc { points: [None; 2] },
                "ellipse" | "ell" => InteractiveCommand::Ellipse {
                    center: None,
                    first_axis: None,
                },
                "polyline" | "pline" => InteractiveCommand::Polyline,
                "rectangle" | "rect" => InteractiveCommand::Rectangle { first: None },
                "srfpt" | "surfacefromcorners" => InteractiveCommand::SrfPt { corners: [None; 3] },
                "move" | "m" => InteractiveCommand::Move { start: None },
                "copy" => InteractiveCommand::Copy { start: None },
                "scale" => InteractiveCommand::Scale {
                    center: None,
                    reference: None,
                },
                "rotate" => InteractiveCommand::Rotate {
                    center: None,
                    reference: None,
                },
                "mirror" => InteractiveCommand::Mirror { start: None },
                _ => return false,
            }
        };

        self.cancel_interactive_command(true);
        self.push_log(format!("> {input}"));
        if matches!(
            command,
            InteractiveCommand::Move { .. }
                | InteractiveCommand::Copy { .. }
                | InteractiveCommand::Scale { .. }
                | InteractiveCommand::Rotate { .. }
                | InteractiveCommand::Mirror { .. }
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
        self.polyline_points.clear();
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
                if let Some(previous) = self.polyline_points.last()
                    && previous.is_near(point, self.document.tolerance())
                {
                    self.push_log("Error: adjacent polyline vertices must differ".to_owned());
                    return;
                }
                self.polyline_points.push(point);
                self.push_log(format!(
                    "Vertex {}: {}",
                    self.polyline_points.len(),
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
            InteractiveCommand::Scale { center: None, .. } => {
                let command = InteractiveCommand::Scale {
                    center: Some(point),
                    reference: None,
                };
                self.active_command = Some(command);
                self.push_log(format!("Center: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                center: Some(center),
                reference: None,
            } => {
                if center.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: scale reference must differ from its center".to_owned());
                    return;
                }
                let command = InteractiveCommand::Scale {
                    center: Some(center),
                    reference: Some(point),
                };
                self.active_command = Some(command);
                self.push_log(format!("Reference: {}", format_model_point(point)));
                self.push_log(command.prompt().to_owned());
            }
            InteractiveCommand::Scale {
                center: Some(center),
                reference: Some(reference),
            } => {
                if center.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: scale target must differ from its center".to_owned());
                    return;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Scale {} {} {}",
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
        }
    }

    fn finish_interactive_polyline(&mut self) {
        if self.polyline_points.len() < 2 {
            self.push_log("Error: a polyline requires at least two vertices".to_owned());
            return;
        }
        let points = std::mem::take(&mut self.polyline_points);
        self.active_command = None;
        let arguments = points
            .into_iter()
            .map(format_model_point)
            .collect::<Vec<_>>()
            .join(" ");
        self.execute_command(&format!("Polyline {arguments}"));
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
        let mut delete_clicked = false;
        let mut join_clicked = false;
        let mut explode_clicked = false;
        let mut flip_clicked = false;
        let mut unify_mesh_normals_clicked = false;
        let mut split_disjoint_mesh_clicked = false;
        let mut extract_non_manifold_clicked = false;
        let mut extract_duplicate_faces_clicked = false;
        let mut close_curve_clicked = false;
        let mut curve_start_clicked = false;
        let mut curve_end_clicked = false;
        let mut length_clicked = false;
        let mut area_clicked = false;
        let mut volume_clicked = false;
        let mut move_clicked = false;
        let mut copy_clicked = false;
        let mut scale_clicked = false;
        let mut rotate_clicked = false;
        let mut mirror_clicked = false;
        let selected = self.document.selected_object_count();
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
                delete_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Delete"))
                    .on_hover_text("Delete selected objects")
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
                length_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Length"))
                    .on_hover_text("Measure selected curve length")
                    .clicked();
                area_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Area"))
                    .on_hover_text("Measure selected planar or mesh area")
                    .clicked();
                volume_clicked = ui
                    .add_enabled(selected > 0, egui::Button::new("Volume"))
                    .on_hover_text("Measure signed volume of selected closed meshes")
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
                    .on_hover_text("Snap to visible Point, End, Mid, Center, and Quad features");
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
        } else if delete_clicked {
            self.execute_command("Delete");
        } else if join_clicked {
            self.execute_command("Join");
        } else if explode_clicked {
            self.execute_command("Explode");
        } else if flip_clicked {
            self.execute_command("Flip");
        } else if unify_mesh_normals_clicked {
            self.execute_command("UnifyMeshNormals");
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
        } else if length_clicked {
            self.execute_command("Length");
        } else if area_clicked {
            self.execute_command("Area");
        } else if volume_clicked {
            self.execute_command("Volume");
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
        }
    }

    fn show_layers(&mut self, root: &mut egui::Ui) {
        let mut actions = Vec::new();
        egui::Panel::right("layers")
            .default_size(220.0)
            .show(root, |ui| {
                ui.heading("Layers");
                ui.separator();
                let current = self.document.current_layer_id();
                let layers: Vec<_> = self
                    .document
                    .layers()
                    .map(|layer| {
                        (
                            layer.id(),
                            layer.name().to_owned(),
                            layer.color(),
                            layer.is_visible(),
                            layer.is_locked(),
                        )
                    })
                    .collect();

                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.small("On");
                    ui.small("Lock");
                    ui.small("Layer");
                });
                for (id, name, color, visible, locked) in layers {
                    ui.horizontal(|ui| {
                        let mut new_visible = visible;
                        let visibility = ui
                            .add_enabled_ui(id != current, |ui| ui.checkbox(&mut new_visible, ""))
                            .inner
                            .on_hover_text(if id == current {
                                "The current layer must remain visible"
                            } else {
                                "Show or hide this layer"
                            });
                        if visibility.changed() {
                            actions.push(SidebarAction::SetVisibility {
                                id,
                                name: name.clone(),
                                visible: new_visible,
                            });
                        }

                        let mut new_locked = locked;
                        let lock = ui
                            .add_enabled_ui(id != current, |ui| ui.checkbox(&mut new_locked, ""))
                            .inner
                            .on_hover_text(if id == current {
                                "The current layer must remain unlocked"
                            } else {
                                "Lock or unlock this layer"
                            });
                        if lock.changed() {
                            actions.push(SidebarAction::SetLocked {
                                id,
                                name: name.clone(),
                                locked: new_locked,
                            });
                        }

                        let swatch = Color32::from_rgb(color.red, color.green, color.blue);
                        ui.label(RichText::new("●").color(swatch));
                        let select = ui
                            .add_enabled_ui(visible && !locked, |ui| {
                                ui.selectable_label(id == current, &name)
                            })
                            .inner;
                        if select.clicked() {
                            actions.push(SidebarAction::SetCurrent {
                                id,
                                name: name.clone(),
                            });
                        }
                    });
                }
                ui.add_space(8.0);
                ui.small("Create a layer with: Layer New name");

                ui.add_space(14.0);
                ui.heading("Groups");
                ui.separator();
                let groups: Vec<_> = self
                    .document
                    .groups()
                    .enumerate()
                    .map(|(index, group)| {
                        (
                            group.id(),
                            group
                                .name()
                                .map(str::to_owned)
                                .unwrap_or_else(|| format!("Group {}", index + 1)),
                            group.members().len(),
                        )
                    })
                    .collect();
                if groups.is_empty() {
                    ui.weak("No groups");
                }
                for (id, name, members) in groups {
                    ui.horizontal(|ui| {
                        ui.label(format!("{name} · {members}"))
                            .on_hover_text(format!("Group {id} with {members} object(s)"));
                        if ui.small_button("×").on_hover_text("Ungroup").clicked() {
                            actions.push(SidebarAction::RemoveGroup {
                                id,
                                name: name.clone(),
                            });
                        }
                    });
                }
                ui.add_space(8.0);
                ui.small("Create a group with: Group [name]");
            });

        for action in actions {
            match action {
                SidebarAction::SetCurrent { id, name } => {
                    match self.document.set_current_layer(id) {
                        Ok(()) => self.push_log(format!("Current layer is '{name}'")),
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                }
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
                                if self.active_command == Some(InteractiveCommand::Polyline) {
                                    "Pick vertices; press Enter to finish or Esc to cancel"
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
        if self.active_command == Some(InteractiveCommand::Polyline)
            && self.command_input.trim().is_empty()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Enter))
        {
            self.finish_interactive_polyline();
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
            anchor: if self.active_command == Some(InteractiveCommand::Polyline) {
                self.polyline_points.last().copied()
            } else {
                self.active_command.and_then(InteractiveCommand::anchor)
            },
            reference: self.active_command.and_then(InteractiveCommand::reference),
        };
        let mut viewport_output = ViewportOutput::default();
        egui::CentralPanel::default().show(ui, |ui| {
            viewport_output =
                self.viewport
                    .show(ui, &self.document, drafting, &self.polyline_points);
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

fn format_model_point(point: Point3) -> String {
    format!("{},{},{}", point.x(), point.y(), point.z())
}

fn same_top_point(left: Point3, right: Point3, tolerance: Tolerance) -> bool {
    (left.x() - right.x()).hypot(left.y() - right.y()) <= tolerance.absolute()
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::Geometry;

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
            polyline_points: Vec::new(),
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
        assert_eq!(app.polyline_points, vec![first]);
        app.accept_drafting_point(second);
        app.accept_drafting_point(third);
        app.accept_drafting_point(first);
        assert_eq!(app.polyline_points.last(), Some(&first));

        app.run_command();
        assert_eq!(app.active_command, None);
        assert!(app.polyline_points.is_empty());
        let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected an interactive polyline")
        };
        assert!(polyline.is_closed());
        assert_eq!(polyline.segment_count(), 3);
        assert_eq!(app.document.undo_label(), Some("Polyline"));

        assert!(app.try_start_interactive_command("Polyline"));
        app.accept_drafting_point(point(5.0, 5.0, 0.0));
        app.cancel_interactive_command(false);
        assert!(app.polyline_points.is_empty());
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
    fn interactive_transforms_require_a_selection() {
        let mut app = test_app();
        for command in ["M", "Copy", "Scale", "Rotate", "Mirror"] {
            assert!(app.try_start_interactive_command(command));
            assert_eq!(app.active_command, None);
            assert!(app.command_log.back().unwrap().contains("no objects"));
        }
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
                center: Some(point(1.0, 1.0, 0.0)),
                reference: None,
            })
        );
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(3.0, 1.0, 0.0));
        assert_eq!(position(&app), point(3.0, 1.0, 0.0));
        assert_eq!(app.document.undo_label(), Some("Scale"));
        app.document.undo().unwrap();

        assert!(app.try_start_interactive_command("Rotate"));
        app.accept_drafting_point(point(1.0, 1.0, 0.0));
        app.accept_drafting_point(point(2.0, 1.0, 0.0));
        app.accept_drafting_point(point(1.0, 2.0, 0.0));
        assert!(position(&app).is_near(point(1.0, 2.0, 0.0), app.document.tolerance()));
        assert_eq!(app.document.undo_label(), Some("Rotate"));
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
}
