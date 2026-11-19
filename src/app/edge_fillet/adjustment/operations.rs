//! Pick and place additional size handles on the original selected edges.
use super::*;
use crate::viewport::edge_blend::HandleMode;
use viboceros_geometry::NurbsCurve;

#[derive(Clone, Copy)]
pub(super) enum HandleOperation {
    Place(Option<f64>),
    Copy,
    Remove,
}

impl VibocerosApp {
    pub(in crate::app::edge_fillet) fn start_edge_handle_operation(
        &mut self,
        mut prompt: Prompt,
        option: &str,
    ) -> bool {
        let operation = if option.eq_ignore_ascii_case("AddHandle") {
            HandleOperation::Place(None)
        } else if option.eq_ignore_ascii_case("CopyHandle") {
            HandleOperation::Copy
        } else if option.eq_ignore_ascii_case("RemoveHandle") {
            if !self.has_added_edge_handles() {
                self.push_log("Use AddHandle to place a handle first.".into());
                self.command_input.clear();
                return true;
            }
            HandleOperation::Remove
        } else {
            return false;
        };
        self.edge_blend_adjustment.active = None;
        self.edge_blend_adjustment.drag_size = None;
        self.edge_blend_adjustment.operation = Some(operation);
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(in crate::app::edge_fillet) fn has_added_edge_handles(&self) -> bool {
        self.edge_blend_adjustment
            .sizes
            .iter()
            .any(|(&pick, (snapshot, sizes))| {
                !sizes.interior.is_empty()
                    && current(self.edge_blend_source_document(), pick, snapshot)
            })
    }

    pub(in crate::app::edge_fillet) fn edge_handle_operation_hint(&self) -> Option<&'static str> {
        match self.edge_blend_adjustment.operation? {
            HandleOperation::Place(_) => {
                Some("Pick a location on a selected edge. Esc returns to handle adjustment.")
            }
            HandleOperation::Copy => {
                Some("Pick the size handle to copy. Esc returns to handle adjustment.")
            }
            HandleOperation::Remove => {
                Some("Pick an added handle to remove. Esc returns to handle adjustment.")
            }
        }
    }

    pub(in crate::app) fn dismiss_edge_handle_operation(&mut self) -> bool {
        if !self.nested_measurement_captures_input()
            || self.edge_blend_adjustment.operation.take().is_none()
        {
            return false;
        }
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(in crate::app) fn edge_handle_placement_curves<'a>(
        &self,
        document: &'a Document,
    ) -> Vec<(ComponentPick, &'a NurbsCurve)> {
        if !matches!(
            self.edge_blend_adjustment.operation,
            Some(HandleOperation::Place(_))
        ) {
            return Vec::new();
        }
        self.component_selection
            .checked_picks(document)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|pick| {
                let Geometry::Brep(brep) = document.object(pick.object)?.geometry() else {
                    return None;
                };
                Some((pick, brep.edges().get(pick.index)?.curve()))
            })
            .collect()
    }

    pub(in crate::app) fn edge_handle_mode<'a>(
        &self,
        curves: &'a [(ComponentPick, &'a NurbsCurve)],
    ) -> HandleMode<'a> {
        match self.edge_blend_adjustment.operation {
            None => HandleMode::Edit,
            Some(HandleOperation::Copy) => HandleMode::Copy,
            Some(HandleOperation::Remove) => HandleMode::Remove,
            Some(HandleOperation::Place(_)) => HandleMode::Place(curves),
        }
    }

    pub(super) fn use_edge_handle_operation(&mut self, gesture: HandleGesture) -> bool {
        let (Some(prompt), Some(operation)) = (
            self.edge_fillet_prompt.filter(|p| p.adjusting),
            self.edge_blend_adjustment.operation,
        ) else {
            return false;
        };
        let id = match gesture {
            HandleGesture::Pick(id) | HandleGesture::Place(id) => id,
            HandleGesture::Drag { .. } | HandleGesture::Select(..) => return false,
        };
        if !self
            .component_selection
            .checked_picks(self.edge_blend_source_document())
            .is_ok_and(|picks| picks.contains(&id.pick))
        {
            return false;
        }
        let Some(source) = self
            .edge_blend_source_document()
            .object(id.pick.object)
            .map(|object| object.geometry_snapshot().clone())
        else {
            return false;
        };
        let mut sizes = self.edge_blend_sizes(prompt, id.pick, None);
        match (operation, gesture, id.location) {
            (HandleOperation::Copy, HandleGesture::Pick(_), location) => {
                let Some(size) = location.size(&sizes) else {
                    return false;
                };
                self.edge_blend_adjustment.operation = Some(HandleOperation::Place(Some(size)));
                self.log_edge_blend_prompt();
                true
            }
            (
                HandleOperation::Place(size),
                HandleGesture::Place(_),
                HandleLocation::Interior(t),
            ) if 0. < t && t < 1. => {
                let index = sizes
                    .interior
                    .partition_point(|(parameter, _)| *parameter < t);
                if sizes
                    .interior
                    .get(index)
                    .is_none_or(|(parameter, _)| *parameter != t)
                {
                    sizes
                        .interior
                        .insert(index, (t, size.unwrap_or(prompt.radius)));
                    self.edge_blend_adjustment
                        .sizes
                        .insert(id.pick, (source, sizes));
                }
                self.edge_blend_adjustment.operation = None;
                self.pick_edge_size_handle(id)
            }
            (HandleOperation::Remove, HandleGesture::Pick(_), HandleLocation::Interior(t)) => {
                sizes.interior.retain(|(parameter, _)| *parameter != t);
                self.edge_blend_adjustment
                    .sizes
                    .insert(id.pick, (source, sizes));
                self.dismiss_edge_handle_operation()
            }
            _ => false,
        }
    }
}
