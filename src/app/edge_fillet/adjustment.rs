//! Edit owned sizes against the original selected edge identities.
use super::*;
use crate::viewport::{
    ComponentPick,
    edge_blend::{HandleGesture, SizeHandle},
};
use std::collections::{BTreeMap, BTreeSet};
use viboceros_command::component_selection::current;
use viboceros_document::GeometrySnapshot;

#[derive(Default)]
pub(in crate::app) struct Adjustment {
    sizes: BTreeMap<ComponentPick, (GeometrySnapshot, f64)>,
    pub(super) active: Option<(ComponentPick, GeometrySnapshot)>,
    pub(super) drag_size: Option<f64>,
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

    pub(super) fn edge_blend_size(
        &self,
        prompt: Prompt,
        pick: ComponentPick,
        trial: Option<f64>,
    ) -> f64 {
        if let Some(trial) = trial
            && self
                .edge_blend_adjustment
                .active
                .as_ref()
                .is_none_or(|(active, source)| {
                    *active == pick && current(&self.document, pick, source)
                })
        {
            return trial;
        }
        self.edge_blend_adjustment
            .sizes
            .get(&pick)
            .filter(|(snapshot, _)| current(&self.document, pick, snapshot))
            .map_or(prompt.radius, |(_, size)| *size)
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
        if let Some((pick, source)) = self
            .edge_blend_adjustment
            .active
            .take()
            .filter(|_| !set_all)
        {
            let size = std::mem::replace(&mut prompt.radius, default);
            if current(&self.document, pick, &source) {
                self.edge_blend_adjustment
                    .sizes
                    .insert(pick, (source, size));
                self.push_log(format!(
                    "Edge {} {} = {size}",
                    pick.index,
                    prompt.parameter()
                ));
            } else {
                self.push_log("The edge source changed. Use SelectEdges to pick it again.".into());
            }
        } else {
            self.edge_blend_adjustment.sizes.clear();
        }
    }

    pub(in crate::app) fn edge_blend_size_handles(&self) -> Vec<SizeHandle> {
        let Some(prompt) = self.edge_fillet_prompt.filter(|prompt| prompt.adjusting) else {
            return Vec::new();
        };
        self.component_selection
            .checked_picks(&self.document)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|pick| {
                let Geometry::Brep(brep) = self.document.object(pick.object)?.geometry() else {
                    return None;
                };
                let curve = brep.edges().get(pick.index)?.curve();
                let domain = curve.domain();
                let point = curve
                    .evaluate(domain.start().midpoint(*domain.end()))
                    .ok()?;
                Some(SizeHandle {
                    pick,
                    point,
                    size: self.edge_blend_size(prompt, pick, self.edge_blend_adjustment.drag_size),
                    active: self
                        .edge_blend_adjustment
                        .active
                        .as_ref()
                        .is_some_and(|(active, _)| *active == pick),
                })
            })
            .collect()
    }

    pub(in crate::app) fn show_edge_size_handles(&self) -> bool {
        self.edge_fillet_prompt.is_some_and(|p| p.show_handles)
    }

    pub(in crate::app) fn pick_edge_size_handle(&mut self, pick: ComponentPick) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt.filter(|p| p.adjusting) else {
            return false;
        };
        if !self
            .component_selection
            .checked_picks(&self.document)
            .is_ok_and(|picks| picks.contains(&pick))
        {
            return false;
        }
        let size = self.edge_blend_size(prompt, pick, None);
        let Some(object) = self.document.object(pick.object) else {
            return false;
        };
        self.edge_blend_adjustment.active = Some((pick, object.geometry_snapshot().clone()));
        self.edge_blend_adjustment.drag_size = None;
        prompt.awaiting_value = true;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.push_log(format!(
            "Edge {} {} = {size}. Enter a size or use FromCurve or FromTwoPoints.",
            pick.index,
            prompt.parameter()
        ));
        true
    }

    pub(in crate::app) fn edit_edge_blend_handle(&mut self, gesture: HandleGesture) -> bool {
        let (pick, value, finished) = match gesture {
            HandleGesture::Pick(pick) => return self.pick_edge_size_handle(pick),
            HandleGesture::Drag {
                pick,
                value,
                finished,
            } => (pick, value, finished),
        };
        let active = self
            .edge_blend_adjustment
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == pick);
        if !active && !self.pick_edge_size_handle(pick) {
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
