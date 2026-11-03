//! Shared analysis mesh controls keep modeling getters active.
use super::*;

impl VibocerosApp {
    pub(super) fn show_analysis_mesh_controls(&mut self, ui: &mut egui::Ui, command: &str) {
        let density = self.document.analysis_subd_density();
        let mut level = density.level();
        let mut adaptive = density.is_adaptive();
        let mut nurbs = self.document.analysis_nurbs_density().value();
        let mut simple = self.document.analysis_simple_planes();
        let mut defaults = false;
        egui::CollapsingHeader::new("Adjust analysis mesh")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut nurbs, 0.0..=1.0).text("NURBS density"));
                ui.checkbox(&mut simple, "Simple planes");
                ui.add(
                    egui::Slider::new(&mut level, 1..=5)
                        .clamping(egui::SliderClamping::Never)
                        .text("Subdivision level"),
                );
                ui.checkbox(&mut adaptive, "Adaptive subdivision")
                    .on_hover_text("Reduce subdivision for large control nets.");
                if ui.button("Restore defaults").clicked() {
                    level = 4;
                    adaptive = true;
                    nurbs = 0.8;
                    simple = false;
                    defaults = true;
                }
            });
        if defaults
            || simple != self.document.analysis_simple_planes()
            || nurbs != self.document.analysis_nurbs_density().value()
            || level != density.level()
            || adaptive != density.is_adaptive()
        {
            self.set_analysis_display(&format!(
                "{command} {}NurbsDensity={nurbs} SimplePlanes={} SubDLevel={level} SubDAdaptive={}",
                if defaults { "MeshDefaults=Yes " } else { "" },
                if simple { "Yes" } else { "No" },
                if adaptive { "Yes" } else { "No" }
            ));
        }
    }
}
