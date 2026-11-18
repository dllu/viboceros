//! Staged circular solid construction and source selection.
use super::*;
use viboceros_command::{
    circle_input::CircleSizeMode,
    solid_axial::{self, Input, Kind},
};
use viboceros_document::HistoryGroup;
use viboceros_geometry::Circle3;

pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
    tolerance: Tolerance,
    guide_picks: Vec<ObjectId>,
    fit_pending: bool,
}

impl VibocerosApp {
    pub(super) fn valid_solid_axial(&mut self) -> bool {
        if let Some(session) = &self.solid_axial_session
            && (!self.document.history_group_is_current(&session.group)
                || session.tolerance != self.document.tolerance())
        {
            let name = session.input.kind.name();
            self.cancel_interactive_command(false);
            self.push_log(format!(
                "{name} inputs or history changed; restart the command"
            ));
            return false;
        }
        true
    }

    pub(super) fn start_solid_axial(&mut self, command: &str) -> bool {
        let words = command.split_whitespace().collect::<Vec<_>>();
        let Some(kind) = words.first().and_then(|name| Kind::from_name(name)) else {
            return false;
        };
        let mut input = Input::new(
            kind,
            self.viewports[self.active_viewport].construction_plane(),
            kind.initial_preferences(&self.commands),
        );
        let guide_mode = words
            .get(1..)
            .unwrap_or_default()
            .last()
            .copied()
            .filter(|mode| {
                let mode = mode.trim_start_matches(['_', '-']);
                mode.eq_ignore_ascii_case("Tangent") || mode.eq_ignore_ascii_case("FitPoints")
            });
        for &word in &words[1..words.len() - usize::from(guide_mode.is_some())] {
            if !input.option(word).is_ok_and(|accepted| accepted) {
                return false;
            }
        }
        if input.validate_mesh_layout().is_err() || guide_mode.is_some() && input.needs_guide() {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group(kind.name()) {
            Ok(group) => group,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.solid_axial_session = Some(Session {
            input,
            group,
            tolerance: self.document.tolerance(),
            guide_picks: vec![],
            fit_pending: false,
        });
        self.solid_axial_prompt();
        if let Some(mode) = guide_mode {
            self.solid_axial_reference_mode(mode);
        }
        true
    }

    fn solid_axial_prompt(&mut self) {
        let input = self.solid_axial_session.as_ref().unwrap().input;
        let kind = input.kind;
        kind.remember(&self.commands, input.preferences);
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        if input.needs_guide() {
            self.start_solid_axial_around(kind);
            return;
        }
        self.active_command = Some(InteractiveCommand::SolidAxial {
            kind,
            anchor: input.anchor(),
        });
        let snap_option = if input.snap_projection_plane().is_none() {
            ""
        } else if self.circle_project_osnap {
            "; ProjectOsnap=Yes"
        } else {
            "; ProjectOsnap=No"
        };
        self.push_log(format!("{}: {}{snap_option}", kind.name(), input.prompt()));
    }

    fn advance_solid_axial(&mut self, result: Result<(), viboceros_command::CommandError>) -> bool {
        if let Err(error) = result {
            self.command_file.fail_dispatch();
            self.push_log(format!("Error: {error}"));
            return false;
        }
        let session = self.solid_axial_session.as_ref().unwrap();
        let Some(definition) = session.input.definition else {
            self.solid_axial_prompt();
            return true;
        };
        let geometry = match definition.geometry(self.commands.use_extrusions(), session.tolerance)
        {
            Ok(geometry) => geometry,
            Err(error) => {
                let settings = definition.kind.preferences(&self.commands);
                let input = &mut self.solid_axial_session.as_mut().unwrap().input;
                input.definition = None;
                input.preferences.height = settings.height;
                input.preferences.end_size = settings.end_size;
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let mut session = self.solid_axial_session.take().unwrap();
        let result = (|| -> Result<(), DocumentError> {
            self.document.begin_group_transaction(&session.group)?;
            let result = self
                .document
                .release_command_selection_on_history_replay(session.guide_picks)
                .and_then(|()| self.document.add_geometry(geometry));
            if let Err(error) = result {
                self.document.rollback_transaction()?;
                return Err(error);
            }
            self.document.commit_group_transaction(&mut session.group)?;
            Ok(())
        })();
        self.cancel_interactive_command(false);
        match result {
            Ok(()) => {
                definition
                    .kind
                    .remember(&self.commands, session.input.preferences);
                self.push_log(format!("Created {}", definition.kind.name()));
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn accept_solid_axial_point(&mut self, point: Point3) -> Option<bool> {
        let session = self.solid_axial_session.as_ref()?;
        if session.fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            return None;
        }
        if !self.valid_solid_axial() {
            return Some(false);
        }
        let input = &mut self.solid_axial_session.as_mut().unwrap().input;
        input.set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = input.point(point);
        Some(self.advance_solid_axial(result))
    }

    pub(super) fn continue_solid_axial(&mut self, text: &str) -> bool {
        let Some(session) = &self.solid_axial_session else {
            return false;
        };
        if session.fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            return false;
        }
        if !self.valid_solid_axial() || self.try_continue_circle_snap_projection(text) {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let input = &mut self.solid_axial_session.as_mut().unwrap().input;
        if input.at_start()
            && ["Tangent", "FitPoints"]
                .iter()
                .any(|mode| word.eq_ignore_ascii_case(mode))
        {
            self.solid_axial_reference_mode(word);
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let result = input.enter();
            self.advance_solid_axial(result);
            return true;
        }
        let scalar = viboceros_geometry::LengthUnitSystem::Unset;
        let units = if input.scalar_units() {
            &scalar
        } else {
            self.document.units()
        };
        let assigned = text
            .split_once('=')
            .or_else(|| text.split_once(char::is_whitespace))
            .and_then(|(name, value)| {
                let mode = CircleSizeMode::parse(name)?;
                let units = if mode == CircleSizeMode::Area {
                    &scalar
                } else {
                    units
                };
                viboceros_drafting::PointInput::parse_length_with_units(value.trim(), units)
                    .and_then(Result::ok)
                    .map(|value| format!("{name}={value}"))
            });
        let result = match input.option(assigned.as_deref().unwrap_or(text)) {
            Ok(true) => Ok(()),
            Err(error) => Err(error),
            Ok(false) if input.wants_number() => {
                let Some(value) =
                    viboceros_drafting::PointInput::parse_length_with_units(text, units)
                else {
                    return false;
                };
                value
                    .map_err(|_| viboceros_command::CommandError::InvalidNumber(text.into()))
                    .and_then(|v| input.number(v))
            }
            Ok(false) if word == "0" => {
                let point = self.viewports[self.active_viewport]
                    .construction_plane()
                    .origin();
                return self.accept_solid_axial_point(point).unwrap_or(false);
            }
            Ok(false) => return false,
        };
        self.advance_solid_axial(result);
        true
    }

    pub(super) fn try_edit_solid_axial_guide(&mut self, text: &str, guide_selected: bool) -> bool {
        let Some(session) = &mut self.solid_axial_session else {
            return false;
        };
        let name = text
            .trim()
            .trim_start_matches(['_', '-'])
            .split(['=', ' '])
            .next()
            .unwrap_or("");
        let reference_mode = guide_selected
            && session.input.at_start()
            && ["Tangent", "FitPoints"]
                .iter()
                .any(|mode| name.eq_ignore_ascii_case(mode));
        if !guide_selected && !name.eq_ignore_ascii_case("DirectionConstraint") {
            return false;
        }
        if !reference_mode {
            match session.input.option(text) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            }
        }
        if !reference_mode && session.input.continues_base_on_guide() {
            let kind = session.input.kind;
            kind.remember(&self.commands, session.input.preferences);
            self.circle_around_prompt();
            return true;
        }
        session.guide_picks.extend(
            self.circle_around_session
                .as_ref()
                .and_then(circle_around::Session::postselected_object),
        );
        self.release_circle_around_picks();
        self.circle_around_session = None;
        self.solid_axial_prompt();
        if reference_mode {
            self.solid_axial_reference_mode(name);
        }
        true
    }

    pub(super) fn solid_axial_preview_input(&self) -> Option<Input> {
        let session = self.solid_axial_session.as_ref()?;
        if session.fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            None
        } else {
            Some(session.input)
        }
    }

    pub(super) fn observe_solid_axial_cursor(&mut self, point: Point3) {
        if let Some(session) = self.solid_axial_session.as_mut() {
            session.input.observe_cursor(point);
        }
    }

    fn solid_axial_reference_mode(&mut self, mode: &str) {
        let session = self.solid_axial_session.as_mut().unwrap();
        let kind = session.input.kind;
        if mode.eq_ignore_ascii_case("Tangent") {
            self.start_solid_axial_tangent(kind);
            return;
        }
        session.fit_pending = true;
        let has_preselection =
            viboceros_command::circle_fit_points::selected_points(&self.document)
                .take(3)
                .count()
                >= 3;
        if has_preselection && self.accept_solid_axial_fit(false) {
            return;
        }
        if !has_preselection {
            self.document.clear_selection();
        }
        self.active_command = None;
        self.object_prompt = Some(object_selection::PendingObjectCommand::selecting(
            solid_axial::selection_prompt(kind),
        ));
        self.command_input.clear();
        self.log_object_prompt();
    }

    pub(super) fn accept_solid_axial_reference(
        &mut self,
        circle: Circle3,
        release: Vec<ObjectId>,
    ) -> bool {
        if !self.valid_solid_axial() {
            return false;
        }
        self.tangent_session = None;
        self.release_circle_around_picks();
        self.circle_around_session = None;
        self.drafting_plane = None;
        let session = self.solid_axial_session.as_mut().unwrap();
        let result = session.input.reference_circle(circle, true);
        session.guide_picks.extend(release);
        self.advance_solid_axial(result)
    }

    pub(super) fn accept_solid_axial_fit(&mut self, postselected: bool) -> bool {
        if !self.valid_solid_axial() {
            return false;
        }
        let Some(session) = self.solid_axial_session.as_mut() else {
            return false;
        };
        if let Err(error) = session.input.fit_points(&self.document) {
            self.push_log(format!("Error: {error}"));
            return false;
        }
        session.fit_pending = false;
        self.object_prompt = None;
        if postselected {
            session
                .guide_picks
                .extend(self.document.selected_object_ids());
            self.document.clear_selection();
        }
        self.advance_solid_axial(Ok(()))
    }

    pub(super) fn solid_axial_fit_preview(&self) -> Option<Circle3> {
        if self.solid_axial_session.as_ref()?.fit_pending {
            viboceros_command::circle_fit_points::selected_circle(&self.document)
                .ok()
                .flatten()
        } else {
            None
        }
    }
}
