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
        self.apply_loft_curve_picks(ids.into_iter().map(|id| (id, None)), mode)
    }

    pub(in crate::app) fn select_loft_curve_click(
        &mut self,
        click: SelectionClick,
        pick: Option<(usize, egui::Pos2)>,
    ) -> bool {
        if self.plane_prompt.is_some() || self.loft_prompt.is_none() {
            return false;
        }
        let direction = (|| {
            let (viewport, pixel) = pick?;
            let object = self.document.object(click.object_id?)?;
            let geometry = object.geometry();
            let curve = geometry.curve_ref()?;
            if curve.is_closed().ok()? {
                return None;
            }
            let tolerance = self.document.tolerance();
            let near =
                self.viewports[viewport].curve_pick_point(geometry, None, pixel, tolerance)?;
            let left = curve.evaluate(*curve.domain().start()).ok()?;
            let right = curve.evaluate(*curve.domain().end()).ok()?;
            Some(near.distance_to(right).ok()? < near.distance_to(left).ok()?)
        })();
        self.apply_loft_curve_picks(click.object_id.map(|id| (id, direction)), click.mode)
    }

    fn apply_loft_curve_picks(
        &mut self,
        ids: impl IntoIterator<Item = (ObjectId, Option<bool>)>,
        mode: SelectionMode,
    ) -> bool {
        let p = self.loft_prompt.as_ref().unwrap();
        if !p.selecting() {
            return true;
        }
        let mut sources = p.state.sources.clone();
        for (id, reversed) in ids {
            let Some(object) = self.document.object(id).filter(|o| {
                self.document.is_object_selectable(id) && o.geometry().curve_ref().is_some()
            }) else {
                continue;
            };
            let index = sources.iter().position(|source| source.id == id);
            match (mode, index) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(index)) => {
                    sources.remove(index);
                }
                (SelectionMode::Remove, None) | (_, Some(_)) => {}
                (_, None) => sources.push(Section {
                    id,
                    geometry: object.geometry_snapshot().clone(),
                    reversed,
                }),
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
            .map(|source| source.id)
            .ne(p.state.sources.iter().map(|source| source.id))
        {
            let p = self.loft_prompt.as_mut().unwrap();
            p.profile_id_pending = false;
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
            .map(|source| source.id)
            .collect::<Vec<_>>();
        self.restore_curve_selection(ids);
    }
}
