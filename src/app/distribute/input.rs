//! Command tokens and measurement subgetters for Distribute.
use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(in crate::app) fn continue_distribute(&mut self, input: &str) -> bool {
        if self.distribute_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.validate_distribute();
        let Some(session) = &mut self.distribute_session else {
            return true;
        };
        let input = input.trim();
        let word = input.trim_start_matches('_');
        if input == "!" || word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            match session.stage {
                Stage::Direction(Getter::Menu) => self.cancel_interactive_command(true),
                Stage::Mode(resume) | Stage::Spacing { resume, .. } => {
                    session.stage = Stage::Direction(resume);
                    self.log_distribute();
                }
                Stage::Direction(_) => self.log_distribute(),
            }
            self.command_input.clear();
            return true;
        }
        match session.stage {
            Stage::Spacing { resume, .. } => {
                if word.eq_ignore_ascii_case("Automatic") {
                    self.set_distribute_spacing(None, resume);
                    self.command_input.clear();
                    return true;
                }
                if let Some(distance) =
                    PointInput::parse_length_with_units(word, self.document.units())
                {
                    match distance {
                        Ok(distance) => self.set_distribute_spacing(Some(distance), resume),
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                    self.command_input.clear();
                    return true;
                }
            }
            Stage::Mode(resume) => {
                self.set_distribute_mode(word, resume);
                self.command_input.clear();
                return true;
            }
            Stage::Direction(getter) => {
                if getter == Getter::Menu {
                    if let Some(axis) = Direction::axis(word) {
                        let plane = self.viewports[self.active_viewport].construction_plane();
                        self.commit_distribute(axis, plane);
                        self.command_input.clear();
                        return true;
                    }
                    if word.eq_ignore_ascii_case("Direction") {
                        session.stage = Stage::Direction(Getter::First);
                        self.log_distribute();
                        self.command_input.clear();
                        return true;
                    }
                }
                let (name, value) = word.split_once('=').map_or((word, None), |(name, val)| {
                    (name, Some(val.trim_start_matches('_')))
                });
                if name.eq_ignore_ascii_case("Spacing") {
                    session.stage = Stage::Spacing {
                        resume: getter,
                        first: None,
                    };
                    self.log_distribute();
                    self.command_input.clear();
                    return true;
                }
                if name.eq_ignore_ascii_case("Mode") {
                    if let Some(value) = value {
                        self.set_distribute_mode(value, getter);
                    } else {
                        session.stage = Stage::Mode(getter);
                        self.log_distribute();
                    }
                    self.command_input.clear();
                    return true;
                }
            }
        }
        if self.try_run_view_command(input) {
            return true;
        }
        if self
            .distribute_session
            .as_ref()
            .is_some_and(Session::wants_point)
            && (self.try_one_shot_snap(input)
                || self.try_continue_point_filter(input)
                || self.try_continue_point_constraint(input)
                || self.try_continue_point_input(input))
        {
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_interactive_command(false);
            return false;
        }
        self.command_file.fail_dispatch();
        self.push_log(format!("Error: unexpected Distribute input {input}"));
        self.command_input.clear();
        true
    }

    fn set_distribute_mode(&mut self, value: &str, resume: Getter) {
        let session = self.distribute_session.as_mut().unwrap();
        match DistributionSettings::parse_over(&["Mode", value], session.settings) {
            Ok(settings) => {
                session.settings = settings;
                session.stage = Stage::Direction(resume);
                session.preview = None;
                self.log_distribute();
            }
            Err(_) => self.push_log("Choose Gap or Center".into()),
        }
    }
}
