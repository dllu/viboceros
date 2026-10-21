//! Pipe command option continuation outside the application dispatcher.
use super::*;

impl VibocerosApp {
    pub(super) fn try_continue_pipe(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Pipe {
            source,
            mut cap_flat,
            mut cap_round,
            mut blend_global,
            mut fit_rail,
            mut wall_thickness,
            mut pick_second_radius,
            first_radius,
        }) = self.active_command
        else {
            return false;
        };
        let Some((name, value)) = input.trim().split_once('=') else {
            return false;
        };
        let name = name.trim_start_matches('_');
        let value = value.trim_start_matches('_');
        let recognized = if name.eq_ignore_ascii_case("Cap") {
            match value.to_ascii_lowercase().as_str() {
                "flat" => {
                    cap_flat = true;
                    cap_round = false;
                    true
                }
                "none" => {
                    cap_flat = false;
                    cap_round = false;
                    true
                }
                "round" => {
                    cap_flat = false;
                    cap_round = true;
                    true
                }
                _ => false,
            }
        } else if name.eq_ignore_ascii_case("ShapeBlending") {
            match value.to_ascii_lowercase().as_str() {
                "local" => {
                    blend_global = false;
                    true
                }
                "global" => {
                    blend_global = true;
                    true
                }
                _ => false,
            }
        } else if name.eq_ignore_ascii_case("FitRail") {
            match value.to_ascii_lowercase().as_str() {
                "yes" => {
                    fit_rail = true;
                    true
                }
                "no" => {
                    fit_rail = false;
                    true
                }
                _ => false,
            }
        } else if name.eq_ignore_ascii_case("Thick") {
            match (first_radius.is_none(), value.to_ascii_lowercase().as_str()) {
                (true, "yes") => {
                    pick_second_radius = wall_thickness.is_none();
                    true
                }
                (true, "no") => {
                    wall_thickness = None;
                    pick_second_radius = false;
                    true
                }
                _ => false,
            }
        } else if name.eq_ignore_ascii_case("WallThickness") {
            match value.parse::<f64>() {
                Ok(thickness)
                    if first_radius.is_none() && thickness.is_finite() && thickness != 0.0 =>
                {
                    wall_thickness = Some(thickness);
                    pick_second_radius = false;
                    true
                }
                _ => false,
            }
        } else {
            return false;
        };
        self.command_input.clear();
        if !recognized {
            self.push_log("Usage: Pipe Cap=None|Flat|Round ShapeBlending=Local|Global FitRail=Yes|No Thick=Yes|No WallThickness=signed-distance".to_owned());
            return true;
        }
        let command = InteractiveCommand::Pipe {
            source,
            cap_flat,
            cap_round,
            blend_global,
            fit_rail,
            wall_thickness,
            pick_second_radius,
            first_radius,
        };
        self.active_command = Some(command);
        self.push_log(command.prompt().to_owned());
        true
    }
}
