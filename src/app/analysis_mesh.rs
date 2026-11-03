//! Shared analysis mesh controls keep modeling getters active.
use super::*;

impl VibocerosApp {
    pub(super) fn show_analysis_mesh_controls(&mut self, ui: &mut egui::Ui, command: &str) {
        let density = self.document.analysis_subd_density();
        let mut level = density.level();
        let mut adaptive = density.is_adaptive();
        egui::CollapsingHeader::new("Adjust SubD mesh")
            .default_open(true)
            .show(ui, |ui| {
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
                }
            });
        if level != density.level() || adaptive != density.is_adaptive() {
            self.set_analysis_display(&format!(
                "{command} SubDLevel={level} SubDAdaptive={}",
                if adaptive { "Yes" } else { "No" }
            ));
        }
    }
}
