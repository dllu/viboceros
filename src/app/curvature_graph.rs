//! Curvature graph controls update display state without interrupting modeling.
use super::*;
impl VibocerosApp {
    pub(super) fn set_curvature_graph_display(&mut self, action: &str) {
        let result = self.commands.execute(&mut self.document, action);
        if let Err(error) = result {
            self.push_log(format!("Error: {error}"));
        }
    }
    pub(super) fn show_curvature_graph(&mut self, ui: &mut egui::Ui) {
        let Ok(Some(view)) = self.commands.curvature_graph_view(&self.document) else {
            return;
        };
        let mut options = view.options.clone();
        let mut open = true;
        let mut action = None;
        egui::Window::new("Curvature Graph")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "{} graphs on {} objects",
                    view.graphs.len(),
                    view.sources
                ));
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
                if view.singular_samples != 0 {
                    ui.label(format!(
                        "{} singular samples omitted",
                        view.singular_samples
                    ));
                }
                ui.horizontal(|ui| {
                    if ui.button("Add objects").clicked() {
                        action = Some("CurvatureGraph Add".to_owned());
                    }
                    if ui.button("Remove objects").clicked() {
                        action = Some("CurvatureGraph Remove".to_owned());
                    }
                });
            });
        if !open {
            action = Some("CurvatureGraphOff".into());
        }
        if options != view.options && action.is_none() {
            action = Some(format!(
                "CurvatureGraph Scale={} Density={} Count={} U={} V={} Color={},{},{} UColor={},{},{} VColor={},{},{}",
                options.scale,
                options.density,
                options.count,
                if options.u { "Yes" } else { "No" },
                if options.v { "Yes" } else { "No" },
                options.color[0],
                options.color[1],
                options.color[2],
                options.u_color[0],
                options.u_color[1],
                options.u_color[2],
                options.v_color[0],
                options.v_color[1],
                options.v_color[2]
            ));
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
