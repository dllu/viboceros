//! Display-unit choices during partial-curve length measurement.
use super::*;

pub(super) fn start_subcurve(arguments: &[&str]) -> Option<Option<&'static str>> {
    match arguments {
        [_] => Some(None),
        [_, units] => {
            let (name, value) = units.split_once('=')?;
            if !name.trim_start_matches('_').eq_ignore_ascii_case("Units") {
                return None;
            }
            viboceros_command::distance_display_units(value).ok()
        }
        _ => None,
    }
}

impl VibocerosApp {
    pub(super) fn try_continue_length(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::LengthSubCrv {
            start,
            display_units: _,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let input = input.trim().trim_start_matches(['_', '-']);
        let Some((name, value)) = input.split_once('=') else {
            return false;
        };
        if !name.eq_ignore_ascii_case("Units") {
            return false;
        }
        match viboceros_command::distance_display_units(value) {
            Ok(display_units) => {
                self.active_command = Some(InteractiveCommand::LengthSubCrv {
                    start,
                    display_units,
                });
                self.push_log(format!(
                    "Length display units: {}",
                    display_units.unwrap_or("Model_Units")
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
