//! Component picks feed the same atomic ExtractPt command as scripts.
use super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_command::ComponentSelectionKind;
mod input_order;
pub(super) use input_order::InputOrder;

pub(super) fn component_kind(kind: ComponentSelectionKind) -> Option<&'static str> {
    match kind {
        ComponentSelectionKind::BrepFace
        | ComponentSelectionKind::MeshFace
        | ComponentSelectionKind::SubDFace => Some("Face"),
        ComponentSelectionKind::BrepEdge
        | ComponentSelectionKind::MeshEdge
        | ComponentSelectionKind::SubDEdge => Some("Edge"),
        ComponentSelectionKind::MeshVertex | ComponentSelectionKind::SubDVertex => Some("Vertex"),
    }
}
impl VibocerosApp {
    pub(super) fn picking_extract_points(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.description.command == "ExtractPt" && p.phase == ObjectPromptPhase::Selecting
        }) || self.intersection_prompt.as_ref().is_some_and(|p| {
            p.curve_extraction.is_some()
                && p.name() == "ExtractPt"
                && p.uv_subcurves.pending.is_none()
        })
    }
    pub(super) fn extract_point_component_arguments(&self) -> Result<String, &'static str> {
        let picks = self.component_selection.checked_picks(&self.document)?;
        let mut regions = std::collections::BTreeMap::new();
        for pick in picks
            .iter()
            .filter(|p| p.kind == ComponentSelectionKind::MeshFace)
        {
            if regions.contains_key(&pick.object) {
                continue;
            }
            if let Some(Geometry::Mesh(mesh)) =
                self.document.object(pick.object).map(|o| o.geometry())
            {
                let faces = mesh
                    .ngons()
                    .iter()
                    .enumerate()
                    .flat_map(|(i, n)| n.faces().iter().map(move |&f| (f as usize, i)))
                    .collect::<std::collections::BTreeMap<_, _>>();
                regions.insert(pick.object, (mesh, faces));
            }
        }
        let mut result = String::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut append = |object, kind, index| {
            if seen.insert((object, kind, index)) {
                result.push_str(&format!(" Component={object},{kind},{index}"));
            }
        };
        for pick in picks {
            if let Some(kind) = component_kind(pick.kind) {
                if pick.kind == ComponentSelectionKind::MeshFace
                    && let Some((mesh, faces)) = regions.get(&pick.object)
                    && let Some(&region) = faces.get(&pick.index)
                {
                    for &face in mesh.ngons()[region].faces() {
                        append(pick.object, kind, face as usize);
                    }
                } else {
                    append(pick.object, kind, pick.index);
                }
            }
        }
        Ok(result)
    }
    pub(super) fn try_start_extract_point_components(&mut self, input: &str) -> bool {
        if !input.split_whitespace().next().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtractPt")
        }) {
            return false;
        }
        match self.commands.object_selection_prompt(input) {
            Ok(Some(_)) => {}
            _ => return false,
        }
        match self.extract_point_component_arguments() {
            Ok(args) if args.is_empty() => false,
            Ok(args) => {
                self.try_execute_command(&format!("{input}{args}"));
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                true
            }
        }
    }
    pub(super) fn finish_extract_point_components(&mut self) -> bool {
        if !self.picking_extract_points() {
            return false;
        }
        let args = match self.extract_point_component_arguments() {
            Ok(args) if args.is_empty() => return false,
            Ok(args) => args,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let pending = self.object_prompt.as_ref().unwrap();
        let input = format!("{}{args}", pending.description.command_line());
        let postselected = pending.postselected;
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let result = if pending.postselected {
            self.commands
                .execute_postselected(&mut self.document, &input, context)
        } else {
            self.commands
                .execute_in_context(&mut self.document, &input, context)
        };
        match result {
            Ok(message) => {
                self.object_prompt = None;
                if postselected {
                    self.component_selection.clear();
                }
                self.push_log(format!("> {input}"));
                self.push_log(message);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn replaying_extracted_point_components(&self, input: &str) -> bool {
        let name = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        (name.eq_ignore_ascii_case("Undo") && self.document.undo_label() == Some("ExtractPt"))
            || (name.eq_ignore_ascii_case("Redo")
                && self.document.redo_label() == Some("ExtractPt"))
    }
}

#[cfg(test)]
mod edge_ranges_tests;
#[cfg(test)]
#[path = "../../crates/viboceros-command/tests/support/extract_points.rs"]
pub(super) mod native;
#[cfg(test)]
mod order_tests;
#[cfg(test)]
mod ranges_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod whole_subd_tests;
