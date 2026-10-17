//! Ellipse construction uses the same edit-free getter for previews and commits.
use super::*;
use viboceros_command::ellipse_input::{Input, Mode, Slot};

pub(super) struct Session {
    pub(super) input: Input,
    pub(super) cursor: Option<Point3>,
}
impl VibocerosApp {
    pub(super) fn try_start_ellipse(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            ["Ellipse", "Ell"]
                .iter()
                .any(|n| w.trim_start_matches(['_', '-']).eq_ignore_ascii_case(n))
        }) {
            return false;
        }
        let mut input = Input::new(
            self.commands.ellipse_options(),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.ellipse_session = Some(Session {
            input,
            cursor: None,
        });
        self.ellipse_prompt();
        true
    }
    fn ellipse_prompt(&mut self) {
        let Some(s) = &self.ellipse_session else {
            return;
        };
        let input = s.input;
        let slot = match input.slot() {
            Slot::First => {
                if input.mode() == Mode::FromFoci {
                    "first focus"
                } else if input.mode() == Mode::Corner {
                    "first corner"
                } else if input.mode() == Mode::Diameter {
                    "first axis start"
                } else {
                    "center"
                }
            }
            Slot::FirstAxis => "first axis end",
            Slot::SecondAxis => "second axis end",
            Slot::OppositeCorner if input.centered_corner() => "corner",
            Slot::OppositeCorner => "opposite corner",
            Slot::SecondFocus => "second focus",
            Slot::ThroughPoint => "point on ellipse",
            Slot::Complete => "complete",
        };
        self.active_command = Some(InteractiveCommand::Ellipse {
            center: input.anchor(),
            first_axis: input.reference(),
        });
        self.drafting_plane = Some(input.mouse_plane());
        let options = if input.slot() == Slot::First {
            "; Default, Vertical, Corner, Diameter, FromFoci"
        } else {
            ""
        };
        let foci = if input.mode() == Mode::FromFoci {
            format!(
                "; MarkFoci={}",
                if input.options().mark_foci {
                    "Yes"
                } else {
                    "No"
                }
            )
        } else {
            String::new()
        };
        let length = input
            .locked_length()
            .map_or(String::new(), |v| format!("; length={v}"));
        let vertical = if input.vertical() && input.mode() == Mode::Diameter {
            " Vertical"
        } else {
            ""
        };
        self.push_log(format!(
            "Ellipse {}{vertical}: {slot}{options}{foci}{length}; Esc cancels",
            input.mode().name()
        ));
    }
    pub(super) fn try_continue_ellipse(&mut self, text: &str) -> bool {
        let Some(s) = &mut self.ellipse_session else {
            return false;
        };
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.input.locked_length().is_some() {
                if let Some(cursor) = s.cursor {
                    self.accept_filtered_drafting_point(cursor, false);
                } else {
                    self.push_log(
                        "Ellipse: move the pointer to choose the locked axis direction".into(),
                    );
                }
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        match s.input.option(text) {
            Ok(true) => {
                s.cursor = None;
                self.point_filter = None;
                self.point_constraint = None;
                self.ellipse_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
            Ok(false) => {
                if self
                    .commands
                    .recognizes(word.split_whitespace().next().unwrap_or(""))
                {
                    return false;
                }
                let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                    text,
                    self.document.units(),
                ) else {
                    return false;
                };
                match value {
                    Ok(value) => match s.input.number(value) {
                        Ok(_) => self.ellipse_prompt(),
                        Err(e) => self.push_log(format!("Error: {e}")),
                    },
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn accept_ellipse_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.ellipse_session.as_mut()?;
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(Some(_)) => {
                let command = s.input.command().expect("completed ellipse");
                self.try_execute_command(&command)
            }
            Ok(None) => {
                s.cursor = None;
                self.ellipse_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
}
