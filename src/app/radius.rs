//! Radius picking does not require preselection; failed picks remain editable.
use super::*;

impl VibocerosApp {
    pub(super) fn finish_radius(&mut self, point: Point3, diameter: bool, mark: bool) -> bool {
        let name = if diameter { "Diameter" } else { "Radius" };
        let input = format!(
            "{name} Mark{name}={} {}",
            if mark { "Yes" } else { "No" },
            format_model_point(point)
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
