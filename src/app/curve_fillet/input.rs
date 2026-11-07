//! Command options and continuation while the ordered getter is active.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn continue_curve_fillet(&mut self, text: &str) -> bool {
        if self.curve_fillet_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        let text = text.trim();
        let word = text.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") || text == "!" {
            self.cancel_interactive_command(true);
            return true;
        }
        let first = word.split_whitespace().next().unwrap_or("");
        if self.commands.recognizes(first)
            && self
                .curve_fillet_session
                .as_ref()
                .unwrap()
                .pending
                .is_none_or(|p| p == "Radius")
            && !["Radius", "Join", "Trim"]
                .iter()
                .any(|o| first.eq_ignore_ascii_case(o))
        {
            self.cancel_interactive_command(false);
            return false;
        }
        self.validate_curve_fillet();
        let Some(_) = self.curve_fillet_session.as_ref() else {
            return true;
        };
        if self.continue_curve_fillet_radius(text) {
            return true;
        }
        let s = self.curve_fillet_session.as_ref().unwrap();
        if !s.options.trim
            && word
                .split_once('=')
                .map_or(word, |(key, _)| key)
                .eq_ignore_ascii_case("Join")
        {
            self.push_log("Join is available with Trim=Yes".into());
            return true;
        }
        if text.is_empty() {
            if s.pending.is_some() {
                self.curve_fillet_session.as_mut().unwrap().pending = None;
                self.curve_fillet_prompt();
            } else if s.picks.len() == 2 {
                self.update_curve_fillet_preview(None, true);
                self.commit_curve_fillet();
            } else {
                self.curve_fillet_prompt();
            }
            return true;
        }
        let edit = if let Some(key) = s.pending {
            format!("{key}={word}")
        } else if ["ExtendArcsBy", "ExtendOtherCurvesBy"]
            .iter()
            .any(|o| word.eq_ignore_ascii_case(o))
        {
            self.curve_fillet_session.as_mut().unwrap().pending =
                Some(if word.eq_ignore_ascii_case("ExtendArcsBy") {
                    "ExtendArcsBy"
                } else {
                    "ExtendOtherCurvesBy"
                });
            self.curve_fillet_prompt();
            return true;
        } else if ["Join", "Trim", "Dynamic"]
            .iter()
            .any(|o| word.eq_ignore_ascii_case(o))
        {
            let old = if word.eq_ignore_ascii_case("Join") {
                s.options.join
            } else if word.eq_ignore_ascii_case("Trim") {
                s.options.trim
            } else {
                s.options.dynamic
            };
            format!("{word}={}", if old { "No" } else { "Yes" })
        } else {
            text.to_owned()
        };
        if s.picks.len() == 2
            && edit
                .split_once('=')
                .is_some_and(|(k, _)| k.eq_ignore_ascii_case("Dynamic"))
        {
            self.push_log("Dynamic is available while selecting the curves".into());
            return true;
        }
        match s.options.updated(
            &edit.split_whitespace().collect::<Vec<_>>(),
            self.document.units(),
        ) {
            Ok(options) => {
                let s = self.curve_fillet_session.as_mut().unwrap();
                s.options = options;
                s.pending = None;
                self.update_curve_fillet_preview(None, true);
                self.curve_fillet_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
}
