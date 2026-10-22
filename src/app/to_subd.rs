//! ToSubD keeps selection and option editing outside model history.
use super::object_selection::{ObjectPromptPhase, PendingObjectCommand};
use super::*;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    to_subd::{ToSubDOptions, ToSubDSelection, ToSubDTarget},
};
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Session {
    targets: Vec<ToSubDTarget>,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: viboceros_geometry::Tolerance,
}
impl Session {
    fn prepare(
        &self,
        document: &Document,
        options: ToSubDOptions,
    ) -> Result<ToSubDSelection, CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, snapshot)| {
                !document.is_object_selectable(*id)
                    || document
                        .object(*id)
                        .is_none_or(|o| o.geometry_snapshot() != snapshot)
            })
        {
            return Err(CommandError::ToSubDStale);
        }
        ToSubDSelection::prepare(document, self.targets.iter().copied(), options)
    }
}
impl VibocerosApp {
    pub(super) fn to_subd_pending(&self) -> bool {
        self.object_prompt
            .as_ref()
            .is_some_and(|p| p.description.command == "ToSubD")
    }
    pub(super) fn picking_to_subd(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.description.command == "ToSubD" && p.phase == ObjectPromptPhase::Selecting
        })
    }
    fn capture_to_subd(&self) -> Result<Session, CommandError> {
        let mut targets = self
            .document
            .selected_objects()
            .filter(|o| viboceros_command::to_subd::accepts(o.geometry()))
            .map(|o| ToSubDTarget::Whole(o.id()))
            .collect::<Vec<_>>();
        targets.extend(
            self.component_selection
                .checked_picks(&self.document)
                .map_err(|_| CommandError::ToSubDStale)?
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
                .map(|p| ToSubDTarget::Face(p.object, p.index)),
        );
        if targets.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut seen = std::collections::BTreeSet::new();
        let sources = targets
            .iter()
            .filter_map(|target| {
                let id = match *target {
                    ToSubDTarget::Whole(id) | ToSubDTarget::Face(id, _) => id,
                };
                seen.insert(id).then(|| {
                    (
                        id,
                        self.document
                            .object(id)
                            .unwrap()
                            .geometry_snapshot()
                            .clone(),
                    )
                })
            })
            .collect();
        Ok(Session {
            targets,
            sources,
            tolerance: self.document.tolerance(),
        })
    }
    pub(super) fn try_start_to_subd(&mut self, input: &str) -> bool {
        if !input.split_whitespace().next().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ToSubD")
        }) {
            return false;
        }
        let description = match self.commands.object_selection_prompt(input) {
            Ok(Some(p)) => p,
            Ok(None) => return false, // Explicit Faces=/Object= invokes the scripted command.
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.component_selection.valid_picks(&self.document);
        let session = match self.capture_to_subd() {
            Ok(s) => Some(s),
            Err(CommandError::NoObjectsSelected) => None,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let components = std::mem::take(&mut self.component_selection);
        self.cancel_interactive_command(false);
        self.component_selection = components;
        let mut pending = PendingObjectCommand::selecting(description);
        if let Some(session) = session {
            if let Err(e) = self
                .commands
                .accept_object_selection_options(&pending.description)
            {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            pending.phase = ObjectPromptPhase::Options;
            pending.postselected = false;
            pending.to_subd = Some(session);
        } else {
            self.document.clear_selection();
            self.component_selection.clear();
        }
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
    pub(super) fn finish_to_subd(&mut self) -> bool {
        if !self.to_subd_pending() {
            return false;
        }
        let mut pending = self.object_prompt.as_ref().unwrap().clone();
        match pending.phase {
            ObjectPromptPhase::Selecting => {
                match self.capture_to_subd().and_then(|session| {
                    self.commands
                        .accept_object_selection_options(&pending.description)?;
                    Ok(session)
                }) {
                    Ok(session) => {
                        pending.to_subd = Some(session);
                        pending.phase = ObjectPromptPhase::Options;
                        self.object_prompt = Some(pending);
                        self.log_object_prompt();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            ObjectPromptPhase::Options => {
                let result = ToSubDOptions::from_prompt(&pending.description).and_then(|options| {
                    pending
                        .to_subd
                        .as_ref()
                        .ok_or(CommandError::ToSubDStale)?
                        .prepare(&self.document, options)?
                        .commit(&mut self.document)
                });
                match result {
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
#[cfg(test)]
mod tests;
