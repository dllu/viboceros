//! Divide's numeric/options stage and immutable viewport station preview.
use super::*;
use std::sync::Arc;
use viboceros_command::divide::{self, prompt::Prompt};
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    pub(super) prompt: Prompt,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: Tolerance,
    points: Arc<[Point3]>,
}

impl Session {
    fn new(doc: &Document, prompt: Prompt) -> Result<Self, viboceros_command::CommandError> {
        let points = divide::preview_points(doc, prompt.options)?.into();
        Ok(Self {
            prompt,
            sources: doc
                .selected_objects()
                .map(|o| (o.id(), o.geometry_snapshot().clone()))
                .collect(),
            tolerance: doc.tolerance(),
            points,
        })
    }

    fn matches_document(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc
                .selected_object_ids()
                .eq(self.sources.iter().map(|s| s.0))
            && self.sources.iter().all(|(id, snapshot)| {
                doc.object(*id)
                    .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            })
    }
}

impl VibocerosApp {
    pub(in crate::app) fn initialize_divide_prompt(&mut self) {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "Divide" && p.phase == ObjectPromptPhase::Options)
        else {
            return;
        };
        let postselected = pending.postselected;
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let curves = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if before != curves {
            if !postselected {
                self.object_prompt.as_mut().unwrap().selection_before = Some(before);
            }
            if let Err(error) = self
                .document
                .select_objects_direct(curves, SelectionMode::Replace)
            {
                self.push_log(format!("Error: {error}"));
                return;
            }
        }
        match Session::new(&self.document, Prompt::default()) {
            Ok(session) => {
                let pending = self.object_prompt.as_mut().unwrap();
                pending.command_override = Some(session.prompt.command_line());
                pending.divide = Some(session);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(in crate::app) fn continue_divide_prompt(&mut self, input: &str) -> bool {
        let Some(session) = self.object_prompt.as_ref().and_then(|p| p.divide.as_ref()) else {
            return false;
        };
        let normalized = input.trim_start_matches(['_', '-']);
        if input.is_empty() || normalized.eq_ignore_ascii_case("Enter") {
            if !session.matches_document(&self.document) {
                self.push_log("Divide sources changed; start the command again".into());
                self.cancel_object_prompt(false);
                return true;
            }
            if normalized.eq_ignore_ascii_case("Enter") {
                return self.try_continue_object_prompt("");
            }
            return false;
        }
        if normalized.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        let name = normalized.split(['=', ' ']).next().unwrap_or("");
        let local = [
            "Length",
            "EqualChordLength",
            "NumberSegments",
            "Split",
            "DeleteRemainder",
            "MarkEnds",
            "GroupOutput",
            "Preview",
        ]
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name));
        if !local && self.commands.recognizes(name) {
            return false;
        }
        let updated = if normalized.eq_ignore_ascii_case("Preview") {
            Ok(session.prompt)
        } else {
            session.prompt.updated(input)
        };
        match updated.and_then(|prompt| Session::new(&self.document, prompt)) {
            Ok(session) => {
                let pending = self.object_prompt.as_mut().unwrap();
                pending.command_override = Some(session.prompt.command_line());
                pending.divide = Some(session);
                self.log_object_prompt();
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(in crate::app) fn divide_preview_points(&self) -> Option<Arc<[Point3]>> {
        let session = self.object_prompt.as_ref()?.divide.as_ref()?;
        session
            .matches_document(&self.document)
            .then(|| Arc::clone(&session.points))
    }
}
