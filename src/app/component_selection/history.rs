//! Result-component replay keyed by stable, bounded document history entries.
use super::*;

#[derive(Clone, Debug)]
pub(in crate::app) struct ComponentReplay {
    owners: std::sync::Arc<[ObjectId]>,
    faces: std::sync::Arc<[ComponentPick]>,
}

impl ComponentSelection {
    pub(in crate::app) fn remember_merge_result(
        &mut self,
        document: &Document,
        owners: Vec<ObjectId>,
        faces: Vec<ComponentPick>,
    ) {
        self.history.retain(|id, _| document.has_history_entry(*id));
        if let Some(id) = document.undo_entry_id() {
            self.history.insert(
                id,
                ComponentReplay {
                    owners: owners.into(),
                    faces: faces.into(),
                },
            );
        }
    }
    pub(in crate::app) fn history_action(
        &self,
        document: &Document,
        input: &str,
    ) -> Option<(ComponentReplay, bool)> {
        let name = input.trim().trim_start_matches(['_', '-']);
        let redo = name.eq_ignore_ascii_case("Redo");
        let id = if redo {
            document.redo_entry_id()
        } else if name.eq_ignore_ascii_case("Undo") {
            document.undo_entry_id()
        } else {
            None
        }?;
        self.history
            .get(&id)
            .cloned()
            .map(|selection| (selection, redo))
    }
    pub(in crate::app) fn replay_history(
        &mut self,
        document: &Document,
        action: (ComponentReplay, bool),
    ) -> Result<(), &'static str> {
        let (selection, redo) = action;
        self.release_objects(&selection.owners);
        if redo {
            self.apply(document, selection.faces.to_vec(), ComponentAction::Add)?;
        }
        self.history.retain(|id, _| document.has_history_entry(*id));
        Ok(())
    }
    pub(in crate::app) fn accept_merge_components(
        &mut self,
        document: &Document,
        owners: &[ObjectId],
        picks: Vec<ComponentPick>,
    ) -> Result<(), &'static str> {
        self.release_objects(owners);
        self.apply(document, picks, ComponentAction::Add)
    }
}
