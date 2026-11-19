//! Edit owned sizes against the original selected edge identities.
use super::*;
use crate::viewport::{
    ComponentPick,
    edge_blend::{HandleGesture, HandleLocation, SizeHandle, SizeHandleId},
};
use std::collections::{BTreeMap, BTreeSet};
use viboceros_command::EdgeBlendSizes;
use viboceros_command::component_selection::current;
use viboceros_document::GeometrySnapshot;

mod operations;
use operations::HandleOperation;

#[derive(Default)]
pub(in crate::app) struct Adjustment {
    sizes: BTreeMap<ComponentPick, (GeometrySnapshot, EdgeBlendSizes)>,
    pub(super) active: Option<(SizeHandleId, GeometrySnapshot)>,
    pub(super) drag_size: Option<f64>,
    operation: Option<HandleOperation>,
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

fn set_profile_size(sizes: &mut EdgeBlendSizes, size: f64) {
    sizes.ends = [size; 2];
    for (_, value) in &mut sizes.interior {
        *value = size;
    }
}

impl VibocerosApp {
    pub(in crate::app) fn adjusting_edge_blend(&self) -> bool {
        self.edge_fillet_prompt
            .is_some_and(|prompt| prompt.adjusting)
    }

    pub(super) fn begin_edge_blend_adjustment(&mut self, mut prompt: Prompt) {
        if let Err(error) = self.capture_edge_blend(prompt, None) {
            self.push_log(format!("Error: {error}"));
            return;
        }
        let picks = self
            .component_selection
            .checked_picks(&self.document)
            .unwrap_or_default();
        let picks = picks.into_iter().collect::<BTreeSet<_>>();
        self.edge_blend_adjustment
            .sizes
            .retain(|pick, (snapshot, _)| {
                picks.contains(pick) && current(&self.document, *pick, snapshot)
            });
        prompt.adjusting = true;
        prompt.awaiting_value = false;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
    }

    pub(super) fn reopen_edge_blend_selection(&mut self, mut prompt: Prompt) {
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        self.edge_blend_adjustment.operation = None;
        prompt.adjusting = false;
        prompt.awaiting_value = false;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
    }

    pub(in crate::app) fn dismiss_edge_size_input(&mut self) -> bool {
        if !self.nested_measurement_captures_input() {
            return false;
        }
        let Some(mut prompt) = self
            .edge_fillet_prompt
            .filter(|p| p.adjusting && p.awaiting_value)
        else {
            return false;
        };
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        prompt.awaiting_value = false;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(super) fn edge_blend_sizes(
        &self,
        prompt: Prompt,
        pick: ComponentPick,
        trial: Option<f64>,
    ) -> EdgeBlendSizes {
        let mut sizes = self
            .edge_blend_adjustment
            .sizes
            .get(&pick)
            .filter(|(snapshot, _)| current(&self.document, pick, snapshot))
            .map(|(_, sizes)| sizes.clone())
            .unwrap_or_else(|| [prompt.radius; 2].into());
        if let Some(trial) = trial {
            match &self.edge_blend_adjustment.active {
                None => set_profile_size(&mut sizes, trial),
                Some((id, source))
                    if prompt.link_handles && current(&self.document, id.pick, source) =>
                {
                    set_profile_size(&mut sizes, trial);
                }
                Some((id, source)) if id.pick == pick && current(&self.document, pick, source) => {
                    id.location.set_size(&mut sizes, trial);
                }
                Some(_) => {}
            }
        }
        sizes
    }

    pub(super) fn store_edge_blend_size(
        &mut self,
        prompt: &mut Prompt,
        default: f64,
        set_all: bool,
    ) {
        if !prompt.adjusting {
            return;
        }
        self.edge_blend_adjustment.drag_size = None;
        if let Some((id, source)) = self
            .edge_blend_adjustment
            .active
            .take()
            .filter(|_| !set_all)
        {
            if !current(&self.document, id.pick, &source) {
                prompt.radius = default;
                self.push_log("The edge source changed. Use SelectEdges to pick it again.".into());
                return;
            }
            if !prompt.link_handles {
                let size = std::mem::replace(&mut prompt.radius, default);
                let mut sizes = self.edge_blend_sizes(*prompt, id.pick, None);
                id.location.set_size(&mut sizes, size);
                self.edge_blend_adjustment
                    .sizes
                    .insert(id.pick, (source, sizes));
                self.push_log(format!(
                    "Edge {} {} {} = {size}",
                    id.pick.index,
                    id.location.label(),
                    prompt.parameter()
                ));
                return;
            }
        }
        self.edge_blend_adjustment.sizes.retain(|_, (_, profile)| {
            set_profile_size(profile, prompt.radius);
            !profile.interior.is_empty()
        });
    }

    pub(in crate::app) fn edge_blend_size_handles(&self) -> Vec<SizeHandle> {
        let Some(prompt) = self.edge_fillet_prompt.filter(|prompt| prompt.adjusting) else {
            return Vec::new();
        };
        let picks = self
            .component_selection
            .checked_picks(&self.document)
            .unwrap_or_default();
        let mut handles = Vec::new();
        for pick in picks {
            let Some(Geometry::Brep(brep)) =
                self.document.object(pick.object).map(|o| o.geometry())
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
            let sizes = self.edge_blend_sizes(prompt, pick, self.edge_blend_adjustment.drag_size);
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
                    active: self
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
        self.edge_fillet_prompt.is_some_and(|p| p.show_handles)
    }

    pub(in crate::app) fn pick_edge_size_handle(&mut self, id: SizeHandleId) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt.filter(|p| p.adjusting) else {
            return false;
        };
        if !self
            .component_selection
            .checked_picks(&self.document)
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
        let Some(object) = self.document.object(id.pick.object) else {
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
        self.edge_blend_adjustment.active = Some((id, object.geometry_snapshot().clone()));
        self.edge_blend_adjustment.drag_size = None;
        prompt.awaiting_value = true;
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
