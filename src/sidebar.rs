use std::collections::BTreeMap;

use eframe::egui::{self, Color32, RichText};
use viboceros_document::{ColorRgb, Document, GroupId, LayerId};

pub(crate) enum SidebarAction {
    AddLayer {
        name: String,
    },
    EditLayer {
        id: LayerId,
        old_name: String,
        name: String,
        color: ColorRgb,
    },
    DeleteLayer {
        id: LayerId,
        name: String,
    },
    SetCurrent {
        id: LayerId,
        name: String,
    },
    SetVisibility {
        id: LayerId,
        name: String,
        visible: bool,
    },
    SetLocked {
        id: LayerId,
        name: String,
        locked: bool,
    },
    RemoveGroup {
        id: GroupId,
        name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayerEditor {
    id: LayerId,
    original_name: String,
    original_color: [u8; 3],
    name: String,
    color: [u8; 3],
}

impl LayerEditor {
    fn new(id: LayerId, name: String, color: ColorRgb) -> Self {
        let color = [color.red, color.green, color.blue];
        Self {
            id,
            original_name: name.clone(),
            original_color: color,
            name,
            color,
        }
    }

    fn has_valid_changes(&self) -> bool {
        let name = self.name.trim();
        !name.is_empty() && (name != self.original_name || self.color != self.original_color)
    }

    fn resolved_color(&self) -> ColorRgb {
        ColorRgb::new(self.color[0], self.color[1], self.color[2])
    }
}

#[derive(Default)]
pub(crate) struct DocumentSidebar {
    new_layer_name: String,
    layer_editor: Option<LayerEditor>,
}

impl DocumentSidebar {
    pub(crate) fn show(&mut self, root: &mut egui::Ui, document: &Document) -> Vec<SidebarAction> {
        let mut actions = Vec::new();
        let mut cancel_editor = None;
        egui::Panel::right("layers")
            .default_size(280.0)
            .show(root, |ui| {
                ui.heading("Layers");
                ui.separator();
                let current = document.current_layer_id();
                let object_counts = document.objects().fold(
                    BTreeMap::<LayerId, usize>::new(),
                    |mut counts, object| {
                        *counts.entry(object.attributes().layer_id()).or_default() += 1;
                        counts
                    },
                );
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.small("On");
                    ui.small("Lock");
                    ui.small("Layer · objects");
                });
                for layer in document.layers() {
                    let id = layer.id();
                    let name = layer.name();
                    let color = layer.color();
                    let visible = layer.is_visible();
                    let locked = layer.is_locked();
                    let object_count = object_counts.get(&id).copied().unwrap_or_default();
                    ui.horizontal(|ui| {
                        let mut new_visible = visible;
                        let visibility = ui
                            .add_enabled_ui(id != current, |ui| ui.checkbox(&mut new_visible, ""))
                            .inner
                            .on_hover_text(if id == current {
                                "The current layer must remain visible"
                            } else {
                                "Show or hide this layer"
                            });
                        if visibility.changed() {
                            actions.push(SidebarAction::SetVisibility {
                                id,
                                name: name.to_owned(),
                                visible: new_visible,
                            });
                        }

                        let mut new_locked = locked;
                        let lock = ui
                            .add_enabled_ui(id != current, |ui| ui.checkbox(&mut new_locked, ""))
                            .inner
                            .on_hover_text(if id == current {
                                "The current layer must remain unlocked"
                            } else {
                                "Lock or unlock this layer"
                            });
                        if lock.changed() {
                            actions.push(SidebarAction::SetLocked {
                                id,
                                name: name.to_owned(),
                                locked: new_locked,
                            });
                        }

                        let swatch = Color32::from_rgb(color.red, color.green, color.blue);
                        ui.label(RichText::new("●").color(swatch))
                            .on_hover_text(format!(
                                "RGB {}, {}, {}",
                                color.red, color.green, color.blue
                            ));
                        let select = ui
                            .add_enabled_ui(visible && !locked, |ui| {
                                ui.selectable_label(id == current, name)
                            })
                            .inner;
                        if select.clicked() {
                            actions.push(SidebarAction::SetCurrent {
                                id,
                                name: name.to_owned(),
                            });
                        }
                        ui.small(format!("{object_count}"))
                            .on_hover_text(format!("{object_count} object(s) on this layer"));
                        if ui.small_button("Edit").clicked() {
                            self.layer_editor = Some(LayerEditor::new(id, name.to_owned(), color));
                        }
                        let can_delete = id != current && object_count == 0;
                        let delete_help = if id == current {
                            "The current layer cannot be deleted"
                        } else if object_count != 0 {
                            "Move or delete this layer's objects before deleting it"
                        } else {
                            "Delete this empty layer"
                        };
                        if ui
                            .add_enabled(can_delete, egui::Button::new("×").small())
                            .on_hover_text(delete_help)
                            .clicked()
                        {
                            actions.push(SidebarAction::DeleteLayer {
                                id,
                                name: name.to_owned(),
                            });
                        }
                    });

                    if self
                        .layer_editor
                        .as_ref()
                        .is_some_and(|editor| editor.id == id)
                    {
                        let editor = self
                            .layer_editor
                            .as_mut()
                            .expect("the layer editor id was checked");
                        ui.indent(("layer_editor", id), |ui| {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Name");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut editor.name)
                                            .desired_width(150.0),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Color");
                                    ui.color_edit_button_srgb(&mut editor.color);
                                    ui.small(format!(
                                        "{}, {}, {}",
                                        editor.color[0], editor.color[1], editor.color[2]
                                    ));
                                });
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            editor.has_valid_changes(),
                                            egui::Button::new("Apply"),
                                        )
                                        .clicked()
                                    {
                                        actions.push(SidebarAction::EditLayer {
                                            id,
                                            old_name: editor.original_name.clone(),
                                            name: editor.name.trim().to_owned(),
                                            color: editor.resolved_color(),
                                        });
                                    }
                                    if ui.button("Cancel").clicked() {
                                        cancel_editor = Some(id);
                                    }
                                });
                            });
                        });
                    }
                }
                ui.add_space(8.0);
                ui.label(RichText::new("New layer").strong());
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.new_layer_name)
                            .desired_width(175.0)
                            .hint_text("Layer name"),
                    );
                    let valid_name = !self.new_layer_name.trim().is_empty();
                    let add_clicked = ui
                        .add_enabled(valid_name, egui::Button::new("Add"))
                        .clicked();
                    let enter_pressed = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if valid_name && (add_clicked || enter_pressed) {
                        actions.push(SidebarAction::AddLayer {
                            name: self.new_layer_name.trim().to_owned(),
                        });
                    }
                });

                ui.add_space(14.0);
                ui.heading("Groups");
                ui.separator();
                if document.groups().len() == 0 {
                    ui.weak("No groups");
                }
                for (index, group) in document.groups().enumerate() {
                    let id = group.id();
                    let name = group
                        .name()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("Group {}", index + 1));
                    let members = group.members().len();
                    ui.horizontal(|ui| {
                        ui.label(format!("{name} · {members}"))
                            .on_hover_text(format!("Group {id} with {members} object(s)"));
                        if ui.small_button("×").on_hover_text("Ungroup").clicked() {
                            actions.push(SidebarAction::RemoveGroup {
                                id,
                                name: name.clone(),
                            });
                        }
                    });
                }
                ui.add_space(8.0);
                ui.small("Create a group with: Group [name]");
            });

        if cancel_editor.is_some_and(|id| {
            self.layer_editor
                .as_ref()
                .is_some_and(|editor| editor.id == id)
        }) {
            self.layer_editor = None;
        }
        actions
    }

    pub(crate) fn clear_new_layer_name(&mut self) {
        self.new_layer_name.clear();
    }

    pub(crate) fn close_layer_editor(&mut self, id: LayerId) {
        if self
            .layer_editor
            .as_ref()
            .is_some_and(|editor| editor.id == id)
        {
            self.layer_editor = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_new_layer_name(&mut self, name: impl Into<String>) {
        self.new_layer_name = name.into();
    }

    #[cfg(test)]
    pub(crate) fn new_layer_name(&self) -> &str {
        &self.new_layer_name
    }
}
