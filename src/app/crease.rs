//! Crease and seam getters share component picking and atomic admission.
use super::*;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    component_seams::{ComponentSeamEdit, EdgeSeamMode, SeamTarget},
    crease::{CreaseMode, CreaseSelection, CreaseTarget},
    vertex_seams::VertexSeamMode,
};
mod mode;
use mode::{Mode, Outcome};

pub(super) enum Prompt {
    Components { mode: Mode, tolerance: Tolerance },
    Spin(Box<super::subd_spin_edge::Prompt>),
    Insert(Box<super::insert_point::Prompt>),
}
impl Prompt {
    pub(super) fn command(&self) -> &'static str {
        match self {
            Self::Components { mode, .. } => mode.command(),
            Self::Spin(_) => "SubDSpinEdge",
            Self::Insert(_) => "InsertPoint",
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        match self {
            Self::Components {
                mode: Mode::Crease(_),
                ..
            } => "Select mesh edges or SubD edges and vertices; Enter applies, Esc cancels",
            Self::Components {
                mode: Mode::Vertices(_),
                ..
            } => "Select SubD vertices or grips; Enter applies, Esc cancels",
            Self::Components {
                mode: Mode::Edges(_),
                ..
            } => "Select a mesh topology edge or SubD edge; Enter applies, Esc cancels",
            Self::Spin(_) => {
                "Pick a SubD edge to spin; Enter finishes (Direction SpinEdgeAgain Undo)"
            }
            Self::Insert(p) => {
                if p.session.is_none() {
                    "Select one mesh or SubD"
                } else {
                    "Pick points on original control edges (Undo); Enter finishes, Esc cancels"
                }
            }
        }
    }
    pub(super) fn eligible(&self, kind: ComponentSelectionKind) -> bool {
        match self {
            Self::Components { mode, .. } => mode.eligible(kind),
            Self::Spin(_) => kind == ComponentSelectionKind::SubDEdge,
            Self::Insert(_) => false,
        }
    }
    pub(super) fn preserves_grip_owners(&self) -> bool {
        matches!(
            self,
            Self::Components {
                mode: Mode::Vertices(_),
                ..
            }
        )
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        match self {
            Self::Spin(p) => Some(&p.scene),
            Self::Insert(p) => Some(&p.scene),
            _ => None,
        }
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
    fn crease_targets(&self, mode: Mode) -> Result<Vec<CreaseTarget>, CommandError> {
        let picks = self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| CommandError::CreaseStale)?;
        let mut targets = picks
            .into_iter()
            .filter(|p| mode.eligible(p.kind))
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
        if mode.eligible(ComponentSelectionKind::SubDVertex) {
            targets.extend(
                self.document
                    .selected_control_points()
                    .filter(|(g, _)| subds.contains(&g.object))
                    .map(|(g, _)| CreaseTarget::Vertex(g.object, g.index)),
            );
        }
        Ok(targets)
    }
    pub(super) fn try_start_crease(&mut self, input: &str) -> bool {
        if self.try_start_insert_point(input) {
            return true;
        }
        if self.try_start_spin_edge(input) {
            return true;
        }
        let mut words = input.split_whitespace();
        let Some(name) = words.next().map(|s| s.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let Some(mode) = Mode::parse(name, &mut words) else {
            return false;
        };
        let targets = match self.crease_targets(mode) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        // Existing object-selected point picking remains available for meshes
        // and SubDs; component-first input uses the shared multi-pick getter.
        if matches!(mode, Mode::Vertices(_) | Mode::Edges(_))
            && targets.is_empty()
            && self.document.selected_object_count() != 0
            && !self.document.selectable_objects().any(|o| {
                matches!(o.geometry(), Geometry::SubD(_))
                    && self.document.grip_mode(o.id()).is_some()
            })
        {
            return false;
        }
        if !targets.is_empty() {
            self.cancel_interactive_command(false);
            match mode.commit(&mut self.document, targets) {
                Ok(out) => self.accept_crease(out),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else {
            self.cancel_interactive_command(false);
            self.component_selection.clear();
            if matches!(mode, Mode::Crease(_)) {
                self.document.clear_selection();
            }
            let prompt = Prompt::Components {
                mode,
                tolerance: self.document.tolerance(),
            };
            self.push_log(format!("{}: {}", mode.command(), prompt.hint()));
            self.crease_prompt = Some(prompt);
        }
        self.command_input.clear();
        true
    }
    pub(super) fn try_continue_crease(&mut self, input: &str) -> bool {
        if self.insert_prompt().is_some() {
            return self.continue_insert_point(input);
        }
        if self.picking_spin_edge() {
            return self.continue_spin_edge(input);
        }
        let Some(Prompt::Components { mode, tolerance }) = self.crease_prompt.as_ref() else {
            return false;
        };
        let (mode, tolerance) = (*mode, *tolerance);
        let input = input.trim().trim_start_matches(['_', '-']);
        if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            let result = if self.document.tolerance() != tolerance {
                Err(CommandError::CreaseStale)
            } else {
                self.crease_targets(mode)
                    .and_then(|t| mode.commit(&mut self.document, t))
            };
            match result {
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
            self.push_log(self.crease_prompt.as_ref().unwrap().hint().into());
        }
        self.command_input.clear();
        true
    }
    fn accept_crease(&mut self, out: Outcome) {
        self.component_selection
            .release_objects(&out.processed_objects);
        self.component_selection
            .release_grip_vertices(&self.document);
        if let Err(e) = self
            .component_selection
            .retain_subd_edges(&self.document, out.retained_edges)
        {
            self.push_log(format!("Error: {e}"));
        }
        self.push_log(out.message);
    }
    pub(super) fn cancel_crease(&mut self) {
        if self.insert_prompt().is_some() {
            self.cancel_insert_point();
            return;
        }
        if self.picking_spin_edge() {
            self.finish_spin_edge();
            return;
        }
        if self.crease_prompt.take().is_some() {
            self.component_selection.clear();
            self.document.clear_selection();
        }
    }
}

#[cfg(test)]
mod tests;
