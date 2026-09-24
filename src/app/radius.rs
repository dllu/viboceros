//! Radius picking does not require preselection; failed picks remain editable.
use super::*;

impl VibocerosApp {
    pub(super) fn finish_radius(
        &mut self,
        point: Point3,
        diameter: bool,
        mark: bool,
        units: Option<&'static str>,
    ) -> bool {
        let name = if diameter { "Diameter" } else { "Radius" };
        let input = format!(
            "{name} Mark{name}={} {}{}",
            if mark { "Yes" } else { "No" },
            format_model_point(point),
            units
                .map(|unit| format!(" Units={unit}"))
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

    pub(super) fn try_continue_radius(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Radius {
            diameter,
            mark,
            display_units: _,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let Some((name, value)) = input.trim().trim_start_matches(['_', '-']).split_once('=')
        else {
            return false;
        };
        if !name.eq_ignore_ascii_case("Units") {
            return false;
        }
        if self.document.selected_object_count() != 0 {
            self.push_log(
                "Error: Radius/Diameter Units is unavailable with preselected objects".into(),
            );
        } else {
            match viboceros_command::distance_display_units(value) {
                Ok(display_units) => {
                    self.active_command = Some(InteractiveCommand::Radius {
                        diameter,
                        mark,
                        display_units,
                    });
                    self.push_log(format!(
                        "Radius/Diameter display units: {}",
                        display_units.unwrap_or("Model_Units")
                    ));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.command_input.clear();
        true
    }
}
