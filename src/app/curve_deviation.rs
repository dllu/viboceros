//! Enter finishes an analysis; Escape or another command dismisses its overlay.
use super::*;
impl VibocerosApp {
    pub(super) fn continue_curve_deviation(&mut self, input: &str) -> bool {
        if self.commands.curve_deviation_view(&self.document).is_none() {
            return false;
        }
        if input.is_empty() {
            self.execute_command("CrvDeviation End");
        } else if input
            .trim_start_matches('_')
            .split_once('=')
            .is_some_and(|(k, _)| k.eq_ignore_ascii_case("KeepMarks"))
        {
            self.execute_command(&format!("CrvDeviation {input}"));
        } else {
            self.dismiss_curve_deviation_for_command(input);
            return false;
        }
        self.command_input.clear();
        true
    }
    pub(super) fn dismiss_curve_deviation_for_command(&mut self, input: &str) {
        let name = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        if !name.eq_ignore_ascii_case("CrvDeviation") && self.commands.recognizes(name) {
            self.commands.clear_curve_deviation();
        }
    }
    pub(super) fn show_curve_deviation(&mut self, ui: &mut egui::Ui) {
        let Some(view) = self.commands.curve_deviation_view(&self.document) else {
            return;
        };
        let mut keep = view.keep_marks;
        let mut open = true;
        let mut end = false;
        egui::Window::new("Curve Deviation")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "Minimum deviation: {}",
                    view.extrema.minimum.distance
                ));
                ui.label(format!(
                    "Maximum deviation: {}",
                    view.extrema.maximum.distance
                ));
                if view.approximate_overlap {
                    ui.label("Overlap and correspondence can differ from Rhino.");
                }
                ui.checkbox(&mut keep, "Keep marks");
                end = ui.button("Done (Enter)").clicked();
            });
        if keep != view.keep_marks {
            self.execute_command(&format!(
                "CrvDeviation KeepMarks={}",
                if keep { "Yes" } else { "No" }
            ));
        }
        if end {
            self.execute_command("CrvDeviation End");
        } else if !open {
            self.commands.clear_curve_deviation();
        }
    }
}
