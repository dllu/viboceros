//! Interactive Catenary input delegates geometry and completion to the command layer.
use super::*;
use viboceros_command::catenary_input::{Input, Mode, Slot};

pub(super) struct Session {
    pub(super) input: Input,
    pending: Option<&'static str>,
}
impl VibocerosApp {
    pub(super) fn try_start_catenary(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Catenary")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.commands.catenary_options(),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.catenary_session = Some(Session {
            input,
            pending: None,
        });
        self.catenary_prompt();
        true
    }
    fn catenary_prompt(&mut self) {
        let Some(s) = &self.catenary_session else {
            return;
        };
        let o = s.input.options();
        let prompt = match s.pending {
            Some("Mode") => "mode: ThroughPoint, Length, Parameter or Apex".to_owned(),
            Some("PointCount") => "number of output points".to_owned(),
            _ => match s.input.slot() {
                Slot::Start => "start of catenary".to_owned(),
                Slot::End => "end of catenary".to_owned(),
                Slot::Axis => {
                    "axis direction from start; Enter accepts the default arrow".to_owned()
                }
                Slot::Constraint => match o.mode {
                    Mode::ThroughPoint => {
                        if s.input.enter_cancels() {
                            "point on catenary".to_owned()
                        } else {
                            "point on catenary; Enter puts the apex at the end".to_owned()
                        }
                    }
                    Mode::Apex => "apex height".to_owned(),
                    Mode::Length | Mode::Parameter => format!(
                        "{}{}; type a value or pick its distance from start",
                        if o.mode == Mode::Length {
                            "length"
                        } else {
                            "parameter"
                        },
                        s.input
                            .scalar_default()
                            .map_or_else(String::new, |n| format!(" <{n}>"))
                    ),
                },
                Slot::Complete => "complete".to_owned(),
            },
        };
        self.active_command = Some(InteractiveCommand::Catenary {
            anchor: s.input.anchor(),
        });
        self.drafting_plane = Some(s.input.mouse_plane());
        self.push_log(format!(
            "Catenary: {prompt}; Mode={}, Output={}, PointCount={}, MarkApex={}; Esc cancels",
            o.mode.name(),
            if o.output == viboceros_geometry::CatenaryOutput::Smooth {
                "Smooth"
            } else {
                "Polyline"
            },
            o.point_count,
            if o.mark_apex { "Yes" } else { "No" }
        ));
    }
    fn finish_catenary(&mut self) {
        let Some(s) = &self.catenary_session else {
            return;
        };
        let options = s.input.options();
        match s.input.command() {
            Ok(command) => {
                if self.try_execute_command(&command) {
                    self.commands.remember_catenary_options(options);
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn try_continue_catenary(&mut self, text: &str) -> bool {
        let Some(s) = &mut self.catenary_session else {
            return false;
        };
        let word = text.trim().trim_start_matches('_');
        if word.eq_ignore_ascii_case("Cancel") || word.eq_ignore_ascii_case("-Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.pending.take().is_some() {
                self.catenary_prompt();
            } else if s.input.enter_cancels() {
                self.cancel_interactive_command(true);
            } else {
                match s.input.enter() {
                    Ok(true) => self.finish_catenary(),
                    Ok(false) => self.catenary_prompt(),
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        if s.pending == Some("PointCount")
            && self
                .commands
                .recognizes(word.split_whitespace().next().unwrap_or(""))
        {
            return false;
        }
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if s.pending == Some("Mode") {
            match s.input.option(&format!("Mode={word}")) {
                Ok(true) => {
                    s.pending = None;
                    self.catenary_prompt();
                }
                _ => {
                    if self
                        .commands
                        .recognizes(word.split_whitespace().next().unwrap_or(""))
                    {
                        return false;
                    }
                    self.push_log(
                        "Error: choose ThroughPoint, Length, Parameter or Apex".to_owned(),
                    );
                }
            }
        } else if s.pending == Some("PointCount") || key.eq_ignore_ascii_case("PointCount") {
            let value = if s.pending.is_some() {
                Some(word)
            } else {
                value
            };
            if let Some(value) = value {
                let parsed = viboceros_drafting::PointInput::parse_length_with_units(
                    value,
                    &viboceros_geometry::LengthUnitSystem::Unset,
                );
                match parsed {
                    Some(Ok(n))
                        if n.is_finite()
                            && n.fract() == 0.
                            && n >= 0.
                            && n <= viboceros_geometry::MAX_CATENARY_POINT_COUNT as f64 =>
                    {
                        match s.input.option(&format!("PointCount={}", n as usize)) {
                            Ok(_) => {
                                s.pending = None;
                                self.catenary_prompt();
                            }
                            Err(e) => self.push_log(format!("Error: {e}")),
                        }
                    }
                    _ => self.push_log(
                        "Error: PointCount requires an integer within the output limits".to_owned(),
                    ),
                }
            } else {
                s.pending = Some("PointCount");
                self.catenary_prompt();
            }
        } else if key.eq_ignore_ascii_case("Mode") && value.is_none() {
            s.pending = Some("Mode");
            self.catenary_prompt();
        } else {
            match s.input.option(word) {
                Ok(true) => self.catenary_prompt(),
                Err(e) => self.push_log(format!("Error: {e}")),
                Ok(false) => {
                    if self
                        .commands
                        .recognizes(word.split_whitespace().next().unwrap_or(""))
                    {
                        return false;
                    }
                    let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                        word,
                        self.document.units(),
                    ) else {
                        return false;
                    };
                    match value {
                        Ok(n) => match s.input.number(n) {
                            Ok(true) => self.finish_catenary(),
                            Ok(false) => self.catenary_prompt(),
                            Err(e) => self.push_log(format!("Error: {e}")),
                        },
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                }
            }
        }
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        true
    }
    pub(super) fn accept_catenary_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.catenary_session.as_mut()?;
        if s.pending.is_some() {
            self.catenary_prompt();
            return Some(false);
        }
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(true) => {
                self.finish_catenary();
                true
            }
            Ok(false) => {
                self.catenary_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
}
