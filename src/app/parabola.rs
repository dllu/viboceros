//! Parabola picking delegates construction, validation and previews to one input.
use super::*;
use viboceros_command::parabola::{Input, Kind, Slot};

pub(super) struct Session {
    pub(super) input: Input,
    pending: Option<&'static str>,
}
impl VibocerosApp {
    pub(super) fn try_start_parabola(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        let Some(kind) = words.next().and_then(Kind::from_name) else {
            return false;
        };
        let mut input = Input::new(
            kind,
            self.commands.parabola_options(kind),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.parabola_session = Some(Session {
            input,
            pending: None,
        });
        self.parabola_prompt();
        true
    }
    fn parabola_prompt(&mut self) {
        let Some(s) = &self.parabola_session else {
            return;
        };
        let options = s.input.options();
        let slot = match s.input.slot() {
            Slot::Focus => "focus",
            Slot::Vertex => "vertex",
            Slot::AxisDirection => "opening direction",
            Slot::Start => "start",
            Slot::End => "end",
            Slot::ThroughPoint => "through point",
            Slot::OpeningDirection => "opening direction",
            Slot::Complete => "complete",
        };
        self.active_command = Some(InteractiveCommand::Parabola {
            kind: s.input.kind(),
            anchor: s.input.anchor(),
        });
        self.drafting_plane = Some(s.input.mouse_plane());
        let mode = if s.input.kind() == Kind::Axis {
            format!(
                "{}Half={}",
                if s.input.point_count() == 0 {
                    "Focus, Vertex; "
                } else {
                    ""
                },
                if options.half { "Yes" } else { "No" }
            )
        } else if s.input.point_count() == 0 {
            format!(
                "Mode={:?}, PickOrder={:?}",
                options.mode, options.pick_order
            )
        } else if s.input.point_count() < 3 {
            format!("Mode={:?}", options.mode)
        } else {
            String::new()
        };
        let slot = match s.pending {
            Some("Mode") => "mode: Focus, ThroughPoint, Vertex",
            Some("PickOrder") => "pick order: Default, EndsFirst, EndsLast",
            _ => slot,
        };
        let mode = if mode.is_empty() {
            mode
        } else {
            format!("{mode}; ")
        };
        self.push_log(format!(
            "{}: {}; {mode}MarkFocus={}; Esc cancels",
            s.input.kind().name(),
            slot,
            if options.mark_focus { "Yes" } else { "No" }
        ));
    }
    pub(super) fn try_continue_parabola(&mut self, text: &str) -> bool {
        let Some(s) = &mut self.parabola_session else {
            return false;
        };
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("Cancel")
        {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if s.pending.is_some()
            && self
                .commands
                .recognizes(word.split_whitespace().next().unwrap_or(""))
        {
            return false;
        }
        let option = s
            .pending
            .map_or_else(|| word.to_owned(), |key| format!("{key}={word}"));
        let handled = match s.input.option(&option) {
            Ok(true) => {
                s.pending = None;
                self.point_filter = None;
                self.point_constraint = None;
                self.parabola_prompt();
                true
            }
            Err(e) => {
                if s.input.kind() == Kind::ThreePoint && word.eq_ignore_ascii_case("Mode") {
                    s.pending = Some("Mode");
                    self.parabola_prompt();
                } else if s.input.kind() == Kind::ThreePoint
                    && word.eq_ignore_ascii_case("PickOrder")
                {
                    s.pending = Some("PickOrder");
                    self.parabola_prompt();
                } else {
                    self.push_log(format!("Error: {e}"));
                }
                true
            }
            Ok(false) => {
                if s.pending.is_some()
                    && !self
                        .commands
                        .recognizes(word.split_whitespace().next().unwrap_or(""))
                {
                    self.parabola_prompt();
                    true
                } else {
                    false
                }
            }
        };
        if handled {
            self.command_input.clear();
        }
        handled
    }
    pub(super) fn accept_parabola_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.parabola_session.as_mut()?;
        if s.pending.is_some() {
            self.parabola_prompt();
            return Some(false);
        }
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(Some(_)) => {
                let command = s.input.command();
                self.try_execute_command(&command)
            }
            Ok(None) => {
                self.parabola_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        })
    }
}
