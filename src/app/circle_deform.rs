//! Deformable Circle uses the ordinary construction getter and captured plane.
use super::*;
use viboceros_command::circle_deform_input::Input;

pub(super) struct Session {
    pub(super) input: Input,
    plane: Frame3,
    tokens: Vec<String>,
}
impl VibocerosApp {
    pub(super) fn try_start_circle_deform(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if !matches!(words.as_slice(),[command,rest @ ..] if rest.first().is_some_and(|w| w.trim_start_matches(['_','-']).eq_ignore_ascii_case("Deformable")) && ["Circle","C"].iter().any(|n|command.trim_start_matches(['_','-']).eq_ignore_ascii_case(n)))
        {
            return false;
        }
        let plane = self.viewports[self.active_viewport].construction_plane();
        let mut input = Input::new(
            plane,
            self.commands.circle_preferences(),
            self.document.tolerance(),
        );
        let mut tokens = Vec::new();
        // Complete invocations are executed by the command backend. Starting
        // an interactive getter accepts only its initial output settings.
        for word in &words[1..] {
            let key = word
                .trim_start_matches(['_', '-'])
                .split('=')
                .next()
                .unwrap_or("");
            if !input.option(word).unwrap_or(false) {
                return false;
            }
            if !["Deformable", "Degree", "PointCount"]
                .iter()
                .any(|k| key.eq_ignore_ascii_case(k))
            {
                tokens.push((*word).to_owned());
            }
        }
        self.cancel_interactive_command(false);
        self.circle_deform_session = Some(Session {
            input,
            plane,
            tokens,
        });
        self.circle_deform_prompt();
        true
    }
    pub(super) fn start_circle_deform(&mut self) {
        self.try_start_circle_deform("Circle Deformable");
    }
    fn circle_deform_prompt(&mut self) {
        let Some(s) = &self.circle_deform_session else {
            return;
        };
        let getter = s.input.getter();
        let prompt = s.input.prompt();
        self.active_command = Some(InteractiveCommand::CircleDeform {
            anchor: getter.anchor(),
        });
        self.drafting_plane = getter.mouse_plane().or(Some(s.plane));
        self.push_log(format!("Circle Deformable: {prompt}; Esc cancels"));
    }
    fn finish_circle_deform(&mut self) -> bool {
        let s = self.circle_deform_session.as_ref().unwrap();
        let command = format!(
            "{} {}",
            s.input.output().command_prefix(),
            s.tokens.join(" ")
        );
        let plane = s.plane;
        self.try_execute_command_with_plane(&command, Some(plane))
    }
    pub(super) fn accept_circle_deform_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.circle_deform_session.as_mut()?;
        Some(match s.input.point(point) {
            Ok(result) => {
                let [x, y, z] = point.to_array();
                s.tokens.push(format!("{x},{y},{z}"));
                if result.is_some() {
                    self.finish_circle_deform()
                } else {
                    self.circle_deform_prompt();
                    true
                }
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
    pub(super) fn try_continue_circle_deform(&mut self, text: &str) -> bool {
        let Some(s) = self.circle_deform_session.as_mut() else {
            return false;
        };
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.input.accepts_default() {
                let setting = s.input.integer_pending();
                match s.input.default_value() {
                    Ok(result) => {
                        if !setting {
                            s.tokens.push("Enter".into());
                        }
                        if result.is_some() {
                            self.finish_circle_deform();
                        } else {
                            self.circle_deform_prompt();
                        }
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        if s.input.at_start() && word.eq_ignore_ascii_case("AroundCurve") {
            let output = s.input.output();
            self.start_circle_around_with_output(Some(output));
            self.command_input.clear();
            return true;
        }
        if s.input.at_start() && word.eq_ignore_ascii_case("FitPoints") {
            let command = format!("{} FitPoints", s.input.output().command_prefix());
            self.cancel_interactive_command(false);
            if !self.try_start_object_prompt(&command) {
                self.execute_command(&command);
            }
            self.command_input.clear();
            return true;
        }
        let key = word.split('=').next().unwrap_or("");
        let setting = s.input.integer_pending()
            || ["Deformable", "Degree", "PointCount"]
                .iter()
                .any(|k| key.eq_ignore_ascii_case(k));
        match s.input.option(word) {
            Ok(true) => {
                if !setting {
                    s.tokens.push(word.to_owned());
                }
                self.point_filter = None;
                self.point_constraint = None;
                self.circle_deform_prompt();
                self.command_input.clear();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_input.clear();
                return true;
            }
            Ok(false) => {}
        }
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let s = self.circle_deform_session.as_mut().unwrap();
        if let Some(value) = value
            && viboceros_command::circle_input::CircleSizeMode::parse(name).is_some()
        {
            if !s.input.option(name).unwrap_or(false) {
                return false;
            }
            s.tokens.push(name.to_owned());
            return self.circle_deform_number(value);
        }
        if !s.input.integer_pending()
            && self
                .commands
                .recognizes(word.split_whitespace().next().unwrap_or(""))
        {
            return false;
        }
        self.circle_deform_number(text)
    }
    fn circle_deform_number(&mut self, text: &str) -> bool {
        let s = self.circle_deform_session.as_mut().unwrap();
        let setting = s.input.integer_pending();
        if !setting && !s.input.getter().requests_size() {
            return false;
        }
        let scalar_units = viboceros_geometry::LengthUnitSystem::Unset;
        let units = if s.input.integer_pending()
            || s.input.getter().size_mode() == viboceros_command::circle_input::CircleSizeMode::Area
        {
            &scalar_units
        } else {
            self.document.units()
        };
        let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(text, units)
        else {
            return false;
        };
        match value
            .map_err(|e| e.to_string())
            .and_then(|v| s.input.number(v).map(|d| (v, d)).map_err(|e| e.to_string()))
        {
            Ok((value, result)) => {
                // Output settings are emitted once in the canonical prefix.
                if !setting {
                    s.tokens.push(value.to_string());
                }
                if result.is_some() {
                    self.finish_circle_deform();
                } else {
                    self.circle_deform_prompt();
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
}
