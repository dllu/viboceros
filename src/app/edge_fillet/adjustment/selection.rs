//! Handle selections stay in the size adjustment stage.
use super::*;

#[derive(Default)]
pub(super) struct HandleSelection {
    locations: BTreeMap<ComponentPick, Vec<HandleLocation>>,
}

impl HandleSelection {
    pub(super) fn locations(&self, pick: ComponentPick) -> &[HandleLocation] {
        self.locations.get(&pick).map_or(&[], Vec::as_slice)
    }

    pub(super) fn contains(&self, id: SizeHandleId) -> bool {
        self.locations(id.pick).contains(&id.location)
    }

    fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    fn len(&self) -> usize {
        self.locations.values().map(Vec::len).sum()
    }

    pub(super) fn clear(&mut self) {
        self.locations.clear();
    }

    pub(super) fn update(
        &mut self,
        ids: impl IntoIterator<Item = SizeHandleId>,
        mode: SelectionMode,
    ) {
        if mode == SelectionMode::Replace {
            self.clear();
        }
        for id in ids {
            let locations = self.locations.entry(id.pick).or_default();
            if let Some(index) = locations
                .iter()
                .position(|&location| location == id.location)
            {
                if matches!(mode, SelectionMode::Remove | SelectionMode::Toggle) {
                    locations.remove(index);
                    if locations.is_empty() {
                        self.locations.remove(&id.pick);
                    }
                }
            } else if mode == SelectionMode::Remove {
                if locations.is_empty() {
                    self.locations.remove(&id.pick);
                }
            } else {
                locations.push(id.location);
            }
        }
    }
}

impl VibocerosApp {
    pub(super) fn select_edge_size_handles(
        &mut self,
        ids: Vec<SizeHandleId>,
        mode: SelectionMode,
    ) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt.filter(|p| p.adjusting) else {
            return false;
        };
        let Ok(picks) = self
            .component_selection
            .checked_picks(self.edge_blend_source_document())
        else {
            return false;
        };
        let picks = picks.into_iter().collect::<BTreeSet<_>>();
        let selection = &mut self.edge_blend_adjustment.selection;
        if ids.is_empty()
            && (mode != SelectionMode::Replace
                || selection.is_empty() && self.edge_blend_adjustment.active.is_none())
        {
            return false;
        }
        if selection.is_empty()
            && mode != SelectionMode::Replace
            && let Some((id, _)) = &self.edge_blend_adjustment.active
        {
            selection.update([*id], SelectionMode::Add);
        }
        selection.update(ids.into_iter().filter(|id| picks.contains(&id.pick)), mode);
        let count = selection.len();
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        if count == 0 {
            self.log_edge_blend_prompt();
        } else {
            self.push_log(format!(
                "Selected {count} size handle(s). Click or drag a selected handle to change the size."
            ));
        }
        true
    }
}
