//! Block definition inspection and explicit document actions.
use super::*;
use viboceros_document::BlockDefinitionId;

#[derive(Default)]
pub(super) struct BlockManager {
    pub open: bool,
    selected: Option<BlockDefinitionId>,
    name: String,
    original_name: String,
    conflicted: bool,
    filter: String,
}

pub(super) enum Action {
    Select(BlockDefinitionId),
    Rename(BlockDefinitionId, String),
    Delete(BlockDefinitionId),
}

impl BlockManager {
    fn show(&mut self, ctx: &egui::Context, document: &Document) -> Vec<Action> {
        if !self.open {
            return Vec::new();
        }
        let mut actions = Vec::new();
        let info = document.block_definition_info();
        if self
            .selected
            .is_some_and(|id| document.block_definition(id).is_none())
        {
            self.selected = None;
            self.name.clear();
            self.original_name.clear();
            self.conflicted = false;
        }
        if let Some(id) = self.selected
            && let Some(definition) = document.block_definition(id)
        {
            if definition.name() != self.original_name {
                if self.name == self.original_name {
                    self.name = definition.name().into();
                }
                self.conflicted = self.name != definition.name();
                if !self.conflicted {
                    self.original_name = definition.name().into();
                }
            } else {
                self.conflicted = false;
            }
        }
        egui::Window::new("Block definitions")
            .open(&mut self.open)
            .default_width(600.)
            .show(ctx, |ui| {
                let mut rows = match info {
                    Ok(rows) => rows,
                    Err(error) => {
                        ui.label(error.to_string());
                        return;
                    }
                };
                rows.sort_by(|a, b| natural_compare(&a.name, &b.name));
                ui.horizontal(|ui| {
                    ui.label("Find");
                    ui.text_edit_singleline(&mut self.filter);
                });
                egui::ScrollArea::vertical()
                    .max_height(320.)
                    .show(ui, |ui| {
                        egui::Grid::new("block-definitions-table")
                            .striped(true)
                            .show(ui, |ui| {
                                for label in ["Name", "Objects", "Top level", "Nested", "Total"] {
                                    ui.strong(label);
                                }
                                ui.end_row();
                                for row in &rows {
                                    if !row
                                        .name
                                        .to_lowercase()
                                        .contains(&self.filter.to_lowercase())
                                    {
                                        continue;
                                    }
                                    if ui
                                        .selectable_label(self.selected == Some(row.id), &row.name)
                                        .clicked()
                                    {
                                        self.selected = Some(row.id);
                                        self.name = row.name.clone();
                            self.original_name=row.name.clone();self.conflicted=false;
                                    }
                                    ui.label(row.object_count.to_string());
                                    ui.label(row.top_level_instances.to_string());
                                    ui.label(row.nested_instances.to_string());
                                    ui.label(row.total_instances().to_string());
                                    ui.end_row();
                                }
                            });
                    });
                if rows.is_empty() {
                    ui.label("No block definitions. Use Block to create one.");
                }
                if let Some(row) = rows.iter().find(|r| Some(r.id) == self.selected) {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut self.name);
                        if ui
                            .add_enabled(
                            !self.conflicted && !self.name.trim().is_empty() && self.name.trim() != row.name,
                                egui::Button::new("Rename"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Rename(row.id, self.name.trim().to_owned()));
                        }
                    });
                if self.conflicted {ui.label("The definition was renamed elsewhere. Select it again to refresh the name.");}
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                row.top_level_instances > 0,
                                egui::Button::new("Select top-level instances"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Select(row.id));
                        }
                        let response = ui.add_enabled(
                            row.definition_references == 0,
                            egui::Button::new("Delete definition and instances"),
                        );
                        if row.definition_references > 0 {
                            response.clone().on_disabled_hover_text(
                                "This definition is used inside another block.",
                            );
                        }
                        if response.clicked() {
                            actions.push(Action::Delete(row.id));
                        }
                    });
                }
            });
        actions
    }
}

fn natural_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (start_a, start_b) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let aa = &a[start_a..i];
            let bb = &b[start_b..j];
            let aa = &aa[aa.iter().position(|v| *v != b'0').unwrap_or(aa.len() - 1)..];
            let bb = &bb[bb.iter().position(|v| *v != b'0').unwrap_or(bb.len() - 1)..];
            let ordering = aa.len().cmp(&bb.len()).then_with(|| aa.cmp(bb));
            if ordering != std::cmp::Ordering::Equal {
                return ordering;
            }
        } else {
            let ordering = a[i].cmp(&b[j]);
            if ordering != std::cmp::Ordering::Equal {
                return ordering;
            }
            i += 1;
            j += 1;
        }
    }
    a.len().cmp(&b.len())
}

impl VibocerosApp {
    pub(super) fn show_block_manager(&mut self, ctx: &egui::Context) {
        for action in self.block_manager.show(ctx, &self.document) {
            self.apply_block_manager_action(action);
        }
    }
    pub(super) fn apply_block_manager_action(&mut self, action: Action) {
        self.cancel_interactive_command(false);
        match action {
            Action::Select(id) => {
                let ids=self.document.objects().filter(|o|matches!(o.geometry(),Geometry::BlockInstance(i) if i.reference().definition()==id)&&self.document.is_object_selectable(o.id())).map(|o|o.id()).collect::<Vec<_>>();
                match self
                    .document
                    .select_objects_direct(ids, SelectionMode::Replace)
                {
                    Ok(count) => self.push_log(format!("Selected {count} block instance(s)")),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            Action::Rename(id, name) => {
                match edit_document_transaction(
                    &mut self.document,
                    "Rename block definition",
                    |d| d.rename_block_definition(id, &name),
                ) {
                    Ok(_) => self.push_log(format!("Renamed block to '{name}'")),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            Action::Delete(id) => match self.document.delete_block_definition_and_instances(id) {
                Ok(count) => {
                    self.push_log(format!("Deleted block definition and {count} instance(s)"))
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
        }
    }
}
