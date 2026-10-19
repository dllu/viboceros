//! Surface soft-edit numeric options share the repeated point getter.
use super::*;
impl VibocerosApp {
    pub(super) fn continue_soft_surface_option(&mut self, text: &str) -> bool {
        let Some(p) = self
            .refinement_prompt
            .as_mut()
            .filter(|p| p.kind == Kind::SoftSurface)
        else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        let word = text.trim_start_matches('_');
        if let Some(axis) = ["U_Distance", "V_Distance"]
            .iter()
            .position(|s| word.eq_ignore_ascii_case(s))
        {
            p.surface_distance_input = Some(axis);
            p.surface_constraint_input = false;
            self.command_input.clear();
            self.log_refinement();
            return true;
        }
        if word.eq_ignore_ascii_case("DirConstraint") {
            p.surface_constraint_input = true;
            p.surface_distance_input = None;
            self.command_input.clear();
            self.log_refinement();
            return true;
        }
        let mut options = p.surface_options;
        let explicit = word.split_once('=').and_then(|(k, v)| {
            ["U_Distance", "V_Distance"]
                .iter()
                .position(|s| k.eq_ignore_ascii_case(s))
                .map(|axis| (axis, v))
        });
        let distance = p
            .surface_distance_input
            .map(|axis| (axis, text))
            .or(explicit);
        let result = if let Some((axis, value)) = distance {
            match viboceros_drafting::PointInput::parse_length_with_units(
                value,
                self.document.units(),
            ) {
                Some(Ok(value)) => {
                    options.accept(&format!("{}={value}", ["U_Distance", "V_Distance"][axis]))
                }
                _ => {
                    self.push_log("Enter a positive distance".into());
                    self.command_input.clear();
                    return true;
                }
            }
        } else if p.surface_constraint_input {
            options.accept(&format!("DirConstraint={word}"))
        } else if word.eq_ignore_ascii_case("FixEdges") {
            options.fix_edges = !options.fix_edges;
            Ok(true)
        } else if word.eq_ignore_ascii_case("Copy") {
            options.copy = !options.copy;
            Ok(true)
        } else {
            options.accept(text)
        };
        match result {
            Ok(false) => return false,
            Err(e) => self.push_log(format!("Error: {e}")),
            Ok(true) => {
                let result = match &mut p.draft {
                    Some(Draft::SoftSurface(d)) => d.set_options(options),
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => {
                        p.surface_options = options;
                        p.surface_distance_input = None;
                        p.surface_constraint_input = false;
                        self.commands.remember_soft_edit_surface_options(options);
                        self.log_refinement();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
        }
        self.command_input.clear();
        true
    }
}
