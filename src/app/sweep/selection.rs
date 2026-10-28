use super::*;
impl VibocerosApp {
    pub(in crate::app) fn select_sweep_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        if self.plane_prompt.is_some() {
            return;
        }
        let Some(p) = self.sweep_session.as_ref().filter(|p| p.selecting()) else {
            return;
        };
        let mut sources = p.state.sources.clone();
        // Toggle can remove every original pick later in the batch. Beyond this
        // bounded buffer even those removals cannot bring it under the limit.
        let maximum = viboceros_geometry::Sweep1::MAX_SECTIONS
            + 1
            + if mode == SelectionMode::Toggle {
                sources.len()
            } else {
                0
            };
        let mut toggled = std::collections::BTreeSet::new();
        for id in ids {
            let Some(o) = self.document.object(id).filter(|o| {
                self.document.is_object_selectable(id) && o.geometry().curve_ref().is_some()
            }) else {
                continue;
            };
            if mode == SelectionMode::Toggle && !toggled.insert(id) {
                continue;
            }
            let index = sources.iter().position(|(selected, _)| *selected == id);
            match (mode, index) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(i)) => {
                    sources.remove(i);
                }
                (SelectionMode::Remove, None) | (_, Some(_)) => {}
                (_, None) => sources.push((id, o.geometry_snapshot().clone())),
            }
            if sources.len() > maximum {
                self.push_log("Too many Sweep1 sections (limit 256 including point ends)".into());
                return;
            }
        }
        let p = self.sweep_session.as_mut().unwrap();
        if sources
            .iter()
            .map(|(id, _)| id)
            .eq(p.state.sources.iter().map(|(id, _)| id))
        {
            return;
        }
        let old = p.state.clone();
        p.state.sources = sources;
        if p.profiles()
            + usize::from(p.state.options.ends.start.is_some())
            + usize::from(p.state.options.ends.end.is_some())
            > viboceros_geometry::Sweep1::MAX_SECTIONS
        {
            p.state = old;
            self.push_log("Too many Sweep1 sections (limit 256 including point ends)".into());
            return;
        }
        p.undo.push(old);
        self.sync_sweep_selection();
        self.log_sweep_input();
    }
}
