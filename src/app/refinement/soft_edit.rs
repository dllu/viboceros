//! Soft edit options and destination previews use the common curve getter.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn update_soft_edit_hover(&mut self, point: Point3) {
        let Some(p) = &mut self.refinement_prompt else {
            return;
        };
        if p.soft_distance_input {
            return;
        }
        let result = match &p.draft {
            Some(Draft::SoftEdit(d)) if d.current(&self.document) && d.anchor().is_some() => {
                d.scene(&self.document, Some(point))
            }
            Some(Draft::SoftSurface(d))
                if p.surface_distance_input.is_none()
                    && d.current(&self.document)
                    && d.anchor().is_some() =>
            {
                d.scene(&self.document, Some(point))
            }
            _ => return,
        };
        let Ok(scene) = result else { return };
        p.scene = Some(scene);
    }
    pub(super) fn continue_soft_edit_option(&mut self, text: &str) -> bool {
        let Some(p) = self
            .refinement_prompt
            .as_mut()
            .filter(|p| p.kind == Kind::SoftEdit)
        else {
            return false;
        };
        let word = text.trim_start_matches('_');
        if word.eq_ignore_ascii_case("Distance") {
            if p.anchor().is_some() {
                p.soft_distance_input = true;
                self.log_refinement();
            } else {
                self.push_log("Pick a curve base point before setting the distance".into());
            }
            self.command_input.clear();
            return true;
        }
        let mut options = p.soft_options;
        let distance = if p.soft_distance_input {
            Some(text)
        } else {
            word.split_once('=')
                .filter(|(k, _)| k.eq_ignore_ascii_case("Distance"))
                .map(|(_, v)| v)
        }
        .and_then(|s| {
            viboceros_drafting::PointInput::parse_length_with_units(s, self.document.units())
        });
        let result = if let Some(distance) = distance {
            match distance {
                Ok(value) => options.accept(&format!("Distance={value}")),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.command_input.clear();
                    return true;
                }
            }
        } else if word.eq_ignore_ascii_case("FixEnds") {
            options.fix_ends = !options.fix_ends;
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
                    Some(Draft::SoftEdit(d)) => d.set_options(options),
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => {
                        p.soft_options = options;
                        p.soft_distance_input = false;
                        self.commands.remember_soft_edit_curve_options(options);
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
