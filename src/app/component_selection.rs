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
    pub(super) fn release_objects(&mut self, ids: &[ObjectId]) {
        let ids = ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        self.picks.retain(|p, _| !ids.contains(&p.object));
        self.order.retain(|p| !ids.contains(&p.object));
        self.clear_choices();
    }
    pub(super) fn release_grip_vertices(&mut self, document: &Document) {
        let owners = document
            .all_control_points()
            .map(|(g, _, _)| g.object)
            .collect::<std::collections::BTreeSet<_>>();
        self.picks.retain(|p, _| {
            p.kind != ComponentSelectionKind::SubDVertex || !owners.contains(&p.object)
        });
        self.order.retain(|p| self.picks.contains_key(p));
    }
    pub(super) fn has_choices(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub(super) fn valid_picks(&mut self, document: &Document) -> Vec<ComponentPick> {
        if self.picks.is_empty() {
            return Vec::new();
        }
        let snapshots = selectable_snapshots(document);
        self.picks
            .retain(|pick, snapshot| snapshots.get(&pick.object) == Some(snapshot));
        self.order.retain(|pick| self.picks.contains_key(pick));
        self.picks.keys().copied().collect()
    }
    pub(super) fn highlights(&mut self, document: &Document) -> Vec<ComponentPick> {
        if self.picks.is_empty() && self.candidates.is_empty() {
            return Vec::new();
        }
        let snapshots = selectable_snapshots(document);
        let mut picks = self
            .picks
            .iter()
            .filter(|(pick, snapshot)| snapshots.get(&pick.object) == Some(*snapshot))
            .map(|(pick, _)| *pick)
            .collect::<Vec<_>>();
        if let Some(hover) = self.hover.filter(|hover| {
            self.candidates.iter().any(|(pick, snapshot)| {
                pick == hover && snapshots.get(&pick.object) == Some(snapshot)
            })
        }) {
            picks.push(hover);
        } else {
            picks.extend(
                self.candidates
                    .iter()
                    .filter(|(pick, snapshot)| snapshots.get(&pick.object) == Some(snapshot))
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
        if self.picks.is_empty() {
            return Ok(Vec::new());
        }
        let snapshots = selectable_snapshots(document);
        if self
            .picks
            .iter()
            .any(|(pick, snapshot)| snapshots.get(&pick.object) != Some(snapshot))
        {
            return Err("component source changed; select the components again");
        }
        Ok(self.order.clone())
    }
    pub(super) fn replace_subdivision_faces(
        &mut self,
        document: &Document,
        faces: Vec<(ObjectId, usize)>,
    ) -> Result<(), &'static str> {
        let picks = faces
            .into_iter()
            .map(|(object, index)| ComponentPick {
                object,
                index,
                kind: ComponentSelectionKind::SubDFace,
            })
            .collect();
        self.clear();
        self.apply(document, picks, ComponentAction::Add)
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
        let staged = stage_sources(document, picks)?;
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

fn selectable_snapshots(document: &Document) -> BTreeMap<ObjectId, GeometrySnapshot> {
    document
        .selectable_objects()
        .map(|o| (o.id(), o.geometry_snapshot().clone()))
        .collect()
}

fn current(document: &Document, pick: ComponentPick, snapshot: &GeometrySnapshot) -> bool {
    document.is_object_selectable(pick.object)
        && document
            .object(pick.object)
            .is_some_and(|object| object.geometry_snapshot() == snapshot)
}

fn stage_sources(
    document: &Document,
    picks: Vec<ComponentPick>,
) -> Result<Vec<(ComponentPick, GeometrySnapshot)>, &'static str> {
    if picks.is_empty() {
        return Ok(Vec::new());
    }
    let objects = document
        .selectable_objects()
        .map(|o| (o.id(), o))
        .collect::<BTreeMap<_, _>>();
    let mut counts = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut staged = Vec::new();
    for pick in picks.into_iter().filter(|p| seen.insert(*p)) {
        let object = objects
            .get(&pick.object)
            .ok_or("component source is unavailable")?;
        let count = if let Some(&count) = counts.get(&(pick.object, pick.kind)) {
            count
        } else {
            let count = match (pick.kind, object.geometry()) {
                (ComponentSelectionKind::MeshFace, Geometry::Mesh(m)) => m.face_count(),
                (ComponentSelectionKind::MeshEdge, Geometry::Mesh(m)) => {
                    m.topology_edge_points().len()
                }
                (ComponentSelectionKind::SubDFace, Geometry::SubD(s)) => s.faces().len(),
                (ComponentSelectionKind::SubDEdge, Geometry::SubD(s)) => s.edges().len(),
                (ComponentSelectionKind::SubDVertex, Geometry::SubD(s)) => s.control_points().len(),
                (ComponentSelectionKind::BrepEdge, Geometry::Brep(b)) => b.edges().len(),
                (ComponentSelectionKind::BrepFace, Geometry::Brep(b)) => b.faces().len(),
                (ComponentSelectionKind::BrepEdge, Geometry::NurbsSurface(s)) => {
                    viboceros_geometry::Brep::try_surface_face(s.clone(), document.tolerance())
                        .map_err(|_| "invalid component surface")?
                        .edges()
                        .len()
                }
                (ComponentSelectionKind::BrepFace, Geometry::NurbsSurface(surface)) => {
                    viboceros_geometry::Brep::try_surface_face(
                        surface.clone(),
                        document.tolerance(),
                    )
                    .map_err(|_| "invalid component surface")?
                    .faces()
                    .len()
                }
                _ => return Err("component kind does not match its source"),
            };
            counts.insert((pick.object, pick.kind), count);
            count
        };
        if pick.index >= count {
            return Err("component index is outside its source");
        }
        staged.push((pick, object.geometry_snapshot().clone()));
    }
    Ok(staged)
}

impl VibocerosApp {
    pub(super) fn component_preselection_available(&self) -> bool {
        self.active_command.is_none()
            && self.surface_path_prompt.is_none()
            && self.shell_prompt.is_none()
            && self.merge_face_prompt.is_none()
            && self.boolean_two_prompt.is_none()
            && self.planar_boolean_prompt.is_none()
            && self.tween_surfaces_prompt.is_none()
            && self.blend_surface_prompt.is_none()
            && self.fillet_surface_prompt.is_none()
            && self.reparameterize_prompt.is_none()
            && self.selection_menu.is_none()
            && self.zoom_factor_pending.is_none()
            && self.snap_size_pending.is_none()
            && (self.object_prompt.is_none()
                || self.picking_to_subd()
                || self.picking_subdivide()
                || self.shrink_prompt_mode()
                    == Some(viboceros_geometry::BrepSurfaceShrinkMode::Standard))
            && self.group_prompt.is_none()
            && self.intersection_prompt.is_none()
            && self.edge_prompt.is_none()
            && self.hole_prompt.is_none()
            && self.unjoin_prompt.is_none()
            && self.edge_fillet_prompt.is_none()
            && self.crease_prompt.is_none()
            && self.soft_crease_prompt.is_none()
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
            && self.shell_prompt.is_none()
            && self.edge_fillet_prompt.is_none()
            && self.crease_prompt.is_none()
            && self.soft_crease_prompt.is_none()
            && !self.picking_extract_faces()
            && !self.picking_subcurve_edge()
            && !self.picking_orient_curve()
            && !self.picking_surface_offset_curve()
            && !self.picking_orient_edge()
            && !self.picking_array_curve()
            && !self.picking_curve_array()
            && !self.picking_flow_guide()
            && !self.picking_spiral_rail()
            && !self.picking_conic_curve()
            && !self.picking_rectangle_curve()
            && !self.picking_ellipse_curve()
            && !self.picking_arc_extension_curve()
            && !self.picking_tangent_curve()
            && !self.picking_circle_around_curve()
            && !self.picking_splop_reference_curve()
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
        let picks = if self.picking_subcurve_edge()
            || self.picking_orient_curve()
            || self.picking_surface_offset_curve()
            || self.picking_orient_edge()
            || self.picking_array_curve()
            || self.picking_curve_array()
            || self.picking_flow_guide()
            || self.picking_spiral_rail()
            || self.picking_conic_curve()
            || self.picking_rectangle_curve()
            || self.picking_ellipse_curve()
            || self.picking_arc_extension_curve()
            || self.picking_tangent_curve()
            || self.picking_circle_around_curve()
            || self.picking_splop_reference_curve()
        {
            click
                .picks
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
                .collect()
        } else if self.picking_soft_crease_edges() {
            click
                .picks
                .into_iter()
                .filter(|pick| soft_crease::eligible_edge(&self.document, pick))
                .collect()
        } else if self.crease_prompt.is_some() {
            click
                .picks
                .into_iter()
                .filter(|p| crease::eligible(p.kind))
                .collect()
        } else if self.shell_prompt.is_some() {
            self.shellable_picks(click.picks)
        } else if self.picking_extract_faces() {
            click
                .picks
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                .collect()
        } else if self.unjoin_prompt.is_some() || self.edge_fillet_prompt.is_some() {
            self.unjoinable_picks(click.picks)
        } else {
            click.picks
        };
        let picks = if self.picking_to_subd() {
            picks
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                .collect()
        } else if self.picking_subdivide() {
            picks
                .into_iter()
                .filter(|p| {
                    matches!(
                        p.kind,
                        ComponentSelectionKind::MeshFace | ComponentSelectionKind::SubDFace
                    )
                })
                .collect()
        } else {
            picks
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
            _ => match stage_sources(&self.document, picks) {
                Ok(candidates) => {
                    for (index, (pick, _)) in candidates.iter().enumerate() {
                        let kind = match pick.kind {
                            ComponentSelectionKind::BrepEdge => "edge",
                            ComponentSelectionKind::BrepFace => "face",
                            ComponentSelectionKind::MeshFace => "mesh face",
                            ComponentSelectionKind::SubDFace => "SubD face",
                            ComponentSelectionKind::MeshEdge => "mesh edge",
                            ComponentSelectionKind::SubDEdge => "SubD edge",
                            ComponentSelectionKind::SubDVertex => "SubD vertex",
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
            },
        }
    }

    fn select_components(&mut self, picks: Vec<ComponentPick>, action: ComponentAction) {
        if self.picking_spiral_rail() {
            if let [pick] = picks.as_slice()
                && pick.kind == viboceros_command::ComponentSelectionKind::BrepEdge
            {
                self.pick_spiral_rail(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_conic_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_conic_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_arc_extension_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_arc_extension_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_tangent_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_tangent_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_splop_reference_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_splop_reference_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_circle_around_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_circle_around_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_ellipse_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_ellipse_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_rectangle_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_rectangle_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_flow_guide() {
            if let [pick] = picks.as_slice() {
                self.pick_flow_guide(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_curve_array() {
            if let [pick] = picks.as_slice() {
                self.pick_curve_array(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_array_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_array_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_surface_offset_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_surface_offset_curve(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_orient_edge() {
            if let [pick] = picks.as_slice() {
                self.pick_curve_to_edge(Some(pick.object), Some(pick.index));
            }
            return;
        }
        if self.picking_orient_curve() {
            if let [pick] = picks.as_slice() {
                self.pick_orient_edge(crate::viewport::EdgePick {
                    object: pick.object,
                    edge: pick.index,
                });
            }
            return;
        }
        if self.picking_subcurve_edge() {
            if let [pick] = picks.as_slice() {
                self.pick_subcurve_edge(crate::viewport::EdgePick {
                    object: pick.object,
                    edge: pick.index,
                });
            }
            return;
        }
        let grips = picks
            .iter()
            .filter(|p| p.kind == ComponentSelectionKind::SubDVertex)
            .filter(|p| self.document.grip_mode(p.object).is_some())
            .map(|p| {
                (
                    *p,
                    viboceros_document::ControlPointId {
                        object: p.object,
                        index: p.index,
                    },
                )
            })
            .collect::<Vec<_>>();
        let parents = picks.iter().map(|pick| pick.object).collect::<Vec<_>>();
        match self
            .component_selection
            .apply(&self.document, picks, action)
        {
            Ok(()) => {
                for mode in [SelectionMode::Add, SelectionMode::Remove] {
                    let selected = mode == SelectionMode::Add;
                    let ids = grips
                        .iter()
                        .filter(|(p, _)| self.component_selection.picks.contains_key(p) == selected)
                        .map(|(_, g)| *g)
                        .collect::<Vec<_>>();
                    if !ids.is_empty()
                        && let Err(e) = self.document.select_control_points(ids, mode)
                    {
                        self.push_log(format!("Error: {e}"));
                    }
                }
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

    pub(super) fn accept_component_window(&mut self, mut window: ComponentWindow) {
        if self.picking_to_subd() {
            window
                .picks
                .retain(|p| p.kind == ComponentSelectionKind::BrepFace);
        }
        if self.picking_subdivide() {
            window.picks.retain(|p| {
                matches!(
                    p.kind,
                    ComponentSelectionKind::MeshFace | ComponentSelectionKind::SubDFace
                )
            });
        }
        if self.crease_prompt.is_some() {
            window.picks.retain(|p| crease::eligible(p.kind));
        }
        if self.picking_soft_crease_edges() {
            window
                .picks
                .retain(|pick| soft_crease::eligible_edge(&self.document, pick));
        }
        if window.preselection {
            if self.component_preselection_available() {
                self.select_components(window.picks, ComponentAction::Add);
            }
        } else if self.crease_prompt.is_some() || self.picking_soft_crease_edges() {
            let action =
                if (window.modifiers.ctrl || window.modifiers.command) && !window.modifiers.shift {
                    ComponentAction::Remove
                } else {
                    ComponentAction::Add
                };
            self.select_components(window.picks, action);
        } else if self.unjoin_prompt.is_some()
            || self.edge_fillet_prompt.is_some()
            || self.shell_prompt.is_some()
            || self.picking_extract_faces()
        {
            let picks = if self.shell_prompt.is_some() {
                self.shellable_picks(window.picks)
            } else if self.picking_extract_faces() {
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
