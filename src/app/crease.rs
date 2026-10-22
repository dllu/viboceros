//! Crease getters collect immutable components and admit one complete edit.
use super::*;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    crease::{CreaseMode, CreaseOutcome, CreaseSelection, CreaseTarget},
};

#[derive(Clone, Copy, Debug)]
pub(super) struct Prompt {
    pub mode: CreaseMode,
    tolerance: Tolerance,
}
impl Prompt {
    pub(super) fn hint(self) -> &'static str {
        "Select mesh edges or SubD edges and vertices; Enter applies, Esc cancels"
    }
}
pub(super) fn eligible(kind: ComponentSelectionKind) -> bool {
    matches!(
        kind,
        ComponentSelectionKind::MeshEdge
            | ComponentSelectionKind::SubDEdge
            | ComponentSelectionKind::SubDVertex
    )
}
impl VibocerosApp {
    fn crease_targets(&self) -> Result<Vec<CreaseTarget>, CommandError> {
        let picks = self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| CommandError::CreaseStale)?;
        let mut targets = picks
            .into_iter()
            .filter(|p| eligible(p.kind))
            .map(|p| match p.kind {
                ComponentSelectionKind::SubDVertex => CreaseTarget::Vertex(p.object, p.index),
                _ => CreaseTarget::Edge(p.object, p.index),
            })
            .collect::<Vec<_>>();
        let subds = self
            .document
            .selectable_objects()
            .filter(|o| matches!(o.geometry(), Geometry::SubD(_)))
            .map(|o| o.id())
            .collect::<std::collections::BTreeSet<_>>();
        targets.extend(
            self.document
                .selected_control_points()
                .filter(|(g, _)| subds.contains(&g.object))
                .map(|(g, _)| CreaseTarget::Vertex(g.object, g.index)),
        );
        Ok(targets)
    }
    pub(super) fn try_start_crease(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(name) = words.next().map(|s| s.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let mode = if name.eq_ignore_ascii_case("Crease") {
            CreaseMode::Add
        } else if name.eq_ignore_ascii_case("RemoveCrease") {
            CreaseMode::Remove
        } else {
            return false;
        };
        if words.next().is_some() {
            return false;
        }
        let targets = match self.crease_targets() {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        if !targets.is_empty() {
            let prepared = CreaseSelection::prepare(&self.document, mode, targets);
            self.cancel_interactive_command(false);
            match prepared.and_then(|p| p.commit(&mut self.document)) {
                Ok(out) => self.accept_crease(out),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else {
            self.cancel_interactive_command(false);
            self.component_selection.clear();
            self.document.clear_selection();
            let prompt = Prompt {
                mode,
                tolerance: self.document.tolerance(),
            };
            self.crease_prompt = Some(prompt);
            self.push_log(format!("{}: {}", mode.command(), prompt.hint()));
        }
        self.command_input.clear();
        true
    }
    pub(super) fn try_continue_crease(&mut self, input: &str) -> bool {
        let Some(prompt) = self.crease_prompt else {
            return false;
        };
        let input = input.trim().trim_start_matches(['_', '-']);
        if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            let prepared = if self.document.tolerance() != prompt.tolerance {
                Err(CommandError::CreaseStale)
            } else {
                self.crease_targets()
                    .and_then(|t| CreaseSelection::prepare(&self.document, prompt.mode, t))
            };
            match prepared.and_then(|p| p.commit(&mut self.document)) {
                Ok(out) => {
                    self.crease_prompt = None;
                    self.accept_crease(out);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else if input.eq_ignore_ascii_case("Cancel") {
            self.cancel_crease();
        } else if input.eq_ignore_ascii_case("SelNone") {
            self.component_selection.clear();
            self.document.clear_selection();
            self.push_log("Deselected components".into());
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|n| self.commands.recognizes(n))
        {
            self.cancel_crease();
            return false;
        } else {
            self.push_log(prompt.hint().into());
        }
        self.command_input.clear();
        true
    }
    fn accept_crease(&mut self, out: CreaseOutcome) {
        self.component_selection
            .release_objects(&out.processed_objects);
        self.component_selection
            .release_grip_vertices(&self.document);
        self.push_log(out.message);
    }
    pub(super) fn cancel_crease(&mut self) {
        if self.crease_prompt.take().is_some() {
            self.component_selection.clear();
            self.document.clear_selection();
        }
    }
}

#[cfg(test)]
mod tests;
