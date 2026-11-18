//! Staged torus construction, source handoffs and one constructive Undo record.
use super::*;
use viboceros_command::circle_input::CircleDefinition;
use viboceros_command::torus_input::{self, Definition, Input, Kind, Preferences};
use viboceros_document::HistoryGroup;
use viboceros_geometry::{Circle3, NurbsCurve, Tolerance};
mod preview;
#[cfg(test)]
mod tests;
pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
    tolerance: Tolerance,
    fit_pending: bool,
    release_picks: Vec<ObjectId>,
    preview: Vec<NurbsCurve>,
    mesh_preview: Option<(Definition, Preferences)>,
}
impl VibocerosApp {
    pub(super) fn valid_torus(&mut self) -> bool {
        let valid = self.torus_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
        });
        if !valid {
            let name = self.torus_session.as_ref().unwrap().input.kind.name();
            self.cancel_interactive_command(false);
            self.push_log(format!(
                "{name} inputs or history changed; restart the command"
            ));
        }
        valid
    }
    pub(super) fn torus_preferences(&self, kind: Kind) -> Preferences {
        match kind {
            Kind::Solid => self.commands.torus_preferences(),
            Kind::SubD => self.commands.subd_torus_preferences(),
            Kind::Mesh => self.commands.mesh_torus_preferences(),
        }
    }

    pub(super) fn start_torus(&mut self, command: &str) -> bool {
        let words = command.split_whitespace().collect::<Vec<_>>();
        let Some(kind) = words.first().and_then(|name| Kind::from_name(name)) else {
            return false;
        };
        let mode = words.last().copied().filter(|s| {
            ["Tangent", "AroundCurve", "FitPoints"]
                .iter()
                .any(|m| s.trim_start_matches(['_', '-']).eq_ignore_ascii_case(m))
        });
        let end = words.len() - usize::from(mode.is_some());
        let mut input = Input::for_kind(
            kind,
            self.viewports[self.active_viewport].construction_plane(),
            self.torus_preferences(kind),
        );
        for word in &words[1..end] {
            match input.option(word) {
                Ok(true) => (),
                _ => return false,
            }
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group(kind.name()) {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.torus_session = Some(Session {
            input,
            group,
            tolerance: self.document.tolerance(),
            fit_pending: false,
            release_picks: vec![],
            preview: vec![],
            mesh_preview: None,
        });
        self.torus_prompt();
        if let Some(mode) = mode {
            self.torus_reference_mode(mode);
        }
        true
    }
    fn torus_prompt(&mut self) {
        let Some(s) = self.torus_session.as_mut() else {
            return;
        };
        let kind = s.input.kind;
        match kind {
            Kind::Solid => self
                .commands
                .remember_torus_preferences(s.input.preferences),
            Kind::SubD => {
                self.commands
                    .remember_subd_torus_preferences(s.input.preferences);
            }
            Kind::Mesh => (),
        }
        let snap_option = if s.input.snap_projection_plane().is_some() {
            if self.circle_project_osnap {
                "; ProjectOsnap=Yes"
            } else {
                "; ProjectOsnap=No"
            }
        } else {
            ""
        };
        let text = format!(
            "{}: {}{snap_option}; Esc cancels",
            kind.name(),
            s.input.prompt()
        );
        s.refresh(None);
        self.active_command = Some(InteractiveCommand::Torus {
            kind,
            anchor: s.input.anchor(),
        });
        self.drafting_plane = s.input.mouse_plane();
        self.command_input.clear();
        self.push_log(text);
    }
    fn advance_torus(&mut self, result: Result<(), viboceros_command::CommandError>) -> bool {
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let Some(s) = self.torus_session.as_ref() else {
            return false;
        };
        let Some(definition) = s.input.definition else {
            self.torus_prompt();
            return true;
        };
        let kind = s.input.kind;
        let geometry = match kind {
            Kind::Solid => definition
                .surface()
                .map(Geometry::NurbsSurface)
                .map_err(Into::into),
            Kind::SubD => definition.geometry(s.input.preferences),
            Kind::Mesh => definition
                .mesh(s.input.preferences, self.document.tolerance())
                .map(Geometry::Mesh)
                .map_err(Into::into),
        };
        let geometry = match geometry {
            Ok(g) => g,
            Err(e) => {
                if kind != Kind::SubD {
                    let tube_radius = self.torus_preferences(kind).tube_radius;
                    let input = &mut self.torus_session.as_mut().unwrap().input;
                    input.definition = None;
                    input.preferences.tube_radius = tube_radius;
                } else {
                    self.cancel_interactive_command(false);
                }
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.torus_session.take().unwrap();
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
                match kind {
                    Kind::Solid => {
                        self.commands
                            .remember_torus_preferences(s.input.preferences);
                    }
                    Kind::SubD => {
                        self.commands
                            .remember_subd_torus_preferences(s.input.preferences);
                    }
                    Kind::Mesh => {
                        self.commands
                            .remember_mesh_torus_preferences(s.input.preferences);
                    }
                }
                self.push_log(format!("Created {}", kind.name()));
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_torus_point(&mut self, point: Point3) -> Option<bool> {
        self.torus_session.as_ref()?;
        if !self.valid_torus() {
            return Some(false);
        }
        if self.torus_session.as_ref().unwrap().fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            return None;
        }
        let s = self.torus_session.as_mut().unwrap();
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = s.input.point(point);
        Some(self.advance_torus(result))
    }
    pub(super) fn continue_torus(&mut self, text: &str) -> bool {
        if self.torus_session.is_none() {
            return false;
        }
        if !self.valid_torus() {
            return true;
        }
        if self.torus_session.as_ref().unwrap().fit_pending
            || self.tangent_session.is_some()
            || self.circle_around_session.is_some()
        {
            return false;
        }
        if self.try_continue_circle_snap_projection(text) {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if self.torus_session.as_ref().unwrap().input.at_start()
            && ["Tangent", "AroundCurve", "FitPoints"]
                .iter()
                .any(|m| word.eq_ignore_ascii_case(m))
        {
            self.torus_reference_mode(word);
            return true;
        }
        let s = self.torus_session.as_mut().unwrap();
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if !s.input.wants_number() {
                self.cancel_interactive_command(true);
            } else {
                let result = s.input.enter();
                self.advance_torus(result);
            }
            return true;
        }
        let assigned = text
            .split_once('=')
            .or_else(|| text.split_once(char::is_whitespace))
            .and_then(|(name, value)| {
                let value = value.trim();
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
                    self.advance_torus(Ok(()));
                    return true;
                }
                self.point_filter = None;
                self.point_constraint = None;
                self.torus_prompt();
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
                    self.advance_torus(result);
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
            self.accept_torus_point(point);
            return true;
        }
        false
    }
    pub(super) fn update_torus_hover(&mut self, point: Point3) {
        if let Some(s) = self.torus_session.as_mut() {
            s.refresh(Some(point));
        }
    }
    fn torus_reference_mode(&mut self, mode: &str) {
        self.point_filter = None;
        self.point_constraint = None;
        let kind = self.torus_session.as_ref().unwrap().input.kind;
        if mode.eq_ignore_ascii_case("Tangent") {
            self.start_torus_tangent(kind);
            return;
        }
        if mode.eq_ignore_ascii_case("AroundCurve") {
            self.start_torus_around(kind);
            return;
        }
        self.torus_session.as_mut().unwrap().fit_pending = true;
        if viboceros_command::circle_fit_points::selected_points(&self.document)
            .take(3)
            .count()
            >= 3
        {
            self.accept_torus_fit(false);
        } else {
            self.document.clear_selection();
            self.active_command = None;
            self.drafting_plane = None;
            self.object_prompt = Some(object_selection::PendingObjectCommand::selecting(
                torus_input::selection_prompt(kind),
            ));
            self.command_input.clear();
            self.push_log(format!(
                "{}: select points or control points; Enter fits, Esc cancels",
                kind.fit_command()
            ));
        }
    }
    pub(super) fn accept_torus_reference(
        &mut self,
        definition: CircleDefinition,
        release: Vec<ObjectId>,
    ) -> bool {
        if !self.valid_torus() {
            return false;
        }
        self.tangent_session = None;
        self.release_circle_around_picks();
        self.circle_around_session = None;
        self.point_filter = None;
        self.point_constraint = None;
        let Some(s) = self.torus_session.as_mut() else {
            return false;
        };
        s.release_picks = release;
        let result = s.input.accept_reference(definition);
        self.advance_torus(result)
    }
    pub(super) fn torus_fit_preview(&self) -> Option<Circle3> {
        let s = self.torus_session.as_ref()?;
        if s.input.kind == Kind::SubD || !s.fit_pending {
            return None;
        }
        viboceros_command::circle_fit_points::selected_circle(&self.document)
            .ok()
            .flatten()
    }

    pub(super) fn accept_torus_fit(&mut self, postselected: bool) -> bool {
        if !self.valid_torus() {
            return false;
        }
        let result = self
            .torus_session
            .as_mut()
            .unwrap()
            .input
            .fit_points(&self.document);
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        self.object_prompt = None;
        let s = self.torus_session.as_mut().unwrap();
        s.fit_pending = false;
        if postselected {
            s.release_picks = self.document.selected_object_ids().collect();
            self.document.clear_selection();
        }
        self.advance_torus(Ok(()))
    }
}
