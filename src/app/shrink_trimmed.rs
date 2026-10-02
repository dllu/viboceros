//! Face preselection and modifier picks reuse the whole-object shrink prompt.
use super::*;
use viboceros_command::{ComponentSelectionKind, ShrinkTrimmedSelection};
use viboceros_geometry::BrepSurfaceShrinkMode;

impl VibocerosApp {
    pub(super) fn shrink_prompt_mode(&self) -> Option<BrepSurfaceShrinkMode> {
        let prompt = self.object_prompt.as_ref()?;
        if prompt.phase != object_selection::ObjectPromptPhase::Selecting {
            return None;
        }
        match prompt.description.command {
            "ShrinkTrimmedSrf" => Some(BrepSurfaceShrinkMode::Standard),
            "ShrinkTrimmedSrfToEdge" => Some(BrepSurfaceShrinkMode::ToEdge),
            _ => None,
        }
    }

    pub(super) fn try_start_shrink_faces(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(name) = words.next() else {
            return false;
        };
        if words.next().is_some() {
            return false;
        }
        let name = name.trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("ShrinkTrimmedSrfToEdge") {
            // Native ignores face preselection for this command.
            self.component_selection.clear();
            return false;
        }
        if !name.eq_ignore_ascii_case("ShrinkTrimmedSrf") {
            return false;
        }
        let faces = self
            .component_selection
            .valid_picks(&self.document)
            .into_iter()
            .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
            .map(|p| (p.object, p.index))
            .collect::<Vec<_>>();
        if faces.is_empty() {
            return false;
        }
        let prepared =
            ShrinkTrimmedSelection::prepare(&self.document, BrepSurfaceShrinkMode::Standard, faces);
        self.cancel_interactive_command(false);
        self.component_selection.clear();
        self.command_input.clear();
        match prepared.and_then(|p| p.commit(&mut self.document, false)) {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn finish_shrink_faces(&mut self, postselected: bool) -> bool {
        if self.shrink_prompt_mode() != Some(BrepSurfaceShrinkMode::Standard) {
            return false;
        }
        let picks = match self.component_selection.checked_picks(&self.document) {
            Ok(picks) => picks,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        let faces = picks
            .into_iter()
            .filter(|p| p.kind == ComponentSelectionKind::BrepFace)
            .map(|p| (p.object, p.index))
            .collect::<Vec<_>>();
        if faces.is_empty() {
            return false;
        }
        match ShrinkTrimmedSelection::prepare(
            &self.document,
            BrepSurfaceShrinkMode::Standard,
            faces,
        )
        .and_then(|p| p.commit(&mut self.document, postselected))
        {
            Ok(message) => {
                self.object_prompt = None;
                self.component_selection.clear();
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
