//! Persistent in-place edit controls, independent of each modelling getter.
use super::*;
impl VibocerosApp {
    pub(super) fn show_block_edit(&mut self, context: &egui::Context) {
        if !self.document.is_block_editing() {
            return;
        }
        let source_key = egui::Id::new("block-edit-add-source");
        let point_key = egui::Id::new("block-edit-base-input");
        let mut source = context.data_mut(|d| d.get_temp::<ObjectId>(source_key));
        let base = self.document.block_edit_base_point().unwrap();
        let mut point_text = context
            .data_mut(|d| d.get_temp::<String>(point_key))
            .unwrap_or_else(|| format!("{},{},{}", base.x(), base.y(), base.z()));
        let candidates = self
            .document
            .block_edit_add_candidate_objects()
            .map(|object| (object.id(), object.attributes().name().unwrap_or("Object")))
            .collect::<Vec<_>>();
        if source.is_some_and(|id| !candidates.iter().any(|(candidate, _)| *candidate == id)) {
            source = None;
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new("Block edit")
            .open(&mut open)
            .collapsible(false)
            .show(context, |ui| {
                ui.label("Edit the exposed members with ordinary commands.");
                ui.label(format!(
                    "{} editable object(s)",
                    self.document.block_edit_objects().len()
                ));
                egui::ComboBox::from_id_salt("block-edit-source")
                    .selected_text(
                        source
                            .and_then(|id| {
                                candidates
                                    .iter()
                                    .find(|(candidate, _)| *candidate == id)
                                    .map(|(_, name)| format!("{name} ({id})"))
                            })
                            .unwrap_or_else(|| "Choose model object".to_owned()),
                    )
                    .show_ui(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(200.).show_rows(
                            ui,
                            ui.spacing().interact_size.y,
                            candidates.len(),
                            |ui, rows| {
                                for row in rows {
                                    let (id, name) = candidates[row];
                                    ui.selectable_value(
                                        &mut source,
                                        Some(id),
                                        format!("{name} ({id})"),
                                    );
                                }
                            },
                        );
                    });
                if ui
                    .add_enabled(source.is_some(), egui::Button::new("Add Object"))
                    .clicked()
                {
                    action = Some(format!("BlockEdit AddObject {}", source.unwrap()));
                }
                if ui.button("Remove Object").clicked() {
                    action = Some("BlockEdit RemoveObject".to_owned());
                }
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut point_text);
                    if ui.button("Set Base Point").clicked() {
                        action = Some(format!("BlockEdit SetBasePoint {point_text}"));
                    }
                });
                ui.separator();
                if ui.button("Save and close").clicked() {
                    action = Some("BlockEdit SaveAndClose".to_owned());
                }
                if ui.button("Discard and cancel").clicked() {
                    action = Some("BlockEdit DiscardAndCancel".to_owned());
                }
            });
        if !open {
            action = Some("BlockEdit DiscardAndCancel".to_owned());
        }
        context.data_mut(|d| {
            if let Some(id) = source {
                d.insert_temp(source_key, id);
            } else {
                d.remove::<ObjectId>(source_key);
            }
            d.insert_temp(point_key, point_text);
        });
        if let Some(action) = action {
            self.execute_command(&action);
            if !self.document.is_block_editing() {
                context.data_mut(|d| {
                    d.remove::<ObjectId>(source_key);
                    d.remove::<String>(point_key);
                });
            }
        }
    }
}
