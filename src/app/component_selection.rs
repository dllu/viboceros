//! Transient UI components retain exact source identities, outside Undo.
use super::*;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use std::collections::BTreeMap;
use viboceros_command::ComponentSelectionKind;
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Copy, Debug, Default)]
enum ComponentAction {
    #[default]
    Add,
    Remove,
    Toggle,
}

#[derive(Debug, Default)]
pub(super) struct ComponentSelection {
    picks: BTreeMap<ComponentPick, GeometrySnapshot>,
    order: Vec<ComponentPick>,
    candidates: Vec<(ComponentPick, GeometrySnapshot)>,
    choice_action: ComponentAction,
    hover: Option<ComponentPick>,
}

impl ComponentSelection {
    pub(super) fn has_choices(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub(super) fn valid_picks(&mut self, document: &Document) -> Vec<ComponentPick> {
        self.picks
            .retain(|pick, snapshot| current(document, *pick, snapshot));
        self.order.retain(|pick| self.picks.contains_key(pick));
        self.picks.keys().copied().collect()
    }
    pub(super) fn highlights(&mut self, document: &Document) -> Vec<ComponentPick> {
        let mut picks = self
            .picks
            .iter()
            .filter(|(pick, snapshot)| current(document, **pick, snapshot))
            .map(|(pick, _)| *pick)
            .collect::<Vec<_>>();
        if let Some(hover) = self.hover.filter(|hover| {
            self.candidates
                .iter()
                .any(|(pick, snapshot)| pick == hover && current(document, *pick, snapshot))
        }) {
            picks.push(hover);
        } else {
            picks.extend(
                self.candidates
                    .iter()
                    .filter(|(pick, snapshot)| current(document, *pick, snapshot))
                    .map(|(pick, _)| *pick),
            );
        }
        picks.sort();
        picks.dedup();
        picks
    }
    pub(super) fn checked_picks(
        &self,
        document: &Document,
    ) -> Result<Vec<ComponentPick>, &'static str> {
        if self
            .picks
            .iter()
            .any(|(pick, snapshot)| !current(document, *pick, snapshot))
        {
            return Err("component source changed; select the components again");
        }
        Ok(self.order.clone())
    }
    pub(super) fn clear(&mut self) {
        self.picks.clear();
        self.order.clear();
        self.clear_choices();
    }
    pub(super) fn clear_choices(&mut self) {
        self.candidates.clear();
        self.choice_action = ComponentAction::Add;
        self.hover = None;
    }
    fn apply(
        &mut self,
        document: &Document,
        picks: Vec<ComponentPick>,
        action: ComponentAction,
    ) -> Result<(), &'static str> {
        let mut seen = std::collections::BTreeSet::new();
        let staged = picks
            .into_iter()
            .filter(|pick| seen.insert(*pick))
            .map(|pick| source(document, pick).map(|snapshot| (pick, snapshot)))
            .collect::<Result<Vec<_>, _>>()?;
        self.clear_choices();
        for (pick, snapshot) in staged {
            if matches!(action, ComponentAction::Remove)
                || (matches!(action, ComponentAction::Toggle) && self.picks.contains_key(&pick))
            {
                self.picks.remove(&pick);
                self.order.retain(|old| *old != pick);
            } else {
                if !self.picks.contains_key(&pick) {
                    self.order.push(pick);
                }
                self.picks.insert(pick, snapshot);
            }
        }
        Ok(())
    }
}

fn current(document: &Document, pick: ComponentPick, snapshot: &GeometrySnapshot) -> bool {
    document.is_object_selectable(pick.object)
        && document
            .object(pick.object)
            .is_some_and(|object| object.geometry_snapshot() == snapshot)
}

fn source(document: &Document, pick: ComponentPick) -> Result<GeometrySnapshot, &'static str> {
    let object = document
        .object(pick.object)
        .filter(|_| document.is_object_selectable(pick.object))
        .ok_or("component source is unavailable")?;
    let converted;
    let brep = match object.geometry() {
        viboceros_document::Geometry::Brep(brep) => brep,
        viboceros_document::Geometry::NurbsSurface(surface) => {
            converted =
                viboceros_geometry::Brep::try_surface_face(surface.clone(), document.tolerance())
                    .map_err(|_| "invalid component surface")?;
            &converted
        }
        _ => return Err("select a surface or B-rep component"),
    };
    let count = match pick.kind {
        ComponentSelectionKind::BrepEdge => brep.edges().len(),
        ComponentSelectionKind::BrepFace => brep.faces().len(),
    };
    if pick.index >= count {
        return Err("component index is outside its source");
    }
    Ok(object.geometry_snapshot().clone())
}

