//! Component-face picking for parameter-domain queries.
use super::*;

impl VibocerosApp {
    pub(super) fn domain_needs_face_pick(&self) -> bool {
        let mut selected = self.document.selected_objects();
        let Some(object) = selected.next() else {
            return false;
        };
        selected.next().is_none()
            && matches!(object.geometry(), viboceros_document::Geometry::Brep(brep) if brep.faces().len() > 1)
    }
    pub(super) fn finish_domain_face(&mut self, point: Point3) -> bool {
        let input = format!("Domain {}", format_model_point(point));
        match self.commands.execute(&mut self.document, &input) {
            Ok(report) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(report);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
