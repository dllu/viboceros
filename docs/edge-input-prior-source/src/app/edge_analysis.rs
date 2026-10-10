//! Edge Analysis controls and camera requests share the console session.
use super::*;
impl VibocerosApp {
    pub(super) fn show_edge_analysis(&mut self, ui: &mut egui::Ui) {
        let Ok(Some(view)) = self.commands.edge_analysis_view(&self.document) else {
            return;
        };
        if view.zoom_requested {
            if let Some(bounds) = view.bounds() {
                if self.viewports[self.active_viewport]
                    .zoom_bounding_box(bounds)
                    .is_ok()
                {
                    self.commands.acknowledge_edge_analysis_zoom();
                }
            } else {
                self.commands.acknowledge_edge_analysis_zoom();
            }
        }
        let mut open = true;
        let mut action = None;
        let mut color = view.color;
        egui::Window::new("Edge Analysis")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "{} edges on {} objects",
                    view.displayed().count(),
                    view.sources
                ));
                ui.horizontal(|ui| {
                    for mode in [
                        viboceros_command::edge_analysis::Mode::All,
                        viboceros_command::edge_analysis::Mode::Naked,
                        viboceros_command::edge_analysis::Mode::NonManifold,
                    ] {
                        if ui
                            .selectable_label(view.mode == mode, mode.label())
                            .clicked()
                        {
                            action = Some(format!("ShowEdges Show={}", mode.label()));
                        }
                    }
                });
                if ui.color_edit_button_srgb(&mut color).changed() {
                    action = Some(format!(
                        "ShowEdges Color={},{},{}",
                        color[0], color[1], color[2]
                    ));
                }
                ui.horizontal(|ui| {
                    for (label, command) in [
                        ("All", "All"),
                        ("Current", "Current"),
                        ("Previous", "Previous"),
                        ("Next", "Next"),
                    ] {
                        if ui.button(label).clicked() {
                            action = Some(format!("ShowEdges Zoom {command}"));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Mark ends").clicked() {
                        action = Some("ShowEdges Mark".into());
                    }
                    if ui.button("Add selected").clicked() {
                        action = Some("ShowEdges Add".into());
                    }
                    if ui.button("Remove selected").clicked() {
                        action = Some("ShowEdges Remove".into());
                    }
                });
            });
        if !open {
            action = Some("ShowEdgesOff".into());
        }
        if let Some(action) = action {
            self.execute_command(&action);
        }
    }
}
