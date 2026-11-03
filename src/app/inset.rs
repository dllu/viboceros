//! Face picking, positive-distance previews and retained inset cap selection.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    inset::{self as solver, Options, Plan, Witness},
};
pub(super) struct Session {
    options: Options,
    targets: Option<Vec<(ObjectId, usize)>>,
    plan: Option<Plan>,
    witness: Option<Witness>,
    edges: Vec<viboceros_geometry::LineSegment>,
    mesh: bool,
    surface: bool,
    distance_supplied: bool,
    crease_supplied: bool,
}
impl Session {
    pub(super) fn edges(&self) -> &[viboceros_geometry::LineSegment] {
        &self.edges
    }
}
pub(super) fn eligible(doc: &Document, p: &ComponentPick) -> bool {
    doc.object(p.object)
        .is_some_and(|o| match (p.kind, o.geometry()) {
            (ComponentSelectionKind::SubDFace, Geometry::SubD(s)) => p.index < s.faces().len(),
            (ComponentSelectionKind::MeshFace, Geometry::Mesh(m)) => p.index < m.face_count(),
            (ComponentSelectionKind::BrepFace, Geometry::Brep(b)) => p.index < b.faces().len(),
            (ComponentSelectionKind::BrepFace, Geometry::NurbsSurface(_)) => p.index == 0,
            _ => false,
        })
}
impl VibocerosApp {
    pub(super) fn replaying_inset_faces(&self, input: &str) -> bool {
        let name = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        (name.eq_ignore_ascii_case("Undo") && self.document.undo_label() == Some("Inset"))
            || (name.eq_ignore_ascii_case("Redo") && self.document.redo_label() == Some("Inset"))
    }

