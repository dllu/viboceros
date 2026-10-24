//! Ordered Boolean picks and cancellation history.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn select_boolean_solids_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let Some(prompt) = &self.intersection_prompt else {
            return;
        };
        let first = prompt
            .first
            .iter()
            .flatten()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let shared_sets = prompt.boolean.as_ref().unwrap().kind.shares_sets();
        let eligible = self
            .document
            .selectable_objects()
            .filter(|o| {
                prompt
                    .boolean
                    .as_ref()
                    .unwrap()
                    .kind
                    .filter()
                    .accepts_object(o)
            })
            .map(|o| o.id())
            .collect::<std::collections::BTreeSet<_>>();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        // Add separately: bulk document selection sorts IDs, but metadata follows pick order.
        for id in ids {
            if eligible.contains(&id)
                && (shared_sets || !first.contains(&id))
                && let Err(error) = self.document.select_objects_direct([id], mode)
            {
                self.push_log(format!("Error: {error}"));
            }
        }
    }

    pub(in crate::app) fn cancel_boolean_solids_prompt(
        &mut self,
        options: BooleanOptions,
        first: Option<Vec<ObjectId>>,
        announce: bool,
    ) {
        let name = options.kind.name();
        if options.kind.mesh_tolerance().is_some()
            && let Some(first) = &first
        {
            // The live cancelled getter retains its first set, but
            // native Undo/Redo treats those picks as transient. A
            // marker-only transaction folds into the preceding entry.
            let result = (|| {
                self.document.begin_transaction(name)?;
                match self
                    .document
                    .release_command_selection_on_history_replay(first.iter().copied())
                {
                    Ok(()) => self.document.commit_transaction().map(|_| ()),
                    Err(error) => {
                        let _ = self.document.rollback_transaction();
                        Err(error)
                    }
                }
            })();
            if let Err(error) = result {
                self.push_log(format!("Error: {error}"));
            }
        }
        let _ = self
            .document
            .select_command_results(if options.kind.shares_sets() {
                vec![]
            } else {
                first.unwrap_or_default()
            });
        if announce {
            self.push_log(format!("Cancelled {}", options.kind.name()));
        }
    }
}
