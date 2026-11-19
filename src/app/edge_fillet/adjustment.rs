//! Edit owned sizes against the original selected edge identities.
use super::*;
use crate::viewport::{
    ComponentPick,
    edge_blend::{HandleGesture, HandleLocation, SizeHandle, SizeHandleId},
};
use std::collections::{BTreeMap, BTreeSet};
use viboceros_command::EdgeBlendSizes;
use viboceros_command::component_selection::{ComponentAction, current};
use viboceros_document::GeometrySnapshot;

mod operations;
mod selection;
use operations::HandleOperation;
use selection::HandleSelection;

pub(super) type EdgeSizes = BTreeMap<ComponentPick, (GeometrySnapshot, EdgeBlendSizes)>;

#[derive(Default)]
pub(in crate::app) struct Adjustment {
    pub(super) sizes: EdgeSizes,
    pub(super) active: Option<(SizeHandleId, GeometrySnapshot)>,
    pub(super) drag_size: Option<f64>,
    operation: Option<HandleOperation>,
    selection: HandleSelection,
}

impl HandleLocation {
    fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Seam => "seam",
            Self::Interior(_) => "interior",
        }
    }

    fn size(self, sizes: &EdgeBlendSizes) -> Option<f64> {
        match self {
            Self::Start | Self::Seam => Some(sizes.ends[0]),
            Self::End => Some(sizes.ends[1]),
            Self::Interior(t) => sizes.interior.iter().find(|(p, _)| *p == t).map(|s| s.1),
        }
    }

    fn set_size(self, sizes: &mut EdgeBlendSizes, size: f64) {
        match self {
            Self::Start => sizes.ends[0] = size,
            Self::End => sizes.ends[1] = size,
            Self::Seam => sizes.ends = [size; 2],
            Self::Interior(t) => {
                if let Some((_, value)) = sizes.interior.iter_mut().find(|(p, _)| *p == t) {
                    *value = size;
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ProfileEdit {
    All(f64),
    Handles {
        id: SizeHandleId,
        size: f64,
        group: bool,
        ratio: Option<f64>,
    },
}

impl ProfileEdit {
    fn affects(self, pick: ComponentPick, selection: &HandleSelection) -> bool {
        match self {
            Self::All(_) | Self::Handles { ratio: Some(_), .. } => true,
            Self::Handles { group: true, .. } => !selection.locations(pick).is_empty(),
            Self::Handles { id, .. } => id.pick == pick,
        }
    }

    fn apply(self, pick: ComponentPick, selection: &HandleSelection, sizes: &mut EdgeBlendSizes) {
        match self {
            Self::All(size) => {
                sizes.ends = [size; 2];
                for (_, value) in &mut sizes.interior {
                    *value = size;
                }
            }
            Self::Handles {
                id,
                size,
                group,
                ratio,
            } => {
                if let Some(ratio) = ratio {
                    for value in sizes
                        .ends
                        .iter_mut()
                        .chain(sizes.interior.iter_mut().map(|s| &mut s.1))
                    {
                        *value *= ratio;
                    }
                }
                if group {
                    for &location in selection.locations(pick) {
                        location.set_size(sizes, size);
                    }
                } else if id.pick == pick {
                    id.location.set_size(sizes, size);
                }
            }
        }
    }
}

impl VibocerosApp {
    /// Keep the size an edge was picked with, even as the next size changes.
    pub(in crate::app) fn track_edge_blend_picks(
        &mut self,
        picks: impl IntoIterator<Item = ComponentPick>,
        action: ComponentAction,
    ) {
        let Some(prompt) = self.edge_fillet_prompt else {
            return;
        };
        let document = edit::source_document(&self.edge_blend_edit, &self.document);
        let sizes = &mut self.edge_blend_adjustment.sizes;
        for pick in picks
            .into_iter()
            .filter(|pick| pick.kind == ComponentSelectionKind::BrepEdge)
        {
            let selected = match action {
                ComponentAction::Add => true,
                ComponentAction::Remove => false,
                ComponentAction::Toggle => self.component_selection.contains(pick),
            };
            if !selected {
                sizes.remove(&pick);
                continue;
            }
            if let Some(object) = document.object(pick.object) {
                let source = object.geometry_snapshot();
                let entry = sizes
                    .entry(pick)
                    .or_insert_with(|| (source.clone(), [prompt.radius; 2].into()));
                if entry.0 != *source {
                    *entry = (source.clone(), [prompt.radius; 2].into());
                }
            }
        }
    }

    pub(super) fn retain_edge_blend_picks(&mut self, picks: &[ComponentPick]) {
        let document = edit::source_document(&self.edge_blend_edit, &self.document);
        let picks = picks.iter().copied().collect::<BTreeSet<_>>();
        self.edge_blend_adjustment
            .sizes
            .retain(|pick, (snapshot, _)| {
                picks.contains(pick) && current(document, *pick, snapshot)
            });
    }

    pub(in crate::app) fn adjusting_edge_blend(&self) -> bool {
        self.edge_fillet_prompt
            .is_some_and(|prompt| prompt.adjusting)
    }

    pub(super) fn begin_edge_blend_adjustment(&mut self, mut prompt: Prompt) {
        if self.edge_blend_edit_target().is_some()
            && self
                .component_selection
                .checked_picks(self.edge_blend_source_document())
                .is_ok_and(|picks| picks.is_empty())
        {
            self.restore_latest_edge_blend(prompt);
            return;
        }
        if let Err(error) = self.capture_edge_blend(prompt, None) {
            self.push_log(format!("Error: {error}"));
            return;
        }
        let picks = self
            .component_selection
            .checked_picks(self.edge_blend_source_document())
            .unwrap_or_default();
        self.retain_edge_blend_picks(&picks);
        self.track_edge_blend_picks(picks, ComponentAction::Add);
        prompt.adjusting = true;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
    }

    pub(super) fn reopen_edge_blend_selection(&mut self, mut prompt: Prompt) {
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        self.edge_blend_adjustment.operation = None;
        self.edge_blend_adjustment.selection.clear();
        prompt.adjusting = false;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
    }

    pub(in crate::app) fn dismiss_edge_size_input(&mut self) -> bool {
        if !self.nested_measurement_captures_input() {
            return false;
        }
        let Some(mut prompt) = self.edge_fillet_prompt.filter(|p| {
            p.size_input.is_some() && (p.adjusting || p.size_input == Some(SizeInput::Next))
        }) else {
            return false;
        };
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(super) fn edge_blend_sizes(
        &self,
        prompt: Prompt,
        pick: ComponentPick,
        edit: Option<ProfileEdit>,
    ) -> EdgeBlendSizes {
        let mut sizes = self
            .edge_blend_adjustment
            .sizes
            .get(&pick)
            .filter(|(snapshot, _)| current(self.edge_blend_source_document(), pick, snapshot))
            .map(|(_, sizes)| sizes.clone())
            .unwrap_or_else(|| [prompt.radius; 2].into());
        if let Some(edit) = edit {
            edit.apply(pick, &self.edge_blend_adjustment.selection, &mut sizes);
        }
        sizes
    }

    fn edge_blend_edit(
        &self,
        prompt: Prompt,
        size: f64,
        set_all: bool,
    ) -> Result<ProfileEdit, &'static str> {
        let Some((id, source)) = self
            .edge_blend_adjustment
            .active
            .as_ref()
            .filter(|_| !set_all && prompt.size_input != Some(SizeInput::All))
        else {
            return Ok(ProfileEdit::All(size));
        };
        let stale = "The edge source changed. Use SelectEdges to pick it again.";
        if !current(self.edge_blend_source_document(), id.pick, source) {
            return Err(stale);
        }
        let ratio = if prompt.toggles.link_handles {
            let old_size = id
                .location
                .size(&self.edge_blend_sizes(prompt, id.pick, None))
                .ok_or(stale)?;
            Some(size / old_size)
        } else {
            None
        };
        Ok(ProfileEdit::Handles {
            id: *id,
            size,
            group: self.edge_blend_adjustment.selection.contains(*id),
            ratio,
        })
    }

    pub(super) fn edge_blend_trial(
        &self,
        prompt: Prompt,
        size: Option<f64>,
    ) -> Option<ProfileEdit> {
        let size = size.filter(|size| size.is_finite() && *size > 0.)?;
        if prompt.size_input == Some(SizeInput::Next) {
            return None;
        }
        self.edge_blend_edit(prompt, size, false).ok()
    }

    pub(super) fn store_edge_blend_size(
        &mut self,
        prompt: &mut Prompt,
        default: f64,
        set_all: bool,
    ) -> Result<(), &'static str> {
        self.edge_blend_adjustment.drag_size = None;
        let size = prompt.radius;
        let old = Prompt {
            radius: default,
            ..*prompt
        };
        let edit = self.edge_blend_edit(old, size, set_all)?;
        let picks = self
            .component_selection
            .checked_picks(self.edge_blend_source_document())?;
        let updated = picks
            .into_iter()
            .filter(|&pick| edit.affects(pick, &self.edge_blend_adjustment.selection))
            .map(|pick| {
                let source = self
                    .edge_blend_source_document()
                    .object(pick.object)
                    .unwrap()
                    .geometry_snapshot()
                    .clone();
                (pick, (source, self.edge_blend_sizes(old, pick, Some(edit))))
            })
            .collect::<Vec<_>>();
        if updated.iter().any(|(_, (_, sizes))| !sizes.is_valid()) {
            return Err("Linked handle sizes must all be finite and positive");
        }
        self.edge_blend_adjustment.sizes.extend(updated);
        if !matches!(edit, ProfileEdit::All(_)) {
            prompt.radius = default;
        }
        if let Some((id, _)) = self
            .edge_blend_adjustment
            .active
            .take()
            .filter(|_| !set_all)
        {
            self.push_log(format!(
                "Edge {} {} {} = {size}",
                id.pick.index,
                id.location.label(),
                prompt.parameter()
            ));
        }
        Ok(())
    }

    pub(in crate::app) fn edge_blend_size_handles(&self) -> Vec<SizeHandle> {
        let Some(prompt) = self.edge_fillet_prompt.filter(|prompt| prompt.adjusting) else {
            return Vec::new();
        };
        let picks = self
            .component_selection
            .checked_picks(self.edge_blend_source_document())
            .unwrap_or_default();
        let mut handles = Vec::new();
        let trial = self.edge_blend_trial(prompt, self.edge_blend_adjustment.drag_size);
        for pick in picks {
            let Some(Geometry::Brep(brep)) = self
                .edge_blend_source_document()
                .object(pick.object)
                .map(|o| o.geometry())
            else {
                continue;
            };
            let Some(edge) = brep.edges().get(pick.index) else {
                continue;
            };
            let [a, b] = edge.vertices();
            let ends: &[(HandleLocation, f64)] = if a == b {
                &[(HandleLocation::Seam, 0.)]
            } else {
                &[(HandleLocation::Start, 0.08), (HandleLocation::End, 0.92)]
            };
            let curve = edge.curve();
            let domain = curve.domain();
            let sizes = self.edge_blend_sizes(prompt, pick, trial);
            for ((location, fraction), size) in ends
                .iter()
                .copied()
                .filter_map(|end| Some((end, end.0.size(&sizes)?)))
                .chain(
                    sizes
                        .interior
                        .iter()
                        .map(|&(t, size)| ((HandleLocation::Interior(t), t), size)),
                )
            {
                let parameter = (domain.end() - domain.start()).mul_add(fraction, *domain.start());
                let Ok(point) = curve.evaluate(parameter) else {
                    continue;
                };
                let id = SizeHandleId { pick, location };
                handles.push(SizeHandle {
                    id,
                    point,
                    size,
                    active: self.edge_blend_adjustment.selection.contains(id)
                        || self
                            .edge_blend_adjustment
                            .active
                            .as_ref()
                            .is_some_and(|(active, _)| *active == id),
                });
            }
        }
        handles
    }

    pub(in crate::app) fn show_edge_size_handles(&self) -> bool {
        self.edge_fillet_prompt
            .is_some_and(|p| p.toggles.show_handles)
    }

    pub(in crate::app) fn pick_edge_size_handle(&mut self, id: SizeHandleId) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt.filter(|p| p.adjusting) else {
            return false;
        };
        if !self
            .component_selection
            .checked_picks(self.edge_blend_source_document())
            .is_ok_and(|picks| picks.contains(&id.pick))
        {
            return false;
        }
        let Some(size) = id
            .location
            .size(&self.edge_blend_sizes(prompt, id.pick, None))
        else {
            return false;
        };
        let Some(object) = self.edge_blend_source_document().object(id.pick.object) else {
            return false;
        };
        let Geometry::Brep(brep) = object.geometry() else {
            return false;
        };
        let Some(edge) = brep.edges().get(id.pick.index) else {
            return false;
        };
        let [a, b] = edge.vertices();
        if !matches!(id.location, HandleLocation::Interior(_))
            && (a == b) != (id.location == HandleLocation::Seam)
        {
            return false;
        }
        let source = object.geometry_snapshot().clone();
        if !self.edge_blend_adjustment.selection.contains(id) {
            self.edge_blend_adjustment.selection.clear();
        }
        self.edge_blend_adjustment.active = Some((id, source));
        self.edge_blend_adjustment.drag_size = None;
        prompt.size_input = Some(SizeInput::Edit);
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.push_log(format!(
            "Edge {} {} {} = {size}. Enter a size or use FromCurve or FromTwoPoints.",
            id.pick.index,
            id.location.label(),
            prompt.parameter()
        ));
        true
    }

    pub(in crate::app) fn edit_edge_blend_handle(&mut self, gesture: HandleGesture) -> bool {
        if self.edge_blend_adjustment.operation.is_some() {
            return self.use_edge_handle_operation(gesture);
        }
        let (id, value, finished) = match gesture {
            HandleGesture::Pick(id) => return self.pick_edge_size_handle(id),
            HandleGesture::Select(ids, mode) => return self.select_edge_size_handles(ids, mode),
            HandleGesture::Place(_) => return false,
            HandleGesture::Drag {
                id,
                value,
                finished,
            } => (id, value, finished),
        };
        let active = self
            .edge_blend_adjustment
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == id);
        if !active && !self.pick_edge_size_handle(id) {
            return false;
        }
        if finished {
            if let Some(value) = value {
                self.try_continue_edge_fillet(&value.to_string());
            } else {
                self.dismiss_edge_size_input();
            }
        } else {
            self.edge_blend_adjustment.drag_size = value;
        }
        true
    }
}
