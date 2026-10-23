//! One object getter, original control-edge point picks, and cancel-safe previews.
use super::*;
use viboceros_command::insert_point::InsertSession;
use viboceros_geometry::{PolygonEdgePoint, UnitVector3};
pub(super) struct Prompt {
    pub session: Option<InsertSession>,
    pub scene: Document,
}
impl Prompt {
    fn refresh(&mut self, document: &Document) -> Result<(), viboceros_command::CommandError> {
        let mut scene = document.clone();
        if let Some(session) = &self.session {
            session.apply_preview(&mut scene)?;
        }
        self.scene = scene;
        Ok(())
    }
}
impl VibocerosApp {
    pub(super) fn insert_prompt(&self) -> Option<&Prompt> {
        match self.crease_prompt.as_ref()? {
            crease::Prompt::Insert(p) => Some(p),
            _ => None,
        }
    }
    pub(super) fn selecting_insert_source(&self) -> bool {
        self.insert_prompt().is_some_and(|p| p.session.is_none())
    }
    pub(super) fn try_start_insert_point(&mut self, input: &str) -> bool {
        if !input
            .trim()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("InsertPoint")
        {
            return false;
        }
        let ids = self.document.selected_object_ids().collect::<Vec<_>>();
        let session = if let [id] = ids.as_slice() {
            InsertSession::new(&self.document, *id).ok()
        } else {
            None
        };
        self.cancel_interactive_command(false);
        self.component_selection.clear();
        self.document.clear_selection();
        self.crease_prompt = Some(crease::Prompt::Insert(Box::new(Prompt {
            session,
            scene: self.document.clone(),
        })));
        self.push_log(self.crease_prompt.as_ref().unwrap().hint().into());
        self.command_input.clear();
        true
    }
    pub(super) fn select_insert_source(&mut self, id: ObjectId) {
        let result = InsertSession::new(&self.document, id);
        match result {
            Ok(session) => {
                self.document.clear_selection();
                if let Some(crease::Prompt::Insert(p)) = &mut self.crease_prompt {
                    p.session = Some(session);
                    p.scene = self.document.clone();
                }
                self.push_log(
                    "InsertPoint: pick points on the control edges (Undo); Enter finishes".into(),
                );
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn pick_insert_point(&mut self, pick: PolygonEdgePoint, normal: UnitVector3) {
        let Some(crease::Prompt::Insert(mut prompt)) = self.crease_prompt.take() else {
            return;
        };
        let result = if let Some(session) = &mut prompt.session {
            session
                .insert(&self.document, pick, Some(normal))
                .and_then(|_| prompt.refresh(&self.document))
        } else {
            Err(viboceros_command::CommandError::InsertPointUnavailable)
        };
        match result {
            Ok(()) => self.crease_prompt = Some(crease::Prompt::Insert(prompt)),
            Err(e) => {
                let stale = matches!(e, viboceros_command::CommandError::InsertPointStale);
                self.push_log(format!("Error: {e}"));
                if !stale {
                    self.crease_prompt = Some(crease::Prompt::Insert(prompt));
                }
            }
        }
    }
    pub(super) fn continue_insert_point(&mut self, input: &str) -> bool {
        // Normalize complete control words while retaining coordinate signs.
        let input = input.trim();
        let keyword = input.trim_start_matches(['_', '-']);
        let token = if ["Undo", "Enter", "Cancel"]
            .iter()
            .any(|word| keyword.eq_ignore_ascii_case(word))
        {
            keyword
        } else {
            input.trim_start_matches('_')
        };
        if token.eq_ignore_ascii_case("Cancel") {
            self.cancel_insert_point();
            self.command_input.clear();
            return true;
        }
        if token.is_empty() || token.eq_ignore_ascii_case("Enter") {
            if self.selecting_insert_source() {
                self.push_log("Select one mesh or SubD".into());
            } else if let Some(crease::Prompt::Insert(p)) = self.crease_prompt.take()
                && let Some(s) = p.session
            {
                match s.finish(&mut self.document) {
                    Ok(message) => self.push_log(message),
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        let name = token.split_whitespace().next().unwrap_or("");
        if !name.eq_ignore_ascii_case("Undo") && self.commands.recognizes(name) {
            self.cancel_insert_point();
            return false;
        }
        let Some(crease::Prompt::Insert(mut prompt)) = self.crease_prompt.take() else {
            return false;
        };
        let mut finish = false;
        let mut cancel = false;
        let result = (|| {
            let mut trial = prompt
                .session
                .as_ref()
                .ok_or(viboceros_command::CommandError::InsertPointUnavailable)?
                .clone();
            let actions = viboceros_command::insert_point::parse_actions(
                &token.split_whitespace().collect::<Vec<_>>(),
            )?;
            for action in actions {
                use viboceros_command::insert_point::InsertAction;
                match action {
                    InsertAction::Undo => {
                        trial.undo_point(&self.document)?;
                    }
                    InsertAction::Point(point) => {
                        let pick = trial.pick_point(point)?;
                        trial.insert(&self.document, pick, None)?;
                    }
                    InsertAction::Edge(pick) => trial.insert(&self.document, pick, None)?,
                    InsertAction::Finish => finish = true,
                    InsertAction::Cancel => cancel = true,
                }
            }
            if !cancel {
                prompt.session = Some(trial);
                prompt.refresh(&self.document)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) if cancel => {
                self.document.clear_selection();
            }
            Ok(()) if finish => {
                if let Some(session) = prompt.session {
                    match session.finish(&mut self.document) {
                        Ok(message) => self.push_log(message),
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                }
            }
            Ok(()) => self.crease_prompt = Some(crease::Prompt::Insert(prompt)),
            Err(e) => {
                let stale = matches!(e, viboceros_command::CommandError::InsertPointStale);
                self.push_log(format!("Error: {e}"));
                if !stale {
                    self.crease_prompt = Some(crease::Prompt::Insert(prompt));
                }
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn cancel_insert_point(&mut self) {
        if matches!(self.crease_prompt, Some(crease::Prompt::Insert(_))) {
            self.crease_prompt = None;
            self.document.clear_selection();
            self.component_selection.clear();
        }
    }
}
