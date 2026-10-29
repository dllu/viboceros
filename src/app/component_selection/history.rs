//! Result-component replay keyed by stable, bounded document history entries.
use super::*;

#[derive(Clone, Debug)]
pub(in crate::app) struct ComponentReplay {
    owners: std::sync::Arc<[ObjectId]>,
    faces: std::sync::Arc<[ComponentPick]>,
    witnesses: std::sync::Arc<[(ObjectId, GeometrySnapshot)]>,
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
                    witnesses: owners
                        .iter()
                        .filter_map(|&id| {
                            document
                                .object(id)
                                .map(|o| (id, o.geometry_snapshot().clone()))
                        })
                        .collect::<Vec<_>>()
                        .into(),
                    owners: owners.into(),
                    faces: faces.into(),
                },
            );
        }
    }
    pub(super) fn invalidate_result_picks(
        &mut self,
        removed: &BTreeMap<ComponentPick, GeometrySnapshot>,
    ) {
        if removed.is_empty() {
            return;
        }
        for selection in self.history.values_mut() {
            let keep = |pick: &ComponentPick| {
                removed.get(pick).is_none_or(|snapshot| {
                    !selection
                        .witnesses
                        .iter()
                        .any(|(id, w)| *id == pick.object && w.shares_storage_with(snapshot))
                })
            };
            if selection.faces.iter().any(|p| !keep(p)) {
                selection.faces = selection
                    .faces
                    .iter()
                    .copied()
                    .filter(keep)
                    .collect::<Vec<_>>()
                    .into();
            }
        }
    }
    pub(in crate::app) fn history_action(
        &mut self,
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
        if !redo && self.history.contains_key(&id) {
            // Only original result faces still selected on the live output
            // survive Redo. Later picks do not become command results.
            let live = self
                .valid_picks(document)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            let selection = self.history.get_mut(&id).unwrap();
            selection.faces = selection
                .faces
                .iter()
                .copied()
                .filter(|p| live.contains(p))
                .collect::<Vec<_>>()
                .into();
        }
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
        self.release_objects_impl(&selection.owners, false);
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
