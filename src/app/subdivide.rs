//! Whole-object and Ctrl+Shift face picking share the staged command operation.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    subdivide::{SubdivisionOutcome, SubdivisionSelection, SubdivisionTarget},
};

impl VibocerosApp {
    pub(super) fn picking_subdivide(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.description.command == "SubDivide"
                && p.phase == object_selection::ObjectPromptPhase::Selecting
        })
    }
    pub(super) fn try_start_subdivide_faces(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDivide")
        }) || words.next().is_some()
        {
            return false;
        }
        let picks = self.component_selection.valid_picks(&self.document);
        if !picks.iter().any(|p| {
            matches!(
                p.kind,
                ComponentSelectionKind::MeshFace | ComponentSelectionKind::SubDFace
            )
        }) {
            self.component_selection.clear();
            return false;
        }
        let prepared = self.prepare_subdivision();
        self.cancel_interactive_command(false);
        self.command_input.clear();
        match prepared.and_then(|p| p.commit(&mut self.document, false)) {
            Ok(out) => self.accept_subdivision_output(out),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    fn prepare_subdivision(&self) -> Result<SubdivisionSelection, CommandError> {
        let picks = self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| CommandError::SubdivisionStale)?;
        let mut targets = self
            .document
            .selected_objects()
            .filter(|o| matches!(o.geometry(), Geometry::Mesh(_) | Geometry::SubD(_)))
            .map(|o| SubdivisionTarget::Whole(o.id()))
            .collect::<Vec<_>>();
        let mut selected = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for p in picks {
            if matches!(
                p.kind,
                ComponentSelectionKind::MeshFace | ComponentSelectionKind::SubDFace
            ) {
                selected.entry(p.object).or_default().insert(p.index);
            }
        }
        for (id, mut faces) in selected {
            // Viewport mesh picks use logical n-gons. Expand all members once
            // per object, without rescanning the n-gon table per picked face.
            if let Some(Geometry::Mesh(mesh)) = self.document.object(id).map(|o| o.geometry()) {
                for ngon in mesh.ngons() {
                    if ngon.faces().iter().any(|f| faces.contains(&(*f as usize))) {
                        faces.extend(ngon.faces().iter().map(|&f| f as usize));
                    }
                }
            }
            targets.extend(faces.into_iter().map(|f| SubdivisionTarget::Face(id, f)));
        }
        SubdivisionSelection::prepare(&self.document, targets)
    }
    pub(super) fn finish_subdivide(&mut self, postselected: bool) -> bool {
        if !self.picking_subdivide() {
            return false;
        }
        match self
            .prepare_subdivision()
            .and_then(|p| p.commit(&mut self.document, postselected))
        {
            Ok(out) => {
                self.object_prompt = None;
                self.accept_subdivision_output(out);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
    fn accept_subdivision_output(&mut self, out: SubdivisionOutcome) {
        if let Err(error) = self
            .component_selection
            .replace_subdivision_faces(&self.document, out.selected_faces)
        {
            self.push_log(format!("Error: {error}"));
        }
        self.push_log(out.message);
    }
}

#[cfg(test)]
mod tests;
