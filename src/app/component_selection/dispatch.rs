//! Component events route to active getters or transient preselection.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn component_preselection_available(&self) -> bool {
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
                || self.picking_extract_points()
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
    pub(in crate::app) fn accept_component_click(&mut self, click: ComponentClick) {
        if self.accept_mesh_conversion_face_click(&click) {
            return;
        }
        if !click.preselection
            && self.unjoin_prompt.is_none()
            && self.shell_prompt.is_none()
            && self.edge_fillet_prompt.is_none()
            && self.crease_prompt.is_none()
            && self.soft_crease_prompt.is_none()
            && !self.picking_extract_points()
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
        } else if let Some(prompt) = &self.crease_prompt {
            click
                .picks
                .into_iter()
                .filter(|p| prompt.eligible(p.kind))
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
        let picks = if self.picking_extract_points() {
            picks
                .into_iter()
                .filter(|p| super::extract_points::component_kind(p.kind).is_some())
                .collect()
        } else if self.picking_to_subd() {
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
                            ComponentSelectionKind::MeshVertex => "mesh vertex",
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
        if self.picking_spin_edge() {
            self.pick_spin_edge(&picks);
            return;
        }

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
        let parents = picks
            .iter()
            .filter(|pick| {
                !(self
                    .crease_prompt
                    .as_ref()
                    .is_some_and(|p| p.preserves_grip_owners())
                    && self.document.grip_mode(pick.object).is_some())
            })
            .map(|pick| pick.object)
            .collect::<Vec<_>>();
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

    pub(in crate::app) fn accept_component_window(&mut self, mut window: ComponentWindow) {
        if self.picking_extract_points() {
            window
                .picks
                .retain(|p| super::extract_points::component_kind(p.kind).is_some());
        }
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
        if let Some(prompt) = &self.crease_prompt {
            window.picks.retain(|p| prompt.eligible(p.kind));
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
        } else if self.crease_prompt.is_some()
            || self.picking_soft_crease_edges()
            || self.picking_extract_points()
        {
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

    pub(in crate::app) fn try_continue_component_choice(&mut self, input: &str) -> bool {
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

    pub(in crate::app) fn show_component_choices(&mut self, ui: &mut egui::Ui) {
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
