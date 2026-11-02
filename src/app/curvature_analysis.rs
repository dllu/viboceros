//! Nonmodal settings share command state and preserve active point getters.
use super::*;
use viboceros_geometry::CurvatureStyle;

impl VibocerosApp {
    pub(super) fn show_curvature_analysis(&mut self, root: &mut egui::Ui) {
        let Some(view) = self.commands.curvature_analysis_view(&self.document) else {
            return;
        };
        let mut options = view.options;
        let mut open = true;
        let mut action = None;
        let mut max_range = false;
        egui::Window::new("Curvature Analysis")
            .open(&mut open)
            .show(root.ctx(), |ui| {
                ui.label(format!("{} objects", view.sources));
                egui::ComboBox::from_id_salt("curvature_style")
                    .selected_text(options.style.name())
                    .show_ui(ui, |ui| {
                        for style in CurvatureStyle::ALL {
                            ui.selectable_value(&mut options.style, style, style.name());
                        }
                    });
                let i = options.index();
                ui.horizontal(|ui| {
                    ui.label("Blue");
                    ui.add(egui::DragValue::new(&mut options.ranges[i][0]).speed(0.001));
                });
                ui.horizontal(|ui| {
                    ui.label("Red");
                    ui.add(egui::DragValue::new(&mut options.ranges[i][1]).speed(0.001));
                });
                ui.checkbox(&mut options.show_isocurves, "Show edges and isocurves");
                if ui.button("Max range").clicked() {
                    max_range = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("Add objects").clicked() {
                        action = Some("CurvatureAnalysis Add");
                    }
                    if ui.button("Remove objects").clicked() {
                        action = Some("CurvatureAnalysis Remove");
                    }
                });
                ui.small("Gray marks unavailable curvature.");
                if view.unavailable > 0 {
                    ui.label(format!(
                        "{} objects could not be analyzed",
                        view.unavailable
                    ));
                }
            });
        if !open {
            self.set_analysis_display("CurvatureAnalysisOff");
            return;
        }
        if options != view.options {
            let range = options.range();
            self.set_analysis_display(&format!(
                "CurvatureAnalysis Style={} Range={},{} ShowIsoCurves={}",
                options.style.name(),
                range[0],
                range[1],
                if options.show_isocurves { "Yes" } else { "No" }
            ));
        }
        if max_range {
            self.set_analysis_display("CurvatureAnalysis MaxRange");
        }
        if let Some(action) = action {
            self.cancel_interactive_command(false);
            self.command_input = action.into();
            self.run_command();
        }
    }
}