    pub(super) fn picking_inset(&self) -> bool {
        self.inset_session
            .as_ref()
            .is_some_and(|s| s.targets.is_none())
    }
    fn inset_targets(
        &self,
        faces: Option<&[usize]>,
        selected_objects: bool,
    ) -> Result<Vec<(ObjectId, usize)>, CommandError> {
        let mut targets = if selected_objects {
            solver::selected_targets(&self.document, faces)?
        } else {
            vec![]
        };
        for p in self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| CommandError::InsetStale)?
        {
            if targets.len() >= 1_000_000 {
                return Err(CommandError::Usage("Too many Inset face targets"));
            }
            if eligible(&self.document, &p) {
                targets.push((p.object, p.index));
            }
        }
        Ok(targets)
    }
    fn inset_mesh_only(&self, targets: &[(ObjectId, usize)]) -> bool {
        !targets.is_empty()
            && targets.iter().all(|(id, _)| {
                self.document
                    .object(*id)
                    .is_some_and(|o| matches!(o.geometry(), Geometry::Mesh(_)))
            })
    }
    fn inset_surface_only(&self, targets: &[(ObjectId, usize)]) -> bool {
        !targets.is_empty()
            && targets.iter().all(|(id, _)| {
                self.document.object(*id).is_some_and(|o| {
                    matches!(o.geometry(), Geometry::Brep(_) | Geometry::NurbsSurface(_))
                })
            })
    }
    pub(super) fn start_inset(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Inset")
        }) {
            return false;
        }
        let targets = match self.inset_targets(None, false) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let mesh = self.inset_mesh_only(&targets)
            || (targets.is_empty()
                && self.document.selected_object_count() > 0
                && self
                    .document
                    .selected_objects()
                    .all(|o| matches!(o.geometry(), Geometry::Mesh(_))));
        let surface = self.inset_surface_only(&targets)
            || (targets.is_empty()
                && self.document.selected_object_count() > 0
                && self.document.selected_objects().all(|o| {
                    matches!(o.geometry(), Geometry::Brep(_) | Geometry::NurbsSurface(_))
                }));
        let defaults = if surface {
            self.commands.inset_surface_options()
        } else {
            self.commands.inset_options(mesh)
        };
        let parsed = match solver::parse(&words[1..], defaults) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let targets = if parsed.faces_supplied || surface && targets.is_empty() {
            match self.inset_targets(parsed.faces.as_deref(), true) {
                Ok(t) => t,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        } else {
            targets
        };
        if !surface {
            self.document.clear_selection();
        }
        let witness = if targets.is_empty() {
            None
        } else {
            match Witness::capture(&self.document, targets.iter().map(|(id, _)| *id)) {
                Ok(w) => Some(w),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        };
        let prepared = if targets.is_empty() {
            None
        } else {
            match Plan::prepare(&self.document, targets.clone(), parsed.options)
                .and_then(|p| p.preview_edges().map(|e| (p, e)))
            {
                Ok(p) => Some(p),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    if parsed.distance_supplied {
                        return true;
                    }
                    None
                }
            }
        };
        self.cancel_interactive_command(false);
        if parsed.distance_supplied
            && let Some((plan, _)) = prepared
        {
            self.commit_inset(&plan, parsed.options, mesh, surface);
            return true;
        }
        let (plan, edges) = prepared.map_or((None, vec![]), |(p, e)| (Some(p), e));
        self.inset_session = Some(Session {
            options: parsed.options,
            targets: (!targets.is_empty()).then_some(targets),
            plan,
            witness,
            edges,
            mesh,
            surface,
            distance_supplied: parsed.distance_supplied,
            crease_supplied: parsed.crease_supplied,
        });
        self.active_command = Some(InteractiveCommand::Inset);
        self.log_inset();
        true
    }
    fn commit_inset(&mut self, plan: &Plan, options: Options, mesh: bool, surface: bool) -> bool {
        match plan.commit(&mut self.document) {
            Ok(message) => {
                if surface {
                    self.commands.remember_inset_surface_options(options);
                    self.component_selection.clear();
                } else {
                    self.commands.remember_inset_options(options, mesh);
                }
                let picks = plan
                    .faces()
                    .iter()
                    .map(|f| ComponentPick {
                        object: f.object,
                        index: f.index,
                        kind: f.kind,
                    })
                    .collect();
                if !surface
                    && let Err(e) = self
                        .component_selection
                        .replace_faces(&self.document, picks)
                {
                    self.push_log(format!("Error: {e}"));
                }
                self.active_command = None;
                self.push_log(message);
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    fn log_inset(&mut self) {
        if let Some(s) = &self.inset_session {
            let message = if s.targets.is_none() {
                "Inset: select surface, mesh or SubD faces; Ctrl/Command removes; Enter accepts selection; Esc cancels".into()
            } else if s.surface {
                format!(
                    "Inset Distance={} CreaseCorners={}; enter a positive distance or CreaseCorners=Yes|No; Enter commits, Esc cancels",
                    s.options.distance,
                    if s.options.crease_corners {
                        "Yes"
                    } else {
                        "No"
                    }
                )
            } else if s.mesh {
                format!(
                    "Inset Distance={}; enter a positive distance; Enter commits, Esc cancels",
                    s.options.distance
                )
            } else {
                format!(
                    "{}; enter a positive distance or Mode=Group|Single; Enter commits, Esc cancels",
                    s.options.command()
                )
            };
            self.push_log(message);
        }
    }
    fn prepare_inset_selection(&self, s: &mut Session) -> Result<(), CommandError> {
        if s.targets.is_none() {
            let targets = self.inset_targets(None, false)?;
            let witness = Witness::capture(&self.document, targets.iter().map(|(id, _)| *id))?;
            s.mesh = self.inset_mesh_only(&targets);
            s.surface = self.inset_surface_only(&targets);
            let defaults = if s.surface {
                self.commands.inset_surface_options()
            } else {
                self.commands.inset_options(s.mesh)
            };
            if !s.distance_supplied {
                s.options.distance = defaults.distance;
            }
            if !s.crease_supplied {
                s.options.crease_corners = defaults.crease_corners;
            }
            s.targets = Some(targets);
            s.witness = Some(witness);
        }
        if !s.witness.as_ref().unwrap().is_current(&self.document) {
            return Err(CommandError::InsetStale);
        }
        let plan = Plan::prepare(
            &self.document,
            s.targets.as_ref().unwrap().iter().copied(),
            s.options,
        )?;
        s.edges = plan.preview_edges()?;
        s.plan = Some(plan);
        Ok(())
    }
    pub(super) fn continue_inset(&mut self, input: &str) -> bool {
        if self.picking_inset()
            && self.component_selection.has_choices()
            && self.try_continue_component_choice(input)
        {
            return true;
        }
        let Some(mut s) = self.inset_session.take() else {
            return false;
        };
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.component_selection.clear();
            self.active_command = None;
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|n| self.commands.recognizes(n))
        {
            self.component_selection.clear();
            self.active_command = None;
            return false;
        } else if input.is_empty() {
            if let Some(plan) = &s.plan {
                if !self.commit_inset(plan, s.options, s.mesh, s.surface) {
                    self.inset_session = Some(s);
                }
            } else {
                if let Err(e) = self.prepare_inset_selection(&mut s) {
                    self.push_log(format!("Error: {e}"));
                }
                self.inset_session = Some(s);
                self.log_inset();
            }
        } else {
            let result = solver::parse(&input.split_whitespace().collect::<Vec<_>>(), s.options)
                .and_then(|p| {
                    if p.faces_supplied {
                        return Err(CommandError::Usage(
                            "Select faces before entering Inset options",
                        ));
                    }
                    if let Some(targets) = &s.targets {
                        if !s.witness.as_ref().unwrap().is_current(&self.document) {
                            return Err(CommandError::InsetStale);
                        }
                        let plan = match Plan::prepare(&self.document, targets.clone(), p.options) {
                            Ok(plan) => plan,
                            Err(e) => {
                                if s.plan.is_none() {
                                    s.options = p.options;
                                    s.distance_supplied |= p.distance_supplied;
                                    s.crease_supplied |= p.crease_supplied;
                                }
                                return Err(e);
                            }
                        };
                        let edges = plan.preview_edges()?;
                        s.plan = Some(plan);
                        s.edges = edges;
                    }
                    s.options = p.options;
                    s.distance_supplied |= p.distance_supplied;
                    s.crease_supplied |= p.crease_supplied;
                    Ok(())
                });
            if let Err(e) = result {
                self.push_log(format!("Error: {e}"));
            }
            self.inset_session = Some(s);
            self.log_inset();
        }
        self.command_input.clear();
        true
    }
}
