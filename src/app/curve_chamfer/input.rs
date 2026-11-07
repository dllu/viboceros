//! Curve-pick options stay available between the ordered source selections.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn continue_curve_chamfer(&mut self, text: &str) -> bool {
        if self.curve_chamfer_session.is_none() || self.plane_prompt.is_some() {
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
            && !["Distances", "Join", "Trim"]
                .iter()
                .any(|k| first.eq_ignore_ascii_case(k))
        {
            self.cancel_interactive_command(false);
            return false;
        }
        self.validate_curve_chamfer();
        if self.curve_chamfer_session.is_none() {
            return true;
        }
        if self.continue_curve_chamfer_distance(text) {
            self.command_input.clear();
            return true;
        }
        let s = self.curve_chamfer_session.as_ref().unwrap();
        let key = word.split_once('=').map_or(word, |(k, _)| k);
        if !s.options.trim && key.eq_ignore_ascii_case("Join") {
            self.push_log("Join is available with Trim=Yes".into());
            return true;
        }
        if text.is_empty() {
            self.curve_chamfer_session.as_mut().unwrap().pending = None;
            self.curve_chamfer_prompt();
            return true;
        }
        let edit = if word.eq_ignore_ascii_case("ExtendArcsBy") {
            format!(
                "ExtendArcsBy={}",
                if s.options.styles.arc == viboceros_geometry::CurveArcExtensionStyle::Arc {
                    "Line"
                } else {
                    "Arc"
                }
            )
        } else if word.eq_ignore_ascii_case("ExtendOtherCurvesBy") {
            format!(
                "ExtendOtherCurvesBy={}",
                if s.options.styles.other == viboceros_geometry::CurveOtherExtensionStyle::Line {
                    "Smooth"
                } else {
                    "Line"
                }
            )
        } else if word.eq_ignore_ascii_case("Join") || word.eq_ignore_ascii_case("Trim") {
            format!(
                "{word}={}",
                if (word.eq_ignore_ascii_case("Join") && s.options.join)
                    || (word.eq_ignore_ascii_case("Trim") && s.options.trim)
                {
                    "No"
                } else {
                    "Yes"
                }
            )
        } else {
            text.to_owned()
        };
        match s.options.updated(
            &edit.split_whitespace().collect::<Vec<_>>(),
            self.document.units(),
        ) {
            Ok(options) => {
                let s = self.curve_chamfer_session.as_mut().unwrap();
                s.options = options;
                s.pending = None;
                self.curve_chamfer_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
}
