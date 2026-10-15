//! Two face picks preserve their original source identity until one commit.
use super::*;
use viboceros_command::coplanar_faces::CoplanarMergeSelection;
use viboceros_command::merge_coplanar_face::MergeCoplanarFaceInput;
use viboceros_document::GeometrySnapshot;
#[derive(Clone, Debug)]
pub(super) struct Prompt {
    pub(super) region: bool,
    pub(super) first: Option<(ObjectId, usize, GeometrySnapshot)>,
    tolerance: Tolerance,
    mesh_wires: Vec<[Point3; 2]>,
}
impl VibocerosApp {
    pub(super) fn start_merge_face_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("MergeCoplanarFace")
        }) {
            return false;
        }
        let Ok(parsed) = MergeCoplanarFaceInput::parse(&words.collect::<Vec<_>>()) else {
            return false;
        };
        if parsed.target.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        self.merge_face_prompt = Some(Prompt {
            region: parsed.region,
            first: None,
            mesh_wires: vec![],
            tolerance: self.document.tolerance(),
        });
        self.push_log("MergeCoplanarFace: select a face; MergeAllCoplanar merges its connected planar region; Esc cancels".into());
        true
    }
    pub(super) fn continue_merge_face_input(&mut self, input: &str) -> bool {
        if self.merge_face_prompt.is_none() {
            return false;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if input
            .trim()
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Cancel")
        {
            self.cancel_interactive_command(true);
        } else if words.is_empty() {
            self.push_log("Select a face, or MergeAllCoplanar; Esc cancels".into());
        } else if words[0]
            .trim_start_matches(['_', '-'])
            .split('=')
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("MergeAllCoplanar"))
        {
            match MergeCoplanarFaceInput::parse(&words) {
                Ok(p) if p.target.is_none() => {
                    let prompt = self.merge_face_prompt.as_mut().unwrap();
                    prompt.region = p.region;
                    if p.region && prompt.first.is_some() {
                        self.finish_merge_face_pick(None);
                    } else {
                        self.push_log(format!(
                            "MergeAllCoplanar={}",
                            if p.region { "Yes" } else { "No" }
                        ));
                    }
                }
                _ => self.push_log(
                    "Enter MergeAllCoplanar, MergeAllCoplanar=Yes or MergeAllCoplanar=No".into(),
                ),
            }
        } else if self.commands.recognizes(words[0]) {
            self.cancel_interactive_command(false);
            return false;
        } else if words.len() == 1
            && words[0].parse::<usize>().is_ok()
            && self.merge_face_prompt.as_ref().unwrap().first.is_some()
        {
            let id = self
                .merge_face_prompt
                .as_ref()
                .unwrap()
                .first
                .as_ref()
                .unwrap()
                .0;
            self.accept_merge_face_pick(id, words[0].parse().unwrap());
        } else {
            let mut seeded = vec!["MergeAllCoplanar=Yes"];
            seeded.extend_from_slice(&words);
            match MergeCoplanarFaceInput::parse(&seeded) {
                Ok(MergeCoplanarFaceInput {
                    target: Some((id, faces)),
                    ..
                }) => self.accept_merge_face_pick(id, faces[0]),
                _ => self.push_log("Pick a face or enter Object=id Face=index; Esc cancels".into()),
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn accept_merge_face_pick(&mut self, id: ObjectId, face: usize) {
        let Some(prompt) = self.merge_face_prompt.as_mut() else {
            return;
        };
        let Some(object) = self
            .document
            .object(id)
            .filter(|_| self.document.is_object_selectable(id))
        else {
            return;
        };
        let count = match object.geometry() {
            Geometry::Brep(b) => b.faces().len(),
            Geometry::Mesh(m) => m.face_count(),
            _ => return,
        };
        if face >= count {
            self.push_log("Face index is outside the source".into());
            return;
        }
        if let Some((first, _, _)) = &prompt.first {
            if *first != id {
                self.push_log("Select another face on the first object".into());
                return;
            }
            self.finish_merge_face_pick(Some(face));
        } else {
            prompt.mesh_wires = match object.geometry() {
                Geometry::Mesh(m) => crate::viewport::mesh_face_outline(m, face),
                _ => vec![],
            };
            prompt.first = Some((id, face, object.geometry_snapshot().clone()));
            prompt.tolerance = self.document.tolerance();
            if prompt.region {
                self.finish_merge_face_pick(None);
            } else {
                self.push_log("Select an adjacent coplanar face, or MergeAllCoplanar".into());
            }
        }
    }
    fn finish_merge_face_pick(&mut self, second: Option<usize>) {
        let Some(mut prompt) = self.merge_face_prompt.take() else {
            return;
        };
        let Some((id, first, snapshot)) = &prompt.first else {
            self.merge_face_prompt = Some(prompt);
            return;
        };
        let result = (|| -> Result<(usize, usize), String> {
            if self.document.tolerance() != prompt.tolerance
                || !self.document.is_object_selectable(*id)
                || !self
                    .document
                    .object(*id)
                    .is_some_and(|o| o.geometry_snapshot() == snapshot)
            {
                return Err("Face source or tolerance changed; cancel and select again".into());
            }
            let faces = if prompt.region {
                vec![*first]
            } else {
                vec![*first, second.ok_or("Select a second face")?]
            };
            CoplanarMergeSelection::prepare_selected(&self.document, *id, &faces, prompt.region)
                .and_then(|s| s.commit(&mut self.document))
                .map_err(|e| e.to_string())
        })();
        match result {
            Ok((n, _)) if n > 0 => {
                self.component_selection.clear();
                self.push_log(format!("Merged {n} coplanar face(s)"));
            }
            Ok(_) => {
                self.push_log("No adjacent coplanar faces merged; select another face".into());
                if prompt.region {
                    prompt.first = None;
                    prompt.mesh_wires.clear();
                }
                self.merge_face_prompt = Some(prompt);
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.merge_face_prompt = Some(prompt);
            }
        }
    }
    pub(super) fn merge_face_mesh_wires(&self) -> Vec<[Point3; 2]> {
        if self.merge_face_highlight().is_none() {
            return vec![];
        }
        self.merge_face_prompt
            .as_ref()
            .map_or_else(Vec::new, |p| p.mesh_wires.clone())
    }
    pub(super) fn merge_face_highlight(&self) -> Option<crate::viewport::ComponentPick> {
        let prompt = self.merge_face_prompt.as_ref()?;
        let (id, index, snapshot) = prompt.first.as_ref()?;
        let object = self.document.object(*id).filter(|o| {
            o.geometry_snapshot() == snapshot && self.document.is_object_selectable(*id)
        })?;
        Some(crate::viewport::ComponentPick {
            object: *id,
            index: *index,
            kind: match object.geometry() {
                Geometry::Mesh(_) => viboceros_command::ComponentSelectionKind::MeshFace,
                _ => viboceros_command::ComponentSelectionKind::BrepFace,
            },
        })
    }
}

#[cfg(test)]
mod tests;
