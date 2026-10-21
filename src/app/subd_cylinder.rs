//! Staged radial input and grouped history for native subdivision cylinders.
use super::*;
use viboceros_command::subd_cylinder::Input;
use viboceros_document::HistoryGroup;
use viboceros_geometry::{Circle3, NurbsCurve, Polyline3, Tolerance};
#[cfg(test)]
mod tests;
pub(super) struct Session {
    input: Input,
    group: HistoryGroup,
    tolerance: Tolerance,
    preview: Vec<NurbsCurve>,
}
impl Session {
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }
    fn circle(&mut self, frame: Frame3, radius: f64) {
        if let Ok(curve) = Circle3::try_new(
            frame.origin(),
            radius,
            frame.z_axis(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .and_then(Circle3::to_nurbs)
        {
            self.preview.push(curve);
        }
    }
    fn refresh(&mut self, cursor: Option<Point3>) {
        self.preview.clear();
        let Some(point) = cursor else {
            if let Some(c) = self.input.circle {
                self.circle(c.frame, c.radius);
            }
            return;
        };
        let Some(c) = self.input.preview_circle(point) else {
            return;
        };
        let mut preview = self.input;
        if preview.circle.is_some()
            && preview.point(point).is_ok()
            && let Some(d) = preview.definition
        {
            self.circle(d.frame, c.radius);
            if let Ok(end) = d.frame.point_at([0., 0., d.height]) {
                self.circle(d.frame.with_origin(end), c.radius);
                if let Ok(line) =
                    Polyline3::try_new(vec![d.frame.origin(), end], Tolerance::NUMERICAL_VALIDATION)
                        .and_then(|p| p.to_nurbs())
                {
                    self.preview.push(line);
                }
            }
        } else {
            self.circle(c.frame, c.radius);
        }
    }
}
impl VibocerosApp {
    fn valid_subd_cylinder(&mut self) -> bool {
        let valid = self.subd_cylinder_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log("SubDCylinder inputs or history changed; restart the command".into());
        }
        valid
    }
    pub(super) fn start_subd_cylinder(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDCylinder")
        }) {
            return false;
        }
        let mut getter = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.subd_cylinder_preferences(),
        );
        for word in &words[1..] {
            // Complete scripts go through the command registry; partial option
            // prefixes start the same point getter as a bare command.
            match getter.option(word) {
                Ok(true) => (),
                _ => return false,
            }
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("SubDCylinder") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.subd_cylinder_session = Some(Session {
            input: getter,
            group,
            tolerance: self.document.tolerance(),
            preview: vec![],
        });
        self.subd_cylinder_prompt();
        true
    }
    fn subd_cylinder_prompt(&mut self) {
        let Some(s) = self.subd_cylinder_session.as_mut() else {
            return;
        };
        self.commands
            .remember_subd_cylinder_preferences(s.input.preferences);
        let message = format!("SubDCylinder: {}; Esc cancels", s.input.prompt());
        self.drafting_plane = s.input.mouse_plane();
        self.active_command = Some(InteractiveCommand::SubDCylinder {
            anchor: s.input.anchor(),
        });
        s.refresh(None);
        self.command_input.clear();
        self.push_log(message);
    }
    fn advance_subd_cylinder(
        &mut self,
        result: Result<(), viboceros_command::CommandError>,
    ) -> bool {
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let s = self.subd_cylinder_session.as_ref().unwrap();
        let Some(definition) = s.input.definition else {
            self.subd_cylinder_prompt();
            return true;
        };
        let geometry = match definition.geometry(s.input.preferences) {
            Ok(g) => g,
            Err(e) => {
                self.subd_cylinder_session
                    .as_mut()
                    .unwrap()
                    .input
                    .definition = None;
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.subd_cylinder_session.take().unwrap();
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match self.document.add_geometry(geometry) {
                Ok(_) => {
                    self.document.commit_group_transaction(&mut s.group)?;
                    Ok(())
                }
                Err(e) => {
                    self.document.rollback_transaction()?;
                    Err(e.into())
                }
            }
        })();
        self.active_command = None;
        self.drafting_plane = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        match result {
            Ok(()) => {
                self.commands
                    .remember_subd_cylinder_preferences(s.input.preferences);
                self.push_log("Created SubDCylinder".into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_subd_cylinder_point(&mut self, point: Point3) -> Option<bool> {
        self.subd_cylinder_session.as_ref()?;
        if !self.valid_subd_cylinder() {
            return Some(false);
        }
        let s = self.subd_cylinder_session.as_mut().unwrap();
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = s.input.point(point);
        Some(self.advance_subd_cylinder(result))
    }
    pub(super) fn continue_subd_cylinder(&mut self, input: &str) -> bool {
        if self.subd_cylinder_session.is_none() {
            return false;
        }
        if !self.valid_subd_cylinder() {
            return true;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.subd_cylinder_session.as_mut().unwrap();
        match s.input.option(word) {
            Ok(true) => {
                self.point_filter = None;
                self.point_constraint = None;
                self.subd_cylinder_prompt();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Ok(false) => (),
        }
        let result = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            Some(s.input.enter())
        } else if s.input.wants_number() {
            let scalar_units = viboceros_geometry::LengthUnitSystem::Unset;
            let units = if s.input.uses_scalar_units() {
                &scalar_units
            } else {
                self.document.units()
            };
            match viboceros_drafting::PointInput::parse_length_with_units(input, units) {
                Some(Ok(v)) => Some(s.input.number(v)),
                Some(Err(e)) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                None => None,
            }
        } else {
            None
        };
        if let Some(result) = result {
            self.advance_subd_cylinder(result);
            return true;
        }
        if word == "0" && !s.input.wants_number() {
            let point = self.viewports[self.active_viewport]
                .construction_plane()
                .origin();
            self.accept_subd_cylinder_point(point);
            return true;
        }
        false
    }
    pub(super) fn update_subd_cylinder_hover(&mut self, point: Point3) {
        if let Some(s) = self.subd_cylinder_session.as_mut() {
            s.input.observe_cursor(point);
            s.refresh(Some(point));
        }
    }
}
