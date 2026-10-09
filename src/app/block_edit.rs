//! Persistent in-place edit controls, independent of each modelling getter.
use super::*;
use std::collections::BTreeSet;
impl VibocerosApp {
    pub(super) fn selection_preview_filter(
        &self,
    ) -> Option<viboceros_command::ObjectSelectionFilter> {
        self.object_prompt
            .as_ref()
            .filter(|prompt| prompt.special_selection.is_some())
            .map(|prompt| prompt.description.filter)
            .or_else(|| {
                self.picking_block_edit_sources()
                    .then_some(viboceros_command::ObjectSelectionFilter::BlockEditSources)
            })
    }
    pub(super) fn picking_block_edit_sources(&self) -> bool {
        self.active_command == Some(InteractiveCommand::BlockEditAdd)
            && matches!(
                self.block_session,
                Some(blocks::PendingBlock::EditAdd { .. })
            )
    }
    pub(super) fn block_edit_picked_sources(&self) -> Vec<ObjectId> {
        match &self.block_session {
            Some(blocks::PendingBlock::EditAdd { sources }) => sources.iter().copied().collect(),
            _ => Vec::new(),
        }
    }
    pub(super) fn try_start_block_edit_input(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.len() != 2
            || !words[0]
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("BlockEdit")
        {
            return false;
        }
        let operation = words[1].trim_start_matches('_');
        if !operation.eq_ignore_ascii_case("AddObject")
            && !operation.eq_ignore_ascii_case("SetBasePoint")
        {
            return false;
        }
        self.cancel_interactive_command(false);
        if !self.document.is_block_editing() {
            self.push_log("Error: no block edit is open".into());
            return true;
        }
        if operation.eq_ignore_ascii_case("AddObject") {
            self.block_session = Some(blocks::PendingBlock::EditAdd {
                sources: BTreeSet::new(),
            });
            self.active_command = Some(InteractiveCommand::BlockEditAdd);
        } else {
            self.block_session = Some(blocks::PendingBlock::EditBasePoint);
            self.active_command = Some(InteractiveCommand::BlockEditBasePoint);
            self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        }
        self.command_input.clear();
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }
    pub(super) fn pick_block_edit_sources(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> bool {
        if !self.picking_block_edit_sources() {
            return false;
        }
        let ids = ids.into_iter().collect::<Vec<_>>();
        if ids
            .iter()
            .any(|id| !self.document.is_block_edit_add_candidate(*id))
        {
            self.push_log("Choose visible, unlocked objects outside this block edit".into());
            return true;
        }
        let Some(blocks::PendingBlock::EditAdd { sources }) = &mut self.block_session else {
            unreachable!()
        };
        match mode {
            SelectionMode::Replace | SelectionMode::Add => sources.extend(ids),
            SelectionMode::Remove => {
                for id in ids {
                    sources.remove(&id);
                }
            }
            SelectionMode::Toggle => {
                for id in ids {
                    if !sources.remove(&id) {
                        sources.insert(id);
                    }
                }
            }
        }
        true
    }
    pub(super) fn continue_block_edit_input(&mut self, input: &str) -> bool {
        if !matches!(
            self.active_command,
            Some(InteractiveCommand::BlockEditAdd | InteractiveCommand::BlockEditBasePoint)
        ) {
            return false;
        }
        let text = input.trim();
        if text.split_whitespace().next().is_some_and(|word| {
            self.commands.recognizes(word)
                && !matches!(
                    word.trim_start_matches('_').to_ascii_lowercase().as_str(),
                    "selall" | "selnone"
                )
                || word
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Cancel")
        }) {
            return false;
        }
        if self.picking_block_edit_sources() {
            if text.is_empty() || text.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
                let ids = self.block_edit_picked_sources();
                if ids.is_empty() {
                    self.push_log("Select at least one external object; Esc cancels".into());
                } else {
                    match self.document.add_objects_to_block_edit(ids) {
                        Ok(ids) => {
                            self.push_log(format!(
                                "Copied {} object(s) into the block edit",
                                ids.len()
                            ));
                            self.cancel_interactive_command(false);
                        }
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                }
            } else if text.trim_start_matches('_').eq_ignore_ascii_case("SelAll") {
                let ids = self
                    .document
                    .block_edit_add_candidates()
                    .collect::<Vec<_>>();
                self.pick_block_edit_sources(ids, SelectionMode::Add);
            } else if text.trim_start_matches('_').eq_ignore_ascii_case("SelNone") {
                if let Some(blocks::PendingBlock::EditAdd { sources }) = &mut self.block_session {
                    sources.clear();
                }
            } else {
                match text
                    .split_whitespace()
                    .map(str::parse::<ObjectId>)
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(ids) => {
                        self.pick_block_edit_sources(ids, SelectionMode::Add);
                    }
                    Err(_) => self.push_log(
                        "Pick external objects or enter object IDs; Enter accepts".into(),
                    ),
                }
            }
        } else if text.is_empty() {
            self.push_log("Pick or enter a new base point; Esc cancels".into());
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }
    pub(super) fn accept_block_edit_base_point(&mut self, point: Point3) -> bool {
        match self.document.set_block_edit_base_point(point) {
            Ok(_) => {
                self.last_point = Some(point);
                self.push_log("Updated block-edit base point".into());
                self.cancel_interactive_command(false);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
    pub(super) fn try_open_block_edit_double_click(&mut self, id: ObjectId) -> bool {
        if self.document.is_block_editing()
            || self.active_command.is_some()
            || self.object_prompt.is_some()
            || self.group_prompt.is_some()
            || self.plane_prompt.is_some()
            || self.intersection_prompt.is_some()
            || self.tween_surfaces_prompt.is_some()
            || self.boolean_two_prompt.is_some()
            || self.planar_boolean_prompt.is_some()
            || self.edge_prompt.is_some()
            || self.hole_prompt.is_some()
            || self.unjoin_prompt.is_some()
            || self.set_view_prompt.is_some()
            || self.end_analysis_pick.is_some()
            || self.remember_copy_prompt
            || self.selection_menu.is_some()
            || self.selection_window_override.is_some()
            || self.circular_selection.is_some()
            || self.boundary_selection.is_some()
            || self.fence_selection.is_some()
            || self.lasso_selection.is_some()
        {
            return false;
        }
        if !self.document.is_object_selectable(id)
            || !self
                .document
                .object(id)
                .is_some_and(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
        {
            return false;
        }
        self.execute_command(&format!("BlockEdit Open {id}"));
        true
    }
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
                if ui.button("Pick objects to add").clicked() {
                    action = Some("BlockEdit AddObject".to_owned());
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
                if ui.button("Pick base point").clicked() {
                    action = Some("BlockEdit SetBasePoint".to_owned());
                }
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
