//! Component-face picking for parameter-domain queries.
use super::*;

impl VibocerosApp {
    pub(super) fn domain_has_one_curve(&self) -> bool {
        let mut selected = self.document.selected_objects();
        let Some(object) = selected.next() else {
            return false;
        };
        selected.next().is_none() && object.geometry().curve_ref().is_some()
    }

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

    pub(super) fn accept_subcurve_measurement_point(
        &mut self,
        name: &'static str,
        start: Option<Point3>,
        point: Point3,
        display_units: Option<&'static str>,
    ) -> bool {
        let Some(start) = start else {
            let next = match name {
                "Domain" => InteractiveCommand::DomainSubCrv { start: Some(point) },
                "Length" => InteractiveCommand::LengthSubCrv {
                    start: Some(point),
                    display_units,
                },
                _ => unreachable!("only supported subcurve measurements are dispatched"),
            };
            self.active_command = Some(next);
            self.push_log(next.prompt().to_owned());
            return true;
        };
        let input = format!(
            "{name} SubCrv {} {}{}",
            format_model_point(start),
            format_model_point(point),
            display_units
                .map(|units| format!(" Units={units}"))
                .unwrap_or_default()
        );
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
