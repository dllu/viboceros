//! Crease and vertex-seam getters share component picking and atomic admission.
use super::*;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    crease::{CreaseMode, CreaseOutcome, CreaseSelection, CreaseTarget},
    vertex_seams::{VertexSeamEdit, VertexSeamMode, VertexSeamTarget},
};
#[derive(Clone, Copy, Debug)]
pub(super) enum Mode {
    Crease(CreaseMode),
    Vertices(VertexSeamMode),
}
impl Mode {
    pub(super) fn command(self) -> &'static str {
        match self {
            Self::Crease(m) => m.command(),
            Self::Vertices(m) => m.command(),
        }
    }
    fn eligible(self, kind: ComponentSelectionKind) -> bool {
        match self {
            Self::Crease(_) => eligible(kind),
            Self::Vertices(_) => kind == ComponentSelectionKind::SubDVertex,
        }
    }
    fn commit(
        self,
        doc: &mut Document,
        targets: Vec<CreaseTarget>,
    ) -> Result<CreaseOutcome, CommandError> {
        match self {
            Self::Crease(m) => CreaseSelection::prepare(doc, m, targets)?.commit(doc),
            Self::Vertices(m) => {
                let targets = targets.into_iter().filter_map(|t| match t {
                    CreaseTarget::Vertex(object, index) => Some(VertexSeamTarget { object, index }),
                    _ => None,
                });
                let out = VertexSeamEdit::prepare(doc, m, targets)?.commit(doc)?;
                Ok(CreaseOutcome {
                    message: out.message,
                    processed_objects: out.released_objects,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Prompt {
    pub mode: Mode,
    tolerance: Tolerance,
}
impl Prompt {
    pub(super) fn hint(self) -> &'static str {
        match self.mode {
            Mode::Crease(_) => {
                "Select mesh edges or SubD edges and vertices; Enter applies, Esc cancels"
            }
            Mode::Vertices(_) => "Select SubD vertices or grips; Enter applies, Esc cancels",
        }
    }
    pub(super) fn eligible(self, kind: ComponentSelectionKind) -> bool {
        self.mode.eligible(kind)
    }
    pub(super) fn preserves_grip_owners(self) -> bool {
        matches!(self.mode, Mode::Vertices(_))
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
            Mode::Crease(CreaseMode::Add)
        } else if name.eq_ignore_ascii_case("RemoveCrease") {
            Mode::Crease(CreaseMode::Remove)
        } else if ["WeldVertices", "WeldVertex", "WeldMeshVertex"]
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            Mode::Vertices(VertexSeamMode::Weld)
        } else if ["UnweldVertex", "UnweldVertices", "UnweldMeshVertex"]
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            let mut modify_normals = true;
            if let Some(option) = words.next() {
                let (key, value) = if let Some(pair) = option.split_once('=') {
                    pair
                } else if let Some(value) = words.next() {
                    (option, value)
                } else {
                    return false;
                };
                if !key
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("ModifyNormals")
                {
                    return false;
                }
                modify_normals = match value.trim_start_matches('_') {
                    s if s.eq_ignore_ascii_case("Yes") => true,
                    s if s.eq_ignore_ascii_case("No") => false,
                    _ => return false,
                };
            }
            Mode::Vertices(VertexSeamMode::Unweld { modify_normals })
        } else {
            return false;
        };
        if words.next().is_some() {
            return false;
        }
        let targets = match self.crease_targets(mode) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        // Existing object-selected point picking remains available for meshes
        // and SubDs; component-first input uses the shared multi-pick getter.
        if matches!(mode, Mode::Vertices(_))
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
            let result = if self.document.tolerance() != prompt.tolerance {
                Err(CommandError::CreaseStale)
            } else {
                self.crease_targets(prompt.mode)
                    .and_then(|t| prompt.mode.commit(&mut self.document, t))
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
