//! Compact Rebuild option controls route through the typed command handlers.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn show_rebuild_choices(&mut self, ui: &mut egui::Ui) {
        self.validate_rebuild_preview();
        let Some(preview) = self.rebuild_preview.as_ref() else {
            return;
        };
        let Some(prompt) = self.object_prompt.as_ref() else {
            return;
        };
        let options = preview.options;
        let phase = prompt.phase;
        let ready = preview.prepared.is_some() && preview.scene.is_some();
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            if let ObjectPromptPhase::RebuildValue(name) = phase {
                if matches!(
                    name,
                    "DeleteInput" | "PreserveTangents" | "PreserveEndTangents" | "ReTrim"
                ) {
                    for label in ["Yes", "No"] {
                        if ui.button(label).clicked() {
                            chosen = Some(label.to_owned());
                        }
                    }
                } else if name == "OutputLayer" {
                    for label in ["Input", "Current"] {
                        if ui.button(label).clicked() {
                            chosen = Some(label.to_owned());
                        }
                    }
                }
                if ui.button("Keep value").clicked() {
                    chosen = Some(String::new());
                }
            } else {
                let mut button = |label: String, input: String| {
                    if ui.button(label).clicked() {
                        chosen = Some(input);
                    }
                };
                match options {
                    Options::Curve(o) => {
                        button(format!("PointCount={}", o.point_count), "PointCount".into());
                        button(format!("Degree={}", o.degree), "Degree".into());
                        button(
                            format!(
                                "PreserveTangents={}",
                                if o.preserve_end_tangents { "Yes" } else { "No" }
                            ),
                            format!(
                                "PreserveTangents={}",
                                if o.preserve_end_tangents { "No" } else { "Yes" }
                            ),
                        );
                        button(
                            format!("DeleteInput={}", if o.delete_input { "Yes" } else { "No" }),
                            format!("DeleteInput={}", if o.delete_input { "No" } else { "Yes" }),
                        );
                        let current = o.output_layer == curve_rebuild::OutputLayer::Current;
                        button(
                            format!("OutputLayer={}", if current { "Current" } else { "Input" }),
                            format!("OutputLayer={}", if current { "Input" } else { "Current" }),
                        );
                    }
                    Options::Surface(o) => {
                        for (axis, prefix) in ["U", "V"].into_iter().enumerate() {
                            button(
                                format!("{prefix}PointCount={}", o.count[axis]),
                                format!("{prefix}PointCount"),
                            );
                            button(
                                format!("{prefix}Degree={}", o.degree[axis]),
                                format!("{prefix}Degree"),
                            );
                        }
                        for (name, value) in [("DeleteInput", o.delete), ("ReTrim", o.retrim)] {
                            button(
                                format!("{name}={}", if value { "Yes" } else { "No" }),
                                format!("{name}={}", if value { "No" } else { "Yes" }),
                            );
                        }
                        button(
                            format!(
                                "OutputLayer={}",
                                if o.current { "Current" } else { "Input" }
                            ),
                            format!(
                                "OutputLayer={}",
                                if o.current { "Input" } else { "Current" }
                            ),
                        );
                    }
                }
                if ui.button("Preview").clicked() {
                    chosen = Some("Preview".into());
                }
                if ui.add_enabled(ready, egui::Button::new("Accept")).clicked() {
                    chosen = Some(String::new());
                }
            }
            if ui.button("Cancel").clicked() {
                chosen = Some("Cancel".into());
            }
        });
        if let Some(input) = chosen {
            self.continue_rebuild_options(&input);
            self.command_focus_requested = true;
        }
    }
}
