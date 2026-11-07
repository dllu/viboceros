//! Curvature graph controls update display state without interrupting modeling.
use super::*;
impl VibocerosApp {
    pub(super) fn curvature_graph_selection_ids(
        &self,
        command: &str,
    ) -> Option<std::collections::BTreeSet<viboceros_document::ObjectId>> {
        (command == "ExtractCurvatureGraph")
            .then(|| self.commands.curvature_graph_curve_sources(&self.document))
    }
    pub(super) fn set_curvature_graph_display(&mut self, action: &str) {
        if action.eq_ignore_ascii_case("CurvatureGraphOff") {
            self.hide_blend_curvature();
        }
        let result = self.commands.execute(&mut self.document, action);
        if let Err(error) = result {
            self.push_log(format!("Error: {error}"));
        }
    }
    pub(super) fn show_curvature_graph(&mut self, ui: &mut egui::Ui) {
        let view = self
            .commands
            .curvature_graph_view(&self.document)
            .ok()
            .flatten();
        let preview = self
            .blend_curve_session
            .as_ref()
            .is_some_and(|s| s.input.ready() && s.input.curvature_display_enabled());
        if view.is_none() && !preview {
            return;
        }
        let original = self.commands.curvature_graph_options();
        let mut options = original.clone();
        let mut open = true;
        let mut action = None;
        egui::Window::new("Curvature Graph")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                if preview {
                    ui.label("Blend curve preview");
                }
                if let Some(view) = &view {
                    ui.label(format!(
                        "{} graphs on {} objects",
                        view.graphs.len(),
                        view.sources
                    ));
                }
                ui.add(egui::Slider::new(&mut options.scale, 0..=300).text("Display scale"));
                ui.add(egui::Slider::new(&mut options.density, 0..=10).text("Density"));
                ui.add(egui::Slider::new(&mut options.count, 2..=64).text("Surface count"));
                ui.horizontal(|ui| {
                    ui.label("Curve hair");
                    ui.color_edit_button_srgb(&mut options.color);
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut options.u, "U hair");
                    ui.color_edit_button_srgb(&mut options.u_color);
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut options.v, "V hair");
                    ui.color_edit_button_srgb(&mut options.v_color);
                });
                if let Some(view) = &view
                    && view.singular_samples != 0
                {
                    ui.label(format!(
                        "{} singular samples omitted",
                        view.singular_samples
                    ));
                }
                ui.add_enabled_ui(!preview, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Add objects").clicked() {
                            action = Some("CurvatureGraph Add".to_owned());
                        }
                        if ui.button("Remove objects").clicked() {
                            action = Some("CurvatureGraph Remove".to_owned());
                        }
                    });
                });
            });
        if !open {
            action = Some("CurvatureGraphOff".into());
        }
        if options != original && action.is_none() {
            if let Some(session) = &self.blend_curve_session
                && let Err(error) = session.preview_input().curvature_display(&options)
            {
                self.push_log(format!("Error: {error}"));
                return;
            }
            if let Err(error) = self
                .commands
                .configure_curvature_graph(&self.document, options)
            {
                self.push_log(format!("Error: {error}"));
            }
        }
        if let Some(action) = action {
            if action.ends_with(" Add") || action.ends_with(" Remove") {
                self.execute_command(&action);
            } else {
                // A display setting must not cancel an in-progress point getter.
                self.set_curvature_graph_display(&action);
            }
        }
    }
}
