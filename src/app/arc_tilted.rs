//! Tilted arc input owns no temporary document objects.
use super::*;
use viboceros_command::arc_tilted_input::Input;

pub(super) struct Session {
    pub(super) input: Input,
    plane: Frame3,
    tokens: Vec<String>,
}
impl VibocerosApp {
    pub(super) fn start_arc_tilted(&mut self, center: Point3) {
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        self.cancel_interactive_command(false);
        self.last_point = Some(center);
        self.arc_tilted_session = Some(Session {
            input: Input::new(center, self.document.tolerance()),
            plane,
            tokens: vec![
                "Arc".into(),
                "Center".into(),
                format_model_point(center),
                "Tilted".into(),
            ],
        });
        self.arc_tilted_prompt();
    }
    pub(super) fn try_start_arc_tilted(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        let [command, rest @ ..] = words.as_slice() else {
            return false;
        };
        if !["Arc", "A"].iter().any(|n| {
            command
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case(n)
        }) {
            return false;
        }
        let rest = if rest.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Center")
        }) {
            &rest[1..]
        } else {
            rest
        };
        let [point, mode] = rest else {
            return false;
        };
        if !mode
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Tilted")
        {
            return false;
        }
        let Some(Ok(input)) = viboceros_drafting::PointInput::parse(point) else {
            return false;
        };
        let Ok(center) = input.resolve(
            self.viewports[self.active_viewport].construction_plane(),
            self.last_point,
        ) else {
            return false;
        };
        self.start_arc_tilted(center);
        true
    }
    fn arc_tilted_prompt(&mut self) {
        let Some(s) = &self.arc_tilted_session else {
            return;
        };
        self.active_command = Some(InteractiveCommand::ArcTilted {
            anchor: Some(s.input.anchor()),
        });
        // Orientation and distance measurements can use another viewport or
        // reference geometry. Endpoint picking uses the completed arc plane.
        self.drafting_plane = if s.input.requests_orientation() || s.input.requests_length() {
            None
        } else {
            s.input.frame().or(Some(s.plane))
        };
        self.push_log(format!("Arc Tilted: {}; Esc cancels", s.input.prompt()));
    }
    fn finish_arc_tilted(&mut self, tail: &str) -> bool {
        let s = self.arc_tilted_session.as_ref().unwrap();
        let command = format!("{} {tail}", s.tokens.join(" "));
        self.finish_arc_command(&command, Some(s.plane))
    }
    pub(super) fn accept_arc_tilted_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.arc_tilted_session.as_mut()?;
        let token = format_model_point(point);
        Some(match s.input.point(point) {
            Ok(Some(_)) => {
                let tail = if s.input.requests_length() {
                    token
                } else {
                    format!(
                        "Direction={} {token}",
                        if s.input.clockwise() {
                            "Clockwise"
                        } else {
                            "Counterclockwise"
                        }
                    )
                };
                self.finish_arc_tilted(&tail)
            }
            Ok(None) => {
                s.tokens.push(token);
                self.arc_tilted_prompt();
                true
            }
            Err(e) => {
                self.command_input.clear();
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
    pub(super) fn try_continue_arc_tilted(&mut self, text: &str) -> bool {
        let Some(s) = self.arc_tilted_session.as_mut() else {
            return false;
        };
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel")
            || word.is_empty()
            || word.eq_ignore_ascii_case("Enter")
        {
            self.cancel_interactive_command(true);
        } else if s.input.option(word) {
            s.tokens.push(word.into());
            self.arc_tilted_prompt();
        } else {
            if s.input.frame().is_none() && !s.input.requests_orientation() {
                return false;
            }
            let inline = word
                .split_once('=')
                .filter(|(key, _)| key.eq_ignore_ascii_case("Length"));
            let length =
                s.input.requests_length() || s.input.requests_orientation() || inline.is_some();
            let value = inline.map_or(text, |(_, v)| v);
            let parsed = if length {
                viboceros_drafting::PointInput::parse_length_with_units(
                    value,
                    self.document.units(),
                )
                .map(|v| v.map_err(|e| e.to_string()))
            } else {
                value.parse::<f64>().ok().map(Ok)
            };
            let Some(parsed) = parsed else {
                return false;
            };
            let mut input = s.input;
            if inline.is_some() {
                input.option("Length");
            }
            let result = parsed.and_then(|n| {
                input
                    .number(n)
                    .map_err(|e| e.to_string())
                    .map(|result| (n, result))
            });
            match result {
                Ok((n, None)) => {
                    s.input = input;
                    s.tokens.push(format!("Height={n}"));
                    self.arc_tilted_prompt();
                }
                Ok((n, Some(_))) => {
                    let tail = if inline.is_some() {
                        format!("Length={n}")
                    } else {
                        n.to_string()
                    };
                    self.finish_arc_tilted(&tail);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        self.command_input.clear();
        true
    }
}
