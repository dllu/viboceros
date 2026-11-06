//! Planar object and perpendicular-curve references for the shared getter.
use super::*;
impl VibocerosApp {
    pub(super) fn accept_project_object_plane(&mut self, id: ObjectId, face: Option<usize>) {
        match infinite_plane::object_plane(&self.document, id, face) {
            Ok(_) => self.add_project_plane(format!(
                "Object:{id}{}",
                face.map(|n| format!(":{n}")).unwrap_or_default()
            )),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
    pub(in crate::app) fn pick_project_plane_reference(&mut self, ids: &[ObjectId]) -> bool {
        let Some(getter) = self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.plane_getter())
        else {
            return false;
        };
        if getter.points_active() {
            return true;
        }
        if !matches!(
            getter.mode,
            PlaneMode::Choose | PlaneMode::Object | PlaneMode::AroundCurve
        ) {
            return true;
        }
        let [id] = ids else {
            self.push_log("Select one infinite-plane reference".into());
            return true;
        };
        if getter.mode != PlaneMode::AroundCurve {
            self.accept_project_object_plane(*id, None);
            return true;
        }
        let valid = self.document.is_object_selectable(*id)
            && self
                .document
                .object(*id)
                .is_some_and(|o| o.geometry().curve_ref().is_some());
        if valid {
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .plane_getter_mut()
                .unwrap()
                .curve = Some(*id);
            self.update_project_plane_marker();
            self.log_intersection_prompt();
        } else {
            self.push_log("Select a reference curve".into());
        }
        true
    }
}
