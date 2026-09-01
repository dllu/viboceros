use std::collections::VecDeque;

use eframe::egui::{self, Color32, RichText};
use viboceros_command::CommandRegistry;
use viboceros_document::Document;
use viboceros_geometry::Point3;

use crate::viewport::{DisplayMode, DraftingInput, Viewport, ViewportOutput};

const MAX_LOG_ENTRIES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq)]
enum InteractiveCommand {
    Point,
    Line { start: Option<Point3> },
}

impl InteractiveCommand {
    const fn name(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Line { .. } => "Line",
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
        }
    }

    const fn anchor(self) -> Option<Point3> {
        match self {
            Self::Point | Self::Line { start: None } => None,
            Self::Line { start } => start,
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
        }
    }

    fn run_command(&mut self) {
        let input = self.command_input.trim().to_owned();
        if input.is_empty() {
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
        if tokens.next().is_some() {
            return false;
        }
        let command = match name
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "point" | "pt" => InteractiveCommand::Point,
            "line" | "l" => InteractiveCommand::Line { start: None },
            _ => return false,
        };

        self.cancel_interactive_command(true);
        self.push_log(format!("> {input}"));
        self.push_log(command.prompt().to_owned());
        self.active_command = Some(command);
        true
    }

    fn cancel_interactive_command(&mut self, announce: bool) {
        if let Some(command) = self.active_command.take()
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
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal(|ui| {
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
                    .on_hover_text("Snap to visible Point, End, and Mid features");
                ui.toggle_value(&mut self.smart_track, "SmartTrack")
                    .on_hover_text("Track horizontally or vertically from the last picked point");
            });
        });
        if undo_clicked {
            self.execute_command("Undo");
        } else if redo_clicked {
            self.execute_command("Redo");
        }
    }

    fn show_layers(&mut self, root: &mut egui::Ui) {
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
                        )
                    })
                    .collect();

                for (id, name, color, visible) in layers {
                    ui.horizontal(|ui| {
                        let swatch = Color32::from_rgb(color.red, color.green, color.blue);
                        ui.label(RichText::new("●").color(swatch));
                        let label = if visible {
                            name
                        } else {
                            format!("{name} (hidden)")
                        };
                        if ui.selectable_label(id == current, label).clicked() {
                            let _ = self.document.set_current_layer(id);
                        }
                    });
                }
                ui.add_space(8.0);
                ui.small("Create a layer with: Layer name");
            });
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
                                "Pick in the viewport or press Esc"
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
            self.cancel_interactive_command(true);
        }
        self.show_toolbar(ui);
        self.show_layers(ui);
        self.show_command_line(ui);
        let drafting = DraftingInput {
            active: self.active_command.is_some(),
            osnap: self.osnap,
            smart_track: self.smart_track,
            anchor: self.active_command.and_then(InteractiveCommand::anchor),
        };
        let mut viewport_output = ViewportOutput::default();
        egui::CentralPanel::default().show(ui, |ui| {
            viewport_output = self.viewport.show(ui, &self.document, drafting);
        });
        if viewport_output.cancelled {
            self.cancel_interactive_command(true);
        } else if let Some(point) = viewport_output.picked_point {
            self.accept_drafting_point(point);
        }
    }
}

fn format_model_point(point: Point3) -> String {
    format!("{},{},{}", point.x(), point.y(), point.z())
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
}