impl VibocerosApp {
    pub(super) fn component_preselection_available(&self) -> bool {
        self.active_command.is_none()
            && self.boolean_two_prompt.is_none()
            && self.planar_boolean_prompt.is_none()
            && self.tween_surfaces_prompt.is_none()
            && self.selection_menu.is_none()
            && self.zoom_factor_pending.is_none()
            && self.snap_size_pending.is_none()
            && (self.object_prompt.is_none()
                || self.shrink_prompt_mode()
                    == Some(viboceros_geometry::BrepSurfaceShrinkMode::Standard))
            && self.group_prompt.is_none()
            && self.intersection_prompt.is_none()
            && self.edge_prompt.is_none()
            && self.hole_prompt.is_none()
            && self.unjoin_prompt.is_none()
            && self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && self.end_analysis_pick.is_none()
            && self.zoom_target.is_none()
            && !self.zoom_window_pending
            && self.circular_selection.is_none()
            && self.boundary_selection.is_none()
            && self.fence_selection.is_none()
            && self.lasso_selection.is_none()
    }
    pub(super) fn accept_component_click(&mut self, click: ComponentClick) {
        if !click.preselection
            && self.unjoin_prompt.is_none()
            && !self.picking_extract_faces()
            && !self.picking_subcurve_edge()
        {
            if self
                .hole_prompt
                .as_ref()
                .is_some_and(super::untrim_holes::HolePrompt::picking_edges)
            {
                self.accept_hole_edges(
                    click
                        .picks
                        .into_iter()
                        .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
                        .map(|p| crate::viewport::EdgePick {
                            object: p.object,
                            edge: p.index,
                        })
                        .collect(),
                );
            } else if self
                .hole_prompt
                .as_ref()
                .is_some_and(super::untrim_holes::HolePrompt::picking_faces)
                && let [pick] = click.picks.as_slice()
                && pick.kind == ComponentSelectionKind::BrepFace
            {
                self.accept_hole_component(
                    pick.object,
                    viboceros_command::UntrimHolesComponent::Face(pick.index),
                );
            }
            return;
        }
        if click.preselection && !self.component_preselection_available() {
            return;
        }
        if !click.preselection && click.modifiers.alt {
            self.component_selection.clear_choices();
            return;
        }
        let picks = if self.picking_subcurve_edge() {
            click
                .picks
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
                .collect()
        } else if self.picking_extract_faces() {
            click
                .picks
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                .collect()
        } else if self.unjoin_prompt.is_some() {
            self.unjoinable_picks(click.picks)
        } else {
            click.picks
        };
        let action = if click.preselection {
            ComponentAction::Toggle
        } else if click.modifiers.ctrl || click.modifiers.command {
            if click.modifiers.shift {
                ComponentAction::Toggle
            } else {
                ComponentAction::Remove
            }
        } else {
            ComponentAction::Add
        };
        match picks.as_slice() {
            [] => self.component_selection.clear_choices(),
            [_] => self.select_components(picks, action),
            _ => {
                match picks
                    .into_iter()
                    .map(|pick| source(&self.document, pick).map(|snapshot| (pick, snapshot)))
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(candidates) => {
                        for (index, (pick, _)) in candidates.iter().enumerate() {
                            let kind = match pick.kind {
                                ComponentSelectionKind::BrepEdge => "edge",
                                ComponentSelectionKind::BrepFace => "face",
                            };
                            self.push_log(format!(
                                "{}: object {} {kind} {}",
                                index + 1,
                                pick.object,
                                pick.index
                            ));
                        }
                        self.component_selection.candidates = candidates;
                        self.component_selection.choice_action = action;
                        self.component_selection.hover = None;
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
        }
    }

    fn select_components(&mut self, picks: Vec<ComponentPick>, action: ComponentAction) {
        if self.picking_subcurve_edge() {
            if let [pick] = picks.as_slice() {
                self.pick_subcurve_edge(crate::viewport::EdgePick {
                    object: pick.object,
                    edge: pick.index,
                });
            }
            return;
        }
        let parents = picks.iter().map(|pick| pick.object).collect::<Vec<_>>();
        match self
            .component_selection
            .apply(&self.document, picks, action)
        {
            Ok(()) => {
                let _ = self
                    .document
                    .select_objects_direct(parents, viboceros_document::SelectionMode::Remove);
                self.push_log(format!(
                    "{} component(s) selected",
                    self.component_selection.picks.len()
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn accept_component_window(&mut self, window: ComponentWindow) {
        if window.preselection {
            if self.component_preselection_available() {
                self.select_components(window.picks, ComponentAction::Add);
            }
        } else if self.unjoin_prompt.is_some() || self.picking_extract_faces() {
            let picks = if self.picking_extract_faces() {
                window
                    .picks
                    .into_iter()
                    .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                    .collect()
            } else {
                self.unjoinable_picks(window.picks)
            };
            let action =
                if (window.modifiers.ctrl || window.modifiers.command) && !window.modifiers.shift {
                    ComponentAction::Remove
                } else {
                    ComponentAction::Add
                };
            self.select_components(picks, action);
        } else {
            self.accept_hole_rectangle(window.picks);
        }
        self.selection_window_override = None;
    }

    pub(super) fn try_continue_component_choice(&mut self, input: &str) -> bool {
        if !self.component_selection.has_choices() {
            return false;
        }
        if input.is_empty() || input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.component_selection.clear_choices();
            self.command_input.clear();
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|word| self.commands.recognizes(word))
        {
            self.component_selection.clear_choices();
            return false;
        }
        let candidate = input
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| self.component_selection.candidates.get(n))
            .cloned();
        match candidate {
            Some((pick, snapshot)) if current(&self.document, pick, &snapshot) => {
                self.select_components(vec![pick], self.component_selection.choice_action)
            }
            Some(_) => {
                self.component_selection.clear_choices();
                self.push_log("Source changed; pick the component again".into());
            }
            None => self.push_log("Choose a component by its listed number".into()),
        }
        self.command_input.clear();
        true
    }

    pub(super) fn show_component_choices(&mut self, ui: &mut egui::Ui) {
        if !self.component_selection.has_choices()
            || self.plane_prompt.is_some()
            || self.set_view_prompt.is_some()
        {
            return;
        }
        let mut chosen = None;
        let mut hover = None;
        ui.horizontal_wrapped(|ui| {
            for (index, (pick, _)) in self.component_selection.candidates.iter().enumerate() {
                let button = ui.button((index + 1).to_string());
                if button.hovered() {
                    hover = Some(*pick);
                }
                if button.clicked() {
                    chosen = Some((index + 1).to_string());
                }
            }
            if ui.button("Cancel").clicked() {
                chosen = Some("Cancel".into());
            }
        });
        self.component_selection.hover = hover;
        if let Some(input) = chosen {
            self.try_continue_component_choice(&input);
        }
    }
}
