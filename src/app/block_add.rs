//! Target-instance and source-object getters for AddObjectsToBlock.
use super::*;
use blocks::PendingBlock;

impl VibocerosApp {
    pub(super) fn adding_to_block(&self) -> bool {
        matches!(
            (&self.block_session, self.active_command),
            (
                Some(PendingBlock::Add { .. }),
                Some(InteractiveCommand::AddObjectsToBlock)
            )
        )
    }
    pub(super) fn picking_block_add_target(&self) -> bool {
        self.adding_to_block()
            && matches!(
                self.block_session,
                Some(PendingBlock::Add { target: None, .. })
            )
    }
    pub(super) fn start_block_add_input(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.len() > 1 {
            return false;
        }
        let supplied_target = if let Some(word) = words.first() {
            let Ok(id) = word.parse::<ObjectId>() else {
                return false;
            };
            Some(id)
        } else {
            None
        };
        self.cancel_interactive_command(false);
        let target = supplied_target.or_else(|| {
            let selected = self.document.selected_objects().collect::<Vec<_>>();
            (selected.len() == 1 && matches!(selected[0].geometry(), Geometry::BlockInstance(_)))
                .then(|| selected[0].id())
        });
        let selection_before = self.document.selected_object_ids().collect::<Vec<_>>();
        self.active_command = Some(InteractiveCommand::AddObjectsToBlock);
        self.block_session = Some(PendingBlock::Add {
            target: None,
            selection_before,
        });
        self.command_input.clear();
        if let Some(target) = target {
            self.accept_block_add_target(target);
        } else {
            self.push_log("AddObjectsToBlock: pick an embedded block instance".into());
        }
        true
    }
    fn accept_block_add_target(&mut self, id: ObjectId) {
        if !self.document.is_object_selectable(id)
            || !self
                .document
                .object(id)
                .is_some_and(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
        {
            self.push_log("Pick a visible, unlocked block instance".into());
            return;
        }
        if let Some(PendingBlock::Add { target, .. }) = &mut self.block_session {
            *target = Some(id);
        }
        self.document.clear_selection();
        self.push_log(
            "AddObjectsToBlock: select objects to add; Enter accepts, Esc cancels".into(),
        );
    }
    pub(super) fn cancel_block_add_input(&mut self) {
        if let Some(PendingBlock::Add {
            selection_before, ..
        }) = &self.block_session
        {
            let ids = selection_before
                .iter()
                .copied()
                .filter(|id| self.document.is_object_selectable(*id))
                .collect::<Vec<_>>();
            let _ = self
                .document
                .select_objects_direct(ids, SelectionMode::Replace);
        }
    }
    pub(super) fn pick_block_add(&mut self, id: Option<ObjectId>, mode: SelectionMode) -> bool {
        if !self.adding_to_block() {
            return false;
        }
        if let Some(id) = id {
            if self.picking_block_add_target() {
                self.accept_block_add_target(id);
            } else if let Some(PendingBlock::Add {
                target: Some(target),
                ..
            }) = self.block_session
                && id != target
            {
                let mode = if mode == SelectionMode::Replace {
                    SelectionMode::Add
                } else {
                    mode
                };
                match self.document.select_object(id, mode) {
                    Ok(_) => {
                        let _ = self
                            .document
                            .select_objects_direct([target], SelectionMode::Remove);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
        }
        true
    }
    pub(super) fn select_block_add_region(&mut self, window: &SelectionWindow) -> bool {
        if !self.adding_to_block() {
            return false;
        }
        if let Some(PendingBlock::Add {
            target: Some(target),
            ..
        }) = self.block_session
        {
            let ids = window
                .object_ids
                .iter()
                .copied()
                .filter(|id| *id != target)
                .collect::<Vec<_>>();
            match self.document.select_objects(ids, window.mode) {
                Ok(_) => {
                    let _ = self
                        .document
                        .select_objects_direct([target], SelectionMode::Remove);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        true
    }
    pub(super) fn continue_block_add_input(&mut self, text: &str) -> bool {
        if !self.adding_to_block() {
            return false;
        }
        let word = text.trim().trim_start_matches('_');
        if self.commands.recognizes(word) || word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if self.picking_block_add_target() {
            if let Ok(id) = word.parse::<ObjectId>() {
                self.accept_block_add_target(id);
            } else {
                self.push_log("Pick a block instance or enter its object ID".into());
            }
        } else if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let Some(PendingBlock::Add {
                target: Some(target),
                ..
            }) = self.block_session
            else {
                unreachable!()
            };
            let sources = self
                .document
                .selected_object_ids()
                .filter(|id| *id != target)
                .collect::<Vec<_>>();
            match self.document.add_objects_to_block(target, sources) {
                Ok(count) => {
                    self.push_log(format!("Added {count} object(s) to the block definition"));
                    self.block_session = None;
                    self.cancel_interactive_command(false);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        } else {
            match word.parse::<ObjectId>() {
                Ok(id) => {
                    self.pick_block_add(Some(id), SelectionMode::Add);
                }
                Err(_) => self
                    .push_log("Select source objects or enter an object ID; Enter accepts".into()),
            }
        }
        self.command_input.clear();
        true
    }
}
