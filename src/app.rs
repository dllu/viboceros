use std::collections::VecDeque;

use eframe::egui::{self, Color32, RichText};
use viboceros_command::CommandRegistry;
use viboceros_document::Document;

use crate::viewport::{DisplayMode, Viewport};

const MAX_LOG_ENTRIES: usize = 100;

pub struct VibocerosApp {
    document: Document,
    commands: CommandRegistry,
    command_input: String,
    command_log: VecDeque<String>,
    viewport: Viewport,
    osnap: bool,
    smart_track: bool,
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
        }
    }

    fn run_command(&mut self) {
        let input = self.command_input.trim().to_owned();
        if input.is_empty() {
            return;
        }
        self.push_log(format!("> {input}"));
        match self.commands.execute(&mut self.document, &input) {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
    }

    fn push_log(&mut self, message: String) {
        if self.command_log.len() == MAX_LOG_ENTRIES {
            self.command_log.pop_front();
        }
        self.command_log.push_back(message);
    }

    fn show_toolbar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Viboceros");
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
                ui.toggle_value(&mut self.osnap, "Osnap");
                ui.toggle_value(&mut self.smart_track, "SmartTrack");
            });
        });
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
                    ui.label(RichText::new("Command:").strong());
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.command_input)
                            .desired_width(f32::INFINITY)
                            .hint_text("Point 0,0,0 | Line 0,0,0 10,5,0"),
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
        self.show_toolbar(ui);
        self.show_layers(ui);
        self.show_command_line(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            self.viewport.show(ui, &self.document);
        });
    }
}
