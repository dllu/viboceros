//! Nonmodal Direction Analysis controls share console state and drawing code.
use super::*;

impl VibocerosApp {
    pub(super) fn set_direction_analysis_display(&mut self, input: &str) {
        match self.commands.execute(&mut self.document, input) {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn show_direction_analysis(&mut self, root: &mut egui::Ui) {
        let Some(view) = self.commands.direction_analysis_view(&self.document) else {
            return;
        };
        let mut options = view.options.clone();
        let mut open = true;
        let mut action = None;
        egui::Window::new("Direction Analysis")
            .open(&mut open)
            .show(root.ctx(), |ui| {
                ui.label(format!("{} objects", view.sources));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut options.direction, "Direction");
                    ui.checkbox(&mut options.u, "U");
                    ui.checkbox(&mut options.v, "V");
                });
                ui.horizontal(|ui| {
                    ui.label("Direction color");
                    ui.color_edit_button_srgb(&mut options.color);
                });
                ui.horizontal(|ui| {
                    if ui.button("Add objects").clicked() {
                        action = Some("ShowDir Add");
                    }
                    if ui.button("Remove objects").clicked() {
                        action = Some("ShowDir Remove");
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (label, command) in [
                        ("Flip direction", "ShowDir Flip"),
                        ("Reverse U", "ShowDir UReverse"),
                        ("Reverse V", "ShowDir VReverse"),
                        ("Swap UV", "ShowDir SwapUV"),
                    ] {
                        if ui.button(label).clicked() {
                            action = Some(command);
                        }
                    }
                });
            });
        if !open {
            self.set_direction_analysis_display("ShowDirOff");
            return;
        }
        if options != view.options {
            self.set_direction_analysis_display(&format!(
                "ShowDir Direction={} U={} V={} Color={},{},{}",
                if options.direction { "Yes" } else { "No" },
                if options.u { "Yes" } else { "No" },
                if options.v { "Yes" } else { "No" },
                options.color[0],
                options.color[1],
                options.color[2]
            ));
        }
        if let Some(action) = action {
            self.cancel_interactive_command(false);
            self.command_input = action.into();
            self.run_command();
        }
    }
}
