//! Nonmodal false-color analysis controls use the command registry's state.
use super::*;
mod direction;
pub(super) use direction::Source;

impl VibocerosApp {
    pub(super) fn show_draft_angle_analysis(&mut self, root: &mut egui::Ui) {
        let Some(view) = self.commands.draft_angle_analysis_view(&self.document) else {
            return;
        };
        let mut options = view.options;
        let mut open = true;
        let mut action = None;
        let mut source = self.draft_pull_source.clone();
        let mut set_direction = false;
        egui::Window::new("Draft Angle Analysis")
            .open(&mut open)
            .show(root.ctx(), |ui| {
                ui.label(format!("{} objects", view.sources));
                ui.horizontal(|ui| {
                    ui.label("Lower (red)");
                    ui.add(
                        egui::DragValue::new(&mut options.range[0])
                            .range(-90.0..=90.0)
                            .speed(0.1)
                            .suffix("°"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Upper (blue)");
                    ui.add(
                        egui::DragValue::new(&mut options.range[1])
                            .range(-90.0..=90.0)
                            .speed(0.1)
                            .suffix("°"),
                    );
                });
                ui.label("Set direction by");
                egui::ComboBox::from_id_salt("draft_pull_source")
                    .selected_text(source.label())
                    .show_ui(ui, |ui| {
                        for choice in Source::STANDARD {
                            ui.selectable_value(&mut source, choice.clone(), choice.label());
                        }
                        for name in self.named_cplanes.names() {
                            ui.selectable_value(
                                &mut source,
                                Source::NamedCPlane(name.to_owned()),
                                name,
                            );
                        }
                    });
                if ui.button("Set direction…").clicked() {
                    set_direction = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Pull vector");
                    for c in &mut options.up {
                        ui.add(egui::DragValue::new(c).speed(0.05));
                    }
                });
                ui.checkbox(&mut options.show_isocurves, "Show edges and isocurves");
                ui.horizontal(|ui| {
                    if ui.button("Add objects").clicked() {
                        action = Some("DraftAngleAnalysis Add");
                    }
                    if ui.button("Remove objects").clicked() {
                        action = Some("DraftAngleAnalysis Remove");
                    }
                });
            });
        if !open {
            if self.picking_draft_pull() {
                self.cancel_interactive_command(false);
            }
            self.set_analysis_display("DraftAngleAnalysisOff");
        } else if options != view.options {
            self.set_analysis_display(&format!(
                "DraftAngleAnalysis Range={},{} Up={},{},{} ShowIsoCurves={}",
                options.range[0],
                options.range[1],
                options.up[0],
                options.up[1],
                options.up[2],
                if options.show_isocurves { "Yes" } else { "No" }
            ));
        }
        if open {
            let picker = matches!(source, Source::TwoPoints | Source::Curve);
            if set_direction || source != self.draft_pull_source && !picker {
                self.set_draft_pull_source(source);
            } else {
                self.draft_pull_source = source;
            }
        }
        if let Some(action) = action {
            self.cancel_interactive_command(false);
            self.command_input = action.into();
            self.run_command();
        }
    }
}
