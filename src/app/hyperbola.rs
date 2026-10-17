//! Interactive Hyperbola input, numeric coefficients and pure preview state.
use super::*;
use viboceros_command::hyperbola::{Coefficient, Construction, Input, Slot};
pub(super) struct Session {
    pub(super) input: Input,
    pending: Option<Coefficient>,
}
impl VibocerosApp {
    pub(super) fn try_start_hyperbola(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Hyperbola")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.commands.hyperbola_options(),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.hyperbola_session = Some(Session {
            input,
            pending: None,
        });
        self.hyperbola_prompt();
        true
    }
    fn hyperbola_prompt(&mut self) {
        let Some(s) = &self.hyperbola_session else {
            return;
        };
        let o = s.input.options();
        let phase = match s.input.slot() {
            Slot::Center => "center",
            Slot::FirstFocus => "first focus",
            Slot::SecondFocus => "second focus",
            Slot::Focus => "focus",
            Slot::Vertex => "vertex",
            Slot::Direction => "direction",
            Slot::End => "end",
            Slot::Complete => "complete",
        };
        let phase = s.pending.map_or_else(
            || phase.to_owned(),
            |key| {
                format!(
                    "new {} coefficient <{}>",
                    key.name(),
                    match key {
                        Coefficient::A => o.a,
                        Coefficient::B => o.b,
                    }
                )
            },
        );
        let options = if s.input.point_count() < 2 && s.input.construction() == Construction::Center
        {
            "; FromCoefficient, FromFoci, FromVertex".to_owned()
        } else if s.input.point_count() == 2 {
            format!(
                "{}; BothBranches={}; MarkFoci={}; ShowAsymptotes={}",
                if s.input.construction() == Construction::Coefficient {
                    format!("; A={}, B={}", o.a, o.b)
                } else {
                    String::new()
                },
                if o.both_branches { "Yes" } else { "No" },
                if o.mark_foci { "Yes" } else { "No" },
                if o.show_asymptotes { "Yes" } else { "No" }
            )
        } else {
            String::new()
        };
        self.active_command = Some(InteractiveCommand::Hyperbola {
            anchor: s.input.anchor(),
        });
        self.drafting_plane = Some(s.input.mouse_plane());
        self.push_log(format!("Hyperbola: {phase}{options}; Esc cancels"));
    }
    pub(super) fn try_continue_hyperbola(&mut self, text: &str) -> bool {
        let Some(s) = &mut self.hyperbola_session else {
            return false;
        };
        let word = text.trim().trim_start_matches('_');
        if s.input.cancels(word) || word.trim_start_matches('-').eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if s.pending.is_some() && (word.is_empty() || word.eq_ignore_ascii_case("Enter")) {
            s.pending = None;
            self.command_input.clear();
            self.hyperbola_prompt();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
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
        let named = if s.input.construction() == Construction::Coefficient {
            let (key, value) = word
                .split_once('=')
                .map_or((word, None), |(k, v)| (k, Some(v)));
            if key.eq_ignore_ascii_case("A") {
                Some((Coefficient::A, value))
            } else if key.eq_ignore_ascii_case("B") {
                Some((Coefficient::B, value))
            } else {
                None
            }
        } else {
            None
        };
        let numeric = s.pending.map(|key| (key, Some(word))).or(named);
        if let Some((key, value)) = numeric {
            self.point_filter = None;
            self.point_constraint = None;
            if let Some(value) = value {
                let value = viboceros_drafting::PointInput::parse_length_with_units(
                    value,
                    self.document.units(),
                )
                .ok_or_else(|| "Hyperbola coefficient requires a positive length".to_owned())
                .and_then(|r| r.map_err(|e| e.to_string()));
                match value
                    .and_then(|value| s.input.coefficient(key, value).map_err(|e| e.to_string()))
                {
                    Ok(()) => {
                        s.pending = None;
                        self.hyperbola_prompt();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            } else {
                s.pending = Some(key);
                self.hyperbola_prompt();
            }
            self.command_input.clear();
            return true;
        }
        match s.input.option(word) {
            Ok(true) => {
                self.point_filter = None;
                self.point_constraint = None;
                self.command_input.clear();
                self.hyperbola_prompt();
                true
            }
            Ok(false) => false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_input.clear();
                true
            }
        }
    }
    pub(super) fn accept_hyperbola_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.hyperbola_session.as_mut()?;
        if s.pending.is_some() {
            self.hyperbola_prompt();
            return Some(false);
        }
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(Some(_)) => {
                let command = s.input.command();
                let plane = s.input.mouse_plane();
                self.try_execute_command_with_plane(&command, Some(plane))
            }
            Ok(None) => {
                self.hyperbola_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
}
