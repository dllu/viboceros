//! Component picking and staged extrusion previews leave history untouched.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    extrude_subd::{self as solver, Options, Plan, Target},
};
use viboceros_geometry::SubDExtrudeSelection;
pub(super) struct Session {
    options: Options,
    targets: Option<Vec<Target>>,
    plan: Option<Plan>,
    edges: Vec<viboceros_geometry::LineSegment>,
}
impl Session {
    pub(super) fn edges(&self) -> &[viboceros_geometry::LineSegment] {
        &self.edges
    }
}
pub(super) fn eligible(doc: &Document, p: &ComponentPick) -> bool {
    match doc.object(p.object).map(|o| o.geometry()) {
        Some(Geometry::SubD(s)) => match p.kind {
            ComponentSelectionKind::SubDFace => p.index < s.faces().len(),
            ComponentSelectionKind::SubDEdge => s.edge_face_counts().get(p.index) == Some(&1),
            _ => false,
        },
        _ => false,
    }
}
impl VibocerosApp {
    pub(super) fn picking_extrude_subd(&self) -> bool {
        self.extrude_subd_session
            .as_ref()
            .is_some_and(|s| s.targets.is_none())
    }
    fn extrusion_targets(
        &self,
        selection: Option<&SubDExtrudeSelection>,
    ) -> Result<Vec<Target>, CommandError> {
        let mut targets = solver::selected_targets(&self.document, selection)?;
        for p in self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| CommandError::ExtrudeSubDStale)?
        {
            if targets.len() >= 400_000 {
                return Err(CommandError::Usage(
                    "Too many ExtrudeSubD component targets",
                ));
            }
            if eligible(&self.document, &p) {
                targets.push(if p.kind == ComponentSelectionKind::SubDFace {
                    Target::Face(p.object, p.index)
                } else {
                    Target::Edge(p.object, p.index)
                });
            }
        }
        Ok(targets)
    }
    pub(super) fn start_extrude_subd(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSubD")
        }) {
            return false;
        }
        let parsed = match solver::parse(&words[1..], self.commands.extrude_subd_options()) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let targets = match self.extrusion_targets(parsed.selection.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
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
                    return true;
                }
            }
        };
        self.cancel_interactive_command(false);
        self.commands.remember_extrude_subd_options(parsed.options);
        if parsed.distance_supplied
            && let Some((plan, _)) = prepared
        {
            match plan.commit(&mut self.document) {
                Ok(message) => {
                    self.component_selection.clear();
                    self.push_log(message);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            };
            return true;
        }
        let (plan, edges) = prepared.map_or((None, vec![]), |(p, e)| (Some(p), e));
        self.extrude_subd_session = Some(Session {
            options: parsed.options,
            targets: (!targets.is_empty()).then_some(targets),
            plan,
            edges,
        });
        self.active_command = Some(InteractiveCommand::ExtrudeSubD);
        self.log_extrude_subd();
        true
    }
    fn log_extrude_subd(&mut self) {
        if let Some(s) = &self.extrude_subd_session {
            let text = if s.targets.is_none() {
                "select SubD faces or naked edges; Ctrl/Command removes; Enter accepts selection"
            } else {
                "enter distance or Basis/Direction; Enter commits"
            };
            self.push_log(format!("{}; {text}; Esc cancels", s.options.command()));
        }
    }
    pub(super) fn continue_extrude_subd(&mut self, input: &str) -> bool {
        if self.picking_extrude_subd()
            && self.component_selection.has_choices()
            && self.try_continue_component_choice(input)
        {
            return true;
        }

        let Some(mut s) = self.extrude_subd_session.take() else {
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
                match plan.commit(&mut self.document) {
                    Ok(message) => {
                        self.component_selection.clear();
                        self.active_command = None;
                        self.push_log(message);
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        self.extrude_subd_session = Some(s);
                    }
                }
            } else {
                let result = self.extrusion_targets(None).and_then(|targets| {
                    Plan::prepare(&self.document, targets.clone(), s.options)
                        .and_then(|p| p.preview_edges().map(|e| (targets, p, e)))
                });
                match result {
                    Ok((targets, plan, edges)) => {
                        s.targets = Some(targets);
                        s.plan = Some(plan);
                        s.edges = edges;
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                };
                self.extrude_subd_session = Some(s);
                self.log_extrude_subd();
            }
        } else {
            let result = solver::parse(&input.split_whitespace().collect::<Vec<_>>(), s.options)
                .and_then(|p| {
                    if p.selection.is_some() {
                        return Err(CommandError::Usage(
                            "Select components before entering extrusion options",
                        ));
                    }
                    if let Some(targets) = &s.targets {
                        if !s.plan.as_ref().unwrap().is_current(&self.document) {
                            return Err(CommandError::ExtrudeSubDStale);
                        }
                        let plan = Plan::prepare(&self.document, targets.clone(), p.options)?;
                        let edges = plan.preview_edges()?;
                        s.plan = Some(plan);
                        s.edges = edges;
                    }
                    s.options = p.options;
                    self.commands.remember_extrude_subd_options(p.options);
                    Ok(())
                });
            if let Err(e) = result {
                self.push_log(format!("Error: {e}"));
            }
            self.extrude_subd_session = Some(s);
            self.log_extrude_subd();
        }
        self.command_input.clear();
        true
    }
}
