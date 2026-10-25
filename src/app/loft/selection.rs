use super::*;

impl VibocerosApp {
    pub(in crate::app) fn select_loft_curves(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(p) = self.loft_prompt.as_ref() else {
            return false;
        };
        if !p.selecting() {
            return true;
        }
        let mut sources = p.state.sources.clone();
        for id in ids {
            let Some(object) = self.document.object(id).filter(|o| {
                self.document.is_object_selectable(id) && o.geometry().curve_ref().is_some()
            }) else {
                continue;
            };
            let index = sources.iter().position(|(selected, _)| *selected == id);
            match (mode, index) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(index)) => {
                    sources.remove(index);
                }
                (SelectionMode::Remove, None) | (_, Some(_)) => {}
                (_, None) => sources.push((id, object.geometry_snapshot().clone())),
            }
        }
        let ends = usize::from(p.state.options.ends.start.is_some())
            + usize::from(p.state.options.ends.end.is_some());
        if sources.len() + ends > viboceros_geometry::MAX_LOFT_SECTIONS {
            self.push_log("Too many Loft sections (limit 256 including point ends)".into());
            return true;
        }
        if sources
            .iter()
            .map(|(id, _)| id)
            .ne(p.state.sources.iter().map(|(id, _)| id))
        {
            let p = self.loft_prompt.as_mut().unwrap();
            p.undo.push(p.state.clone());
            p.state.sources = sources;
        }
        self.sync_loft_selection();
        self.log_loft();
        true
    }

    pub(in crate::app) fn sync_loft_selection(&mut self) {
        let ids = self
            .loft_prompt
            .as_ref()
            .unwrap()
            .state
            .sources
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        self.restore_loft_selection(ids);
    }

    pub(in crate::app) fn restore_loft_selection(&mut self, ids: Vec<ObjectId>) {
        if self.document.selected_object_ids().eq(ids.iter().copied()) {
            return;
        }
        self.document.clear_selection();
        for id in ids {
            if self.document.is_object_selectable(id) {
                let _ = self
                    .document
                    .select_objects_direct([id], SelectionMode::Add);
            }
        }
    }
}
