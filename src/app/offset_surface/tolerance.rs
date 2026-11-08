//! Numeric tolerance input owns viewport clicks and cancels the whole command.
use super::*;

impl VibocerosApp {
    pub(super) fn continue_offset_surface_tolerance(&mut self, text: &str) -> bool {
        let Some(s) = &self.offset_surface_session else {
            return false;
        };
        let word = text.trim_start_matches('_');
        if !s.tolerance_menu
            && word
                .split_once('=')
                .map_or(word, |p| p.0)
                .eq_ignore_ascii_case("Tolerance")
        {
            if s.options.loose && s.loose_available {
                self.push_log("Tolerance is available with Loose=No".into());
                return true;
            }
            let s = self.offset_surface_session.as_mut().unwrap();
            s.tolerance_menu = true;
            s.distance_menu = false;
            s.through_point = false;
            s.distance_start = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.active_command = Some(InteractiveCommand::OffsetSurface { anchor: None });
            self.log_offset_surface();
            return true;
        }
        if !s.tolerance_menu {
            return false;
        }
        if word.is_empty() {
            self.offset_surface_session.as_mut().unwrap().tolerance_menu = false;
            self.log_offset_surface();
            return true;
        }
        let value = if word.eq_ignore_ascii_case("Default") {
            Ok(0.)
        } else {
            solver::distance(word, self.document.units())
        };
        let value = match value {
            Ok(v) if v >= 0. => v,
            _ => {
                self.push_log("Offset tolerance: enter a nonnegative length, or Default".into());
                return true;
            }
        };
        let s = self.offset_surface_session.as_mut().unwrap();
        let old = s.options;
        s.options = Options {
            tolerance: value,
            ..s.options
        }
        .normalize_tolerance(self.document.tolerance());
        s.tolerance_menu = false;
        self.commands
            .remember_offset_surface_options(s.options)
            .expect("validated offset tolerance");
        if old != s.options {
            self.update_offset_surface_preview();
        }
        self.log_offset_surface();
        true
    }
}
