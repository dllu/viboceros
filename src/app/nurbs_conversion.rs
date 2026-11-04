//! Mesh/SubD edge targets join whole-object ToNURBS confirmation without edits.
use super::object_selection::{ObjectPromptPhase, PendingObjectCommand};
use super::*;
use viboceros_command::{ComponentSelectionKind, to_nurbs::ToNurbsTarget};
use viboceros_document::{GeometrySnapshot, GroupId, ObjectAttributes};
#[derive(Clone, Debug, PartialEq)]
struct Witness {
    sources: Vec<(ObjectId, GeometrySnapshot, ObjectAttributes, Vec<GroupId>)>,
    selected: Vec<ObjectId>,
    tolerance: Tolerance,
    control_net: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Session {
    order: Vec<ToNurbsTarget>,
    witness: Option<Witness>,
}
impl Session {
    fn pin(&mut self, doc: &Document) {
        let ids = self
            .order
            .iter()
            .map(|t| t.object())
            .collect::<std::collections::BTreeSet<_>>();
        self.witness = Some(Witness {
            sources: ids
                .into_iter()
                .map(|id| {
                    let o = doc.object(id).unwrap();
                    (
                        id,
                        o.geometry_snapshot().clone(),
                        o.attributes().clone(),
                        o.group_ids().to_vec(),
                    )
                })
                .collect(),
            selected: doc.selected_object_ids().collect(),
            tolerance: doc.tolerance(),
            control_net: doc.subd_control_net(),
        });
    }
    fn current(&self, doc: &Document) -> bool {
        self.witness.as_ref().is_some_and(|w| {
            w.tolerance == doc.tolerance()
                && doc.selected_object_ids().eq(w.selected.iter().copied())
                && w.sources.iter().all(|(id, s, a, g)| {
                    doc.is_object_selectable(*id)
                        && doc.object(*id).is_some_and(|o| {
                            o.geometry_snapshot() == s && o.attributes() == a && o.group_ids() == g
                        })
                })
        })
    }
}
pub(super) fn eligible(kind: ComponentSelectionKind) -> bool {
    matches!(
        kind,
        ComponentSelectionKind::MeshEdge | ComponentSelectionKind::SubDEdge
    )
}
impl VibocerosApp {
    pub(super) fn picking_nurbs_edges(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.description.command == "ToNURBS" && p.phase == ObjectPromptPhase::Selecting
        })
    }
    fn nurbs_targets(&self) -> Result<Vec<ToNurbsTarget>, String> {
        let mut targets = self
            .document
            .selected_object_ids()
            .map(ToNurbsTarget::Whole)
            .collect::<Vec<_>>();
        targets.extend(
            self.component_selection
                .checked_picks(&self.document)
                .map_err(str::to_owned)?
                .into_iter()
                .filter(|p| eligible(p.kind))
                .map(|p| ToNurbsTarget::Edge(p.object, p.index)),
        );
        Ok(targets)
    }
    pub(super) fn record_nurbs_targets(&mut self) {
        if !self.picking_nurbs_edges() {
            return;
        }
        let Ok(current) = self.nurbs_targets() else {
            return;
        };
        let pending = self.object_prompt.as_mut().unwrap();
        if pending.to_nurbs.is_none()
            && !current.iter().any(|t| matches!(t, ToNurbsTarget::Edge(..)))
        {
            return;
        }
        let session = pending.to_nurbs.get_or_insert_with(Session::default);
        let set = current
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        session.order.retain(|t| set.contains(t));
        let mut seen = session
            .order
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        session
            .order
            .extend(current.into_iter().filter(|t| seen.insert(*t)));
    }
    pub(super) fn renew_nurbs_edge_order(&mut self, owner: ObjectId) {
        if !self.picking_nurbs_edges() {
            return;
        }
        let Ok(fresh) = self.nurbs_targets() else {
            return;
        };
        if let Some(session) = self
            .object_prompt
            .as_mut()
            .and_then(|p| p.to_nurbs.as_mut())
        {
            let belongs = |t: &ToNurbsTarget| matches!(t,ToNurbsTarget::Edge(id,_) if *id==owner);
            session.order.retain(|t| !belongs(t));
            session.order.extend(fresh.into_iter().filter(belongs));
        }
    }
    pub(super) fn try_start_nurbs_edges(&mut self, input: &str) -> bool {
        if !input.split_whitespace().next().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ToNURBS")
        }) {
            return false;
        }
        self.component_selection.valid_picks(&self.document);
        let Ok(targets) = self.nurbs_targets() else {
            return false;
        };
        if !targets.iter().any(|t| matches!(t, ToNurbsTarget::Edge(..))) {
            return false;
        }
        // Explicit scripted edge selectors bypass interactive input.
        match self.commands.object_selection_prompt(input) {
            Ok(None) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            _ => {}
        }
        let description =
            match self
                .commands
                .to_nurbs_target_confirmation(&self.document, input, &targets)
            {
                Ok(Some(p)) => p,
                Ok(None) => return false,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            };
        let components = std::mem::take(&mut self.component_selection);
        self.cancel_interactive_command(false);
        self.component_selection = components;
        let mut session = Session {
            order: targets,
            witness: None,
        };
        session.pin(&self.document);
        let mut pending = PendingObjectCommand::selecting(description);
        pending.phase = ObjectPromptPhase::Options;
        pending.postselected = false;
        pending.to_nurbs = Some(session);
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
    pub(super) fn finish_nurbs_edges(&mut self) -> bool {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.description.command != "ToNURBS")
        {
            return false;
        }
        if self.picking_nurbs_edges() {
            self.record_nurbs_targets();
        }
        let mut pending = self.object_prompt.as_ref().unwrap().clone();
        let Some(mut session) = pending.to_nurbs.take() else {
            return false;
        };
        match pending.phase {
            ObjectPromptPhase::Selecting => {
                if session.order.is_empty() {
                    return false;
                }
                let input = pending.description.command_line();
                match self.commands.to_nurbs_target_confirmation(
                    &self.document,
                    &input,
                    &session.order,
                ) {
                    Ok(Some(description)) => {
                        session.pin(&self.document);
                        pending.description = description;
                        pending.phase = ObjectPromptPhase::Options;
                        pending.to_nurbs = Some(session);
                        self.object_prompt = Some(pending);
                        self.log_object_prompt();
                    }
                    Ok(None) => {
                        match self.commands.execute_to_nurbs_targets(
                            &mut self.document,
                            &input,
                            &session.order,
                            pending.postselected,
                        ) {
                            Ok(message) => {
                                self.object_prompt = None;
                                self.component_selection.clear();
                                self.push_log(message)
                            }
                            Err(e) => self.push_log(format!("Error: {e}")),
                        }
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            ObjectPromptPhase::Options => {
                let same_targets = self.nurbs_targets().is_ok_and(|t| {
                    t.into_iter().collect::<std::collections::BTreeSet<_>>()
                        == session.order.iter().copied().collect()
                });
                if !session.current(&self.document) || !same_targets {
                    self.push_log("Error: ToNURBS input changed; select the targets again".into());
                    return true;
                }
                match self.commands.execute_to_nurbs_targets_in_context(
                    &mut self.document,
                    &pending.description.command_line(),
                    &session.order,
                    viboceros_command::to_nurbs::ToNurbsContext {
                        postselected: pending.postselected,
                        control_net: session.witness.as_ref().unwrap().control_net,
                    },
                ) {
                    Ok(message) => {
                        self.object_prompt = None;
                        self.component_selection.clear();
                        self.push_log(message);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            _ => return false,
        }
        self.command_input.clear();
        true
    }
}
