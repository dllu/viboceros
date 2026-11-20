use super::*;
use crate::viewport::EdgePick;
use viboceros_geometry::CurveRef;

impl VibocerosApp {
    pub(in crate::app) fn picking_loft_edges(&self) -> bool {
        self.plane_prompt.is_none() && self.loft_prompt.as_ref().is_some_and(Prompt::selecting)
    }

    pub(in crate::app) fn select_loft_curves(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> bool {
        if self.plane_prompt.is_some() || self.loft_prompt.is_none() {
            return false;
        }
        self.apply_loft_picks(ids.into_iter().map(|id| ((id, None), None)), mode)
    }

    pub(in crate::app) fn select_loft_curve_click(
        &mut self,
        click: SelectionClick,
        pick: Option<(usize, egui::Pos2)>,
    ) -> bool {
        if self.plane_prompt.is_some() || self.loft_prompt.is_none() {
            return false;
        }
        let point = click
            .object_id
            .zip(pick)
            .and_then(|(id, (viewport, pixel))| {
                self.viewports[viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    None,
                    pixel,
                    self.document.tolerance(),
                )
            });
        self.apply_loft_picks(click.object_id.map(|id| ((id, None), point)), click.mode)
    }

    pub(in crate::app) fn select_loft_edge_click(
        &mut self,
        picks: &[EdgePick],
        contacts: &[(EdgePick, Point3)],
        modifiers: egui::Modifiers,
    ) -> bool {
        if !self.picking_loft_edges() {
            return false;
        }
        if let Some(pick) = picks.first() {
            let point = contacts
                .iter()
                .find(|(edge, _)| edge == pick)
                .map(|(_, point)| *point);
            let mode = if modifiers.ctrl && !modifiers.shift {
                SelectionMode::Remove
            } else {
                SelectionMode::Add
            };
            self.apply_loft_picks([((pick.object, Some(pick.edge)), point)], mode);
        }
        true
    }

    pub(super) fn apply_loft_picks(
        &mut self,
        picks: impl IntoIterator<Item = (SectionKey, Option<Point3>)>,
        mode: SelectionMode,
    ) -> bool {
        let p = self.loft_prompt.as_ref().unwrap();
        if !p.selecting() {
            return true;
        }
        let mut sources = p.state.sources.clone();
        for (key, point) in picks {
            let index = sources
                .iter()
                .position(|section| section_key(&section.source) == key);
            match (mode, index) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(index)) => {
                    sources.remove(index);
                }
                (SelectionMode::Remove, None) | (_, Some(_)) => {}
                (_, None) => {
                    if key.1.is_none()
                        && self
                            .document
                            .object(key.0)
                            .is_none_or(|object| object.geometry().curve_ref().is_none())
                    {
                        continue;
                    }
                    match CapturedCurve::capture(&self.document, key.0, key.1) {
                        Ok(source) => {
                            let reversed =
                                point.and_then(|point| near_end(source.curve_ref(), point));
                            sources.push(Section { source, reversed });
                        }
                        Err(error) => {
                            self.push_log(format!("Unable to read Loft section: {error}"));
                            return true;
                        }
                    }
                }
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
            .map(|section| section_key(&section.source))
            .ne(p
                .state
                .sources
                .iter()
                .map(|section| section_key(&section.source)))
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
            .filter_map(|section| {
                let (id, edge) = section_key(&section.source);
                edge.is_none().then_some(id)
            })
            .collect::<Vec<_>>();
        self.restore_curve_selection(ids);
    }
}

fn near_end(curve: CurveRef<'_>, point: Point3) -> Option<bool> {
    if curve.is_closed().ok()? {
        return None;
    }
    let left = curve.evaluate(*curve.domain().start()).ok()?;
    let right = curve.evaluate(*curve.domain().end()).ok()?;
    Some(point.distance_to(right).ok()? < point.distance_to(left).ok()?)
}
