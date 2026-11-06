//! Corner picks, triangular completion and edit-free command lifetime.
use super::*;
use viboceros_command::srf_pt::{Definition, Input};

pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    tolerance: Tolerance,
}
impl VibocerosApp {
    pub(super) fn try_start_srf_pt(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.len() != 1
            || !["SrfPt", "SurfaceFromCorners"].iter().any(|n| {
                words[0]
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case(n)
            })
        {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("SrfPt") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.srf_pt_session = Some(Session {
            group,
            tolerance: self.document.tolerance(),
        });
        self.active_command = Some(InteractiveCommand::SrfPt { corners: [None; 3] });
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }
    fn validate_srf_pt(&mut self) -> bool {
        if self.srf_pt_session.as_ref().is_some_and(|s| {
            s.tolerance != self.document.tolerance()
                || !self.document.history_group_is_current(&s.group)
        }) {
            self.cancel_interactive_command(false);
            self.push_log("SrfPt document or tolerance changed; restart the command".into());
            false
        } else {
            true
        }
    }
    pub(super) fn srf_pt_input(&self) -> Option<Input> {
        if let Some(InteractiveCommand::SrfPt { corners }) = self.active_command {
            Some(Input::from_corners(corners))
        } else {
            None
        }
    }
    pub(super) fn accept_srf_pt(&mut self, corners: [Option<Point3>; 3], point: Point3) -> bool {
        if !self.validate_srf_pt() {
            return false;
        }
        let mut input = Input::from_corners(corners);
        if let Some(definition) = input.point(point) {
            return self.finish_srf_pt(definition);
        }
        self.active_command = Some(InteractiveCommand::SrfPt {
            corners: input.corners(),
        });
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }
    pub(super) fn continue_srf_pt(&mut self, text: &str) -> bool {
        let Some(input) = self.srf_pt_input() else {
            return false;
        };
        if !self.validate_srf_pt() {
            return true;
        }
        let word = text.trim().trim_start_matches('_');
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if let Some(definition) = input.enter() {
                self.finish_srf_pt(definition);
            } else {
                self.cancel_interactive_command(true);
            }
            return true;
        }
        false
    }
    fn finish_srf_pt(&mut self, definition: Definition) -> bool {
        if let Err(e) = definition.geometry(self.document.tolerance()) {
            self.cancel_interactive_command(false);
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let mut points = definition.perimeter();
        points.pop();
        let tail = if matches!(definition, Definition::Triangle(_)) {
            " Enter"
        } else {
            ""
        };
        self.try_execute_command(&format!(
            "SrfPt {}{tail}",
            points
                .into_iter()
                .map(format_model_point)
                .collect::<Vec<_>>()
                .join(" ")
        ))
    }
}
