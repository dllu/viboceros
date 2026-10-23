//! Threshold face picks consume stored mesh face IDs without changing selection.
use super::*;
use crate::viewport::ComponentClick;
use viboceros_command::ComponentSelectionKind;
#[cfg(test)]
#[path = "face_sample_tests.rs"]
mod tests;

impl VibocerosApp {
    pub(in crate::app) fn picking_nonplanar_threshold(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.phase == ObjectPromptPhase::Options
                && p.mesh_conversion.as_ref().is_some_and(|s| {
                    matches!(s.options, Settings::Nonplanar(_))
                        && matches!(s.value, Some("Distance" | "Angle"))
                })
        })
    }
    pub(in crate::app) fn cancel_mesh_conversion_value(&mut self) -> bool {
        let Some(session) = self
            .object_prompt
            .as_mut()
            .and_then(|p| p.mesh_conversion.as_mut())
        else {
            return false;
        };
        if !matches!(session.options, Settings::Nonplanar(_)) || session.value.is_none() {
            return false;
        }
        session.value = None;
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
    pub(in crate::app) fn accept_mesh_conversion_face_click(
        &mut self,
        click: &ComponentClick,
    ) -> bool {
        if !self.picking_nonplanar_threshold() {
            return false;
        }
        if click.preselection {
            return true;
        }
        let pending = self.object_prompt.as_ref().unwrap();
        let session = pending.mesh_conversion.as_ref().unwrap();
        if !session.valid(&self.document) {
            self.cancel_object_prompt(false);
            self.push_log("TriangulateNonPlanarQuads inputs changed; select meshes again".into());
            return true;
        }
        let [pick] = click.picks.as_slice() else {
            self.push_log("Pick one quadrangular mesh face".into());
            return true;
        };
        if pick.kind != ComponentSelectionKind::MeshFace
            || !self.document.is_object_selectable(pick.object)
        {
            return true;
        }
        let Some(object) = self.document.object(pick.object) else {
            return true;
        };
        let Geometry::Mesh(mesh) = object.geometry() else {
            return true;
        };
        let Settings::Nonplanar(options) = session.options else {
            unreachable!()
        };
        let threshold = if session.value == Some("Angle") {
            nonplanar::SelectBy::Angle
        } else {
            nonplanar::SelectBy::Distance
        };
        match options.with_sampled_quad(mesh, pick.index, threshold) {
            Ok(options) => {
                let pending = self.object_prompt.as_mut().unwrap();
                let session = pending.mesh_conversion.as_mut().unwrap();
                session.options = Settings::Nonplanar(options);
                session.value = None;
                pending.command_override = Some(options.command());
                self.command_input.clear();
                self.log_object_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}
