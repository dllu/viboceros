//! Staged torus construction, source handoffs and one constructive Undo record.
use super::*;
use viboceros_command::circle_input::CircleDefinition;
use viboceros_command::subd_torus::{self as solver, Input};
use viboceros_document::HistoryGroup;
use viboceros_geometry::{Circle3, NurbsCurve, Tolerance};
#[cfg(test)]
mod tests;
pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
    tolerance: Tolerance,
    fit_pending: bool,
    release_picks: Vec<ObjectId>,
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
    fn refresh(&mut self, point: Option<Point3>) {
        self.preview.clear();
        let mut copy = self.input;
        let result = if let Some(point) = point {
            copy.point(point)
        } else {
            copy.enter()
        };
        if result.is_ok()
            && let Some(d) = copy.definition
        {
            let frame = d.circle.frame;
            let major = d.major_radius();
            self.circle(frame, major);
            self.circle(frame, major + d.tube_radius);
            self.circle(frame, (major - d.tube_radius).abs());
            if let Ok(origin) = frame.point_at([major, 0., 0.])
                && let Ok(tube) = Frame3::try_from_directions(
                    origin,
                    frame.x_axis().as_vector(),
                    frame.z_axis().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
            {
                self.circle(tube, d.tube_radius);
            }
        } else if let Some(c) = point
            .and_then(|p| self.input.preview_circle(p))
            .or(self.input.circle)
        {
            self.circle(c.frame, c.radius);
        }
    }
}
impl VibocerosApp {
    pub(super) fn valid_subd_torus(&mut self) -> bool {
        let valid = self.subd_torus_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log("SubDTorus inputs or history changed; restart the command".into());
        }
        valid
    }
    pub(super) fn start_subd_torus(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDTorus")
        }) {
            return false;
        }
        let mode = words.last().copied().filter(|s| {
            ["Tangent", "AroundCurve", "FitPoints"]
                .iter()
                .any(|m| s.trim_start_matches(['_', '-']).eq_ignore_ascii_case(m))
        });
        let end = words.len() - usize::from(mode.is_some());
        let mut input = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.subd_torus_preferences(),
        );
        for word in &words[1..end] {
            match input.option(word) {
                Ok(true) => (),
                _ => return false,
            }
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("SubDTorus") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.subd_torus_session = Some(Session {
            input,
            group,
            tolerance: self.document.tolerance(),
            fit_pending: false,
            release_picks: vec![],
            preview: vec![],
        });
        self.subd_torus_prompt();
        if let Some(mode) = mode {
            self.subd_torus_reference_mode(mode);
        }
        true
    }
    fn subd_torus_prompt(&mut self) {
        let Some(s) = self.subd_torus_session.as_mut() else {
            return;
        };
        self.commands
            .remember_subd_torus_preferences(s.input.preferences);
        let text = format!("SubDTorus: {}; Esc cancels", s.input.prompt());
        s.refresh(None);
        self.active_command = Some(InteractiveCommand::SubDTorus {
            anchor: s.input.anchor(),
        });
        self.drafting_plane = s.input.mouse_plane();
        self.command_input.clear();
        self.push_log(text);
    }
    fn advance_subd_torus(&mut self, result: Result<(), viboceros_command::CommandError>) -> bool {
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let Some(s) = self.subd_torus_session.as_ref() else {
            return false;
        };
        let Some(definition) = s.input.definition else {
            self.subd_torus_prompt();
            return true;
        };
        let geometry = match definition.geometry(s.input.preferences) {
            Ok(g) => g,
            Err(e) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.subd_torus_session.take().unwrap();
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match self
                .document
                .release_command_selection_on_history_replay(s.release_picks.iter().copied())
                .and_then(|()| self.document.add_geometry(geometry))
            {
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
                    .remember_subd_torus_preferences(s.input.preferences);
                self.push_log("Created SubDTorus".into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_subd_torus_point(&mut self, point: Point3) -> Option<bool> {
        self.subd_torus_session.as_ref()?;
        if !self.valid_subd_torus() {
            return Some(false);
        }
        if self.subd_torus_session.as_ref().unwrap().fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            return None;
        }
        let s = self.subd_torus_session.as_mut().unwrap();
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = s.input.point(point);
        Some(self.advance_subd_torus(result))
    }
    pub(super) fn continue_subd_torus(&mut self, text: &str) -> bool {
        if self.subd_torus_session.is_none() {
            return false;
        }
        if !self.valid_subd_torus() {
            return true;
        }
        if self.subd_torus_session.as_ref().unwrap().fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            return false;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if self.subd_torus_session.as_ref().unwrap().input.at_start()
            && ["Tangent", "AroundCurve", "FitPoints"]
                .iter()
                .any(|m| word.eq_ignore_ascii_case(m))
        {
            self.subd_torus_reference_mode(word);
            return true;
        }
        let s = self.subd_torus_session.as_mut().unwrap();
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if !s.input.wants_number() {
                self.cancel_interactive_command(true);
            } else {
                let result = s.input.enter();
                self.advance_subd_torus(result);
            }
            return true;
        }
        let assigned = text.split_once('=').and_then(|(name, value)| {
            let mode = viboceros_command::circle_input::CircleSizeMode::parse(
                name.trim_start_matches(['_', '-']),
            )?;
            let scalar = viboceros_geometry::LengthUnitSystem::Unset;
            let units = if mode == viboceros_command::circle_input::CircleSizeMode::Area {
                &scalar
            } else {
                self.document.units()
            };
            viboceros_drafting::PointInput::parse_length_with_units(value, units)
                .and_then(Result::ok)
                .map(|value| format!("{name}={value}"))
        });
        match s.input.option(assigned.as_deref().unwrap_or(text)) {
            Ok(true) => {
                if s.input.definition.is_some() {
                    self.advance_subd_torus(Ok(()));
                    return true;
                }
                self.point_filter = None;
                self.point_constraint = None;
                self.subd_torus_prompt();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Ok(false) => (),
        }
        if s.input.wants_number() {
            let scalar = viboceros_geometry::LengthUnitSystem::Unset;
            let units = if s.input.scalar_units() {
                &scalar
            } else {
                self.document.units()
            };
            match viboceros_drafting::PointInput::parse_length_with_units(text, units) {
                Some(Ok(value)) => {
                    let result = s.input.number(value);
                    self.advance_subd_torus(result);
                    return true;
                }
                Some(Err(e)) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                None => (),
            }
        } else if word == "0" {
            let point = self.viewports[self.active_viewport]
                .construction_plane()
                .origin();
            self.accept_subd_torus_point(point);
            return true;
        }
        false
    }
    pub(super) fn update_subd_torus_hover(&mut self, point: Point3) {
        if let Some(s) = self.subd_torus_session.as_mut() {
            s.refresh(Some(point));
        }
    }
    fn subd_torus_reference_mode(&mut self, mode: &str) {
        self.point_filter = None;
        self.point_constraint = None;
        if mode.eq_ignore_ascii_case("Tangent") {
            self.start_subd_torus_tangent();
            return;
        }
        if mode.eq_ignore_ascii_case("AroundCurve") {
            self.start_subd_torus_around();
            return;
        }
        self.subd_torus_session.as_mut().unwrap().fit_pending = true;
        if viboceros_command::circle_fit_points::selected_points(&self.document)
            .take(3)
            .count()
            >= 3
        {
            self.accept_subd_torus_fit(false);
        } else {
            self.document.clear_selection();
            self.active_command = None;
            self.drafting_plane = None;
            self.object_prompt = Some(object_selection::PendingObjectCommand::selecting(
                solver::selection_prompt(),
            ));
            self.command_input.clear();
            self.push_log(
                "SubDTorus FitPoints: select points or control points; Enter fits, Esc cancels"
                    .into(),
            );
        }
    }
    pub(super) fn accept_subd_torus_reference(
        &mut self,
        definition: CircleDefinition,
        release: Vec<ObjectId>,
    ) -> bool {
        if !self.valid_subd_torus() {
            return false;
        }
        self.tangent_session = None;
        self.release_circle_around_picks();
        self.circle_around_session = None;
        self.point_filter = None;
        self.point_constraint = None;
        let Some(s) = self.subd_torus_session.as_mut() else {
            return false;
        };
        s.release_picks = release;
        let result = s.input.accept_reference(definition);
        self.advance_subd_torus(result)
    }
    pub(super) fn accept_subd_torus_fit(&mut self, postselected: bool) -> bool {
        if !self.valid_subd_torus() {
            return false;
        }
        let result = self
            .subd_torus_session
            .as_mut()
            .unwrap()
            .input
            .fit_points(&self.document);
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        self.object_prompt = None;
        let s = self.subd_torus_session.as_mut().unwrap();
        s.fit_pending = false;
        if postselected {
            s.release_picks = self.document.selected_object_ids().collect();
            self.document.clear_selection();
        }
        self.advance_subd_torus(Ok(()))
    }
}
