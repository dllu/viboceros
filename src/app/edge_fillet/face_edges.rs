//! Face picking expands to the edge set owned by the parent blend command.
use super::*;
use crate::viewport::ComponentPick;
use std::collections::BTreeMap;
use viboceros_command::component_selection::{current, stage_sources};
use viboceros_command::edge_blend_face_edges;
use viboceros_document::GeometrySnapshot;

pub(in crate::app) struct FaceEdgeSelection {
    prior: Vec<(ComponentPick, GeometrySnapshot)>,
}

impl VibocerosApp {
    pub(super) fn begin_edge_blend_faces(&mut self) {
        self.component_selection.clear_choices();
        self.command_input.clear();
        if self.edge_blend_faces.is_none() {
            let prior = self
                .component_selection
                .checked_picks(&self.document)
                .and_then(|picks| stage_sources(&self.document, picks));
            match prior {
                Ok(prior) => self.edge_blend_faces = Some(FaceEdgeSelection { prior }),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return;
                }
            }
        }
        self.push_log(FACE_EDGES_HINT.into());
    }

    pub(in crate::app) fn picking_edge_blend_faces(&self) -> bool {
        self.edge_blend_faces.is_some()
            && self.nested_measurement.is_none()
            && self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
    }

    pub(in crate::app) fn finish_edge_blend_faces(&mut self, accept: bool) -> bool {
        if !self.picking_edge_blend_faces() {
            return false;
        }
        let selection = self.edge_blend_faces.take().unwrap();
        if !accept {
            let prior = selection
                .prior
                .into_iter()
                .filter_map(|(pick, snapshot)| {
                    current(&self.document, pick, &snapshot).then_some(pick)
                })
                .collect();
            if let Err(error) = self.component_selection.replace(&self.document, prior) {
                self.push_log(format!("Error: {error}"));
            }
        }
        self.component_selection.clear_choices();
        self.command_input.clear();
        self.log_edge_blend_prompt();
        true
    }

    pub(in crate::app) fn edge_blend_face_picks(
        &self,
        picks: Vec<ComponentPick>,
    ) -> Result<Vec<ComponentPick>, String> {
        let mut faces = BTreeMap::<ObjectId, Vec<usize>>::new();
        for pick in picks {
            if pick.kind != ComponentSelectionKind::BrepFace
                || !self.document.is_object_selectable(pick.object)
            {
                return Err("Pick faces on an available closed polysurface".into());
            }
            faces.entry(pick.object).or_default().push(pick.index);
        }
        let tolerance = self.document.tolerance();
        let mut result = Vec::new();
        for (object, indices) in faces {
            let Some(Geometry::Brep(brep)) =
                self.document.object(object).map(|object| object.geometry())
            else {
                return Err("Pick faces on a closed polysurface".into());
            };
            let edges = edge_blend_face_edges(brep, indices, tolerance)
                .map_err(|error| error.to_string())?;
            result.extend(edges.into_iter().map(|index| ComponentPick {
                object,
                kind: ComponentSelectionKind::BrepEdge,
                index,
            }));
        }
        Ok(result)
    }
}
