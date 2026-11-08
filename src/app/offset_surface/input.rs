//! The direction getter accepts numbers, options and an explicit distance subprompt.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn continue_offset_surface(&mut self, text: &str) -> bool {
        if self.offset_surface_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.validate_offset_surface();
        let Some(s) = &self.offset_surface_session else {
            return true;
        };
        let text = text.trim();
        let word = text.trim_start_matches('_');
        if word.eq_ignore_ascii_case("Cancel") || text == "!" {
            if !self.cancel_offset_surface_distance() {
                self.cancel_interactive_command(true);
            }
            return true;
        }
        if text.is_empty() {
            if s.through_point {
                self.log_offset_surface();
            } else if s.distance_menu {
                if s.options.distance == 0. {
                    self.continue_offset_surface("1");
                } else if s.options.distance > 0. {
                    self.cancel_offset_surface_distance();
                } else {
                    self.log_offset_surface();
                }
            } else {
                self.commit_offset_surface();
            }
            return true;
        }
        if word.eq_ignore_ascii_case("Distance") {
            let s = self.offset_surface_session.as_mut().unwrap();
            s.distance_menu = true;
            s.through_point = false;
            s.distance_start = None;
            self.log_offset_surface();
            return true;
        }
        if word.eq_ignore_ascii_case("ThroughPoint") {
            let s = self.offset_surface_session.as_mut().unwrap();
            s.through_point = true;
            s.distance_menu = false;
            s.distance_start = None;
            self.log_offset_surface();
            return true;
        }
        if s.distance_menu
            && let Ok(value) = solver::distance(text, self.document.units())
        {
            if value < 0. {
                self.push_log(
                    "Offset distance: enter a positive value or two distance points".into(),
                );
                return true;
            }
            if value == 0. {
                self.cancel_offset_surface_distance();
                return true;
            }
        }
        if s.wants_point()
            && solver::distance(text, self.document.units()).is_err()
            && (self.try_continue_point_filter(text) || self.try_continue_point_input(text))
        {
            return true;
        }
        let s = self.offset_surface_session.as_ref().unwrap();
        if self
            .commands
            .recognizes(word.split_whitespace().next().unwrap_or(""))
            && !s.distance_menu
        {
            self.cancel_interactive_command(false);
            return false;
        }
        let edit = if ["Solid", "BothSides", "DeleteInput", "Loose"]
            .iter()
            .any(|o| word.eq_ignore_ascii_case(o))
        {
            let old = if word.eq_ignore_ascii_case("Solid") {
                s.options.solid
            } else if word.eq_ignore_ascii_case("BothSides") {
                s.options.both_sides
            } else if word.eq_ignore_ascii_case("Loose") {
                s.options.loose
            } else {
                s.options.delete_input
            };
            format!("{word}={}", if old { "No" } else { "Yes" })
        } else {
            word.to_owned()
        };
        if (!s.options.solid || !s.delete_available)
            && edit
                .split_once('=')
                .is_some_and(|(k, _)| k.eq_ignore_ascii_case("DeleteInput"))
        {
            self.push_log("DeleteInput is available with Solid=Yes on an open input".into());
            return true;
        }
        let result = solver::parse(
            &edit.split_whitespace().collect::<Vec<_>>(),
            s.options,
            self.document.units(),
        );
        match result {
            Ok(input) => {
                if !s.loose_available
                    && edit
                        .split_once('=')
                        .is_some_and(|(k, _)| k.eq_ignore_ascii_case("Loose"))
                {
                    self.push_log("Loose is available when every input is a single surface without interior creases".into());
                    return true;
                }
                let options = input.options;
                if let Some(point) = input.through_point {
                    let s = self.offset_surface_session.as_mut().unwrap();
                    s.options = options;
                    s.through_point = true;
                    s.distance_menu = false;
                    s.distance_start = None;
                    self.accept_offset_surface_point(point);
                    return true;
                }
                let s = self.offset_surface_session.as_mut().unwrap();
                let geometry_changed = Options {
                    delete_input: options.delete_input,
                    ..s.options
                } != options;
                s.options = options;
                if !geometry_changed && let Some(p) = &mut s.candidate {
                    p.set_delete_input(options.delete_input);
                }
                self.cancel_offset_surface_distance();
                self.commands
                    .remember_offset_surface_options(options)
                    .expect("validated offset options");
                if geometry_changed {
                    self.update_offset_surface_preview();
                }
                self.log_offset_surface();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(in crate::app) fn cancel_offset_surface_distance(&mut self) -> bool {
        let Some(s) = &mut self.offset_surface_session else {
            return false;
        };
        if !s.distance_menu && !s.through_point {
            return false;
        }
        s.distance_menu = false;
        s.through_point = false;
        s.distance_start = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.active_command = Some(InteractiveCommand::OffsetSurface { anchor: None });
        self.log_offset_surface();
        true
    }
    pub(in crate::app) fn accept_offset_surface_point(&mut self, point: Point3) -> Option<bool> {
        if self.plane_prompt.is_some() {
            return None;
        }
        let s = self.offset_surface_session.as_mut()?;
        if !s.wants_point() {
            return Some(false);
        }
        if s.through_point {
            let distance =
                solver::through_distance(&self.document, point, s.options.flip_all, &s.flips);
            match distance {
                Ok(distance) => {
                    s.options.distance = distance;
                    s.through_point = false;
                    self.commands
                        .remember_offset_surface_options(s.options)
                        .expect("finite through-point distance");
                    self.update_offset_surface_preview();
                    self.commit_offset_surface();
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            }
            return Some(true);
        }
        if let Some(start) = s.distance_start {
            match start.distance_to(point) {
                Ok(distance) => {
                    self.continue_offset_surface(&distance.to_string());
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            }
        } else {
            s.distance_start = Some(point);
            self.active_command = Some(InteractiveCommand::OffsetSurface {
                anchor: Some(point),
            });
            self.log_offset_surface();
        }
        Some(true)
    }
}
