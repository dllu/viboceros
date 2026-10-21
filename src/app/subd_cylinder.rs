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
    fit_pending: bool,
    release_picks: Vec<ObjectId>,
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
    pub(super) fn valid_subd_cylinder(&mut self) -> bool {
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
        let mode = words.last().copied().filter(|w| {
            ["Tangent", "FitPoints"]
                .iter()
                .any(|m| w.trim_start_matches(['_', '-']).eq_ignore_ascii_case(m))
        });
        let end = words.len() - usize::from(mode.is_some());
        for word in &words[1..end] {
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
            fit_pending: false,
            release_picks: vec![],
        });
        self.subd_cylinder_prompt();
        if let Some(mode) = mode {
            self.subd_cylinder_reference_mode(mode);
        }
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
                if definition.circle.radius == 0. {
                    self.cancel_interactive_command(false);
                    self.push_log(format!("Error: {e}"));
                    return false;
                }
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
            let result = self
                .document
                .release_command_selection_on_history_replay(s.release_picks.iter().copied())
                .and_then(|()| self.document.add_geometry(geometry));
            match result {
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
        if self.subd_cylinder_session.as_ref().unwrap().fit_pending
            || self.tangent_session.is_some()
        {
            return false;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if self
            .subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .at_start()
            && ["Tangent", "FitPoints"]
                .iter()
                .any(|m| word.eq_ignore_ascii_case(m))
        {
            self.subd_cylinder_reference_mode(word);
            return true;
        }
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

impl VibocerosApp {
    fn subd_cylinder_reference_mode(&mut self, mode: &str) {
        self.point_filter = None;
        self.point_constraint = None;
        if mode.eq_ignore_ascii_case("Tangent") {
            self.start_subd_cylinder_tangent();
            return;
        }
        self.subd_cylinder_session.as_mut().unwrap().fit_pending = true;
        if viboceros_command::circle_fit_points::selected_points(&self.document)
            .take(3)
            .count()
            >= 3
        {
            self.accept_subd_cylinder_fit(false);
        } else {
            self.document.clear_selection();
            self.active_command = None;
            self.drafting_plane = None;
            self.object_prompt = Some(object_selection::PendingObjectCommand::selecting(
                viboceros_command::subd_cylinder::selection_prompt(),
            ));
            self.command_input.clear();
            self.push_log("SubDCylinder FitPoints: select points or control points; Enter accepts, Esc cancels".into());
        }
    }
    pub(super) fn accept_subd_cylinder_reference(&mut self, circle: Circle3) -> bool {
        if !self.valid_subd_cylinder() {
            return false;
        }
        self.tangent_session = None;
        self.point_filter = None;
        self.point_constraint = None;
        let result = self
            .subd_cylinder_session
            .as_mut()
            .unwrap()
            .input
            .reference_circle(circle);
        self.advance_subd_cylinder(result)
    }
    pub(super) fn accept_subd_cylinder_fit(&mut self, postselected: bool) -> bool {
        if !self.valid_subd_cylinder() {
            return false;
        }
        let result = self
            .subd_cylinder_session
            .as_mut()
            .unwrap()
            .input
            .fit_points(&self.document);
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        self.object_prompt = None;
        let s = self.subd_cylinder_session.as_mut().unwrap();
        s.fit_pending = false;
        if postselected {
            s.release_picks = self.document.selected_object_ids().collect();
            self.document.clear_selection();
        }
        self.advance_subd_cylinder(Ok(()))
    }
}
