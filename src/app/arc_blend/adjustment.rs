//! Handle selection and bounded cursor previews remain outside model history.
use super::*;
use viboceros_command::arc_blend::Handle;
impl VibocerosApp {
    pub(in crate::app) fn select_arc_blend_handle(&mut self, handle: Handle) {
        if self.plane_prompt.is_some() {
            return;
        }
        self.validate_arc_blend();
        let Some(s) = &mut self.arc_blend_session else {
            return;
        };
        match s.input.select_handle(handle) {
            Ok(()) => {
                s.candidate = None;
                self.arc_blend_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(in crate::app) fn hover_arc_blend_parameter(&mut self, t: f64) {
        if self.plane_prompt.is_some() {
            return;
        }
        self.validate_arc_blend();
        let Some(s) = &mut self.arc_blend_session else {
            return;
        };
        if s.candidate.as_ref().is_some_and(|(old, _)| *old == t) {
            return;
        }
        let mut next = s.input.clone();
        s.candidate = next.adjust_parameter(t).is_ok().then_some((t, next));
    }
    pub(in crate::app) fn accept_arc_blend_parameter(&mut self, t: f64) -> bool {
        if self.plane_prompt.is_some()
            || self
                .arc_blend_session
                .as_ref()
                .is_none_or(|s| s.input.active_handle().is_none())
        {
            return false;
        }
        self.hover_arc_blend_parameter(t);
        let s = self.arc_blend_session.as_mut().unwrap();
        if let Some((_, mut next)) = s.candidate.take() {
            next.finish_adjustment();
            s.input = next;
            self.arc_blend_prompt();
        } else {
            self.push_log("Error: no valid arc blend at this location".into());
        }
        true
    }
    pub(in crate::app) fn cancel_arc_blend_adjustment(&mut self) -> bool {
        if self.plane_prompt.is_some()
            || self
                .arc_blend_session
                .as_ref()
                .is_none_or(|s| s.input.active_handle().is_none())
        {
            return false;
        }
        let s = self.arc_blend_session.as_mut().unwrap();
        s.input.finish_adjustment();
        s.candidate = None;
        self.command_input.clear();
        self.arc_blend_prompt();
        true
    }
    pub(in crate::app) fn continue_arc_blend_adjustment(&mut self, text: &str) -> bool {
        if text.is_empty() {
            self.cancel_arc_blend_adjustment();
            return true;
        }
        let s = self.arc_blend_session.as_ref().unwrap();
        let mut next = s.input.clone();
        let result = if next.active_handle() == Some(Handle::Ratio) && text.parse::<f64>().is_ok() {
            next.option(&format!("Ratio={text}")).map(|_| ())
        } else {
            let point =
                viboceros_drafting::PointInput::parse_with_units(text, self.document.units())
                    .and_then(Result::ok)
                    .and_then(|p| {
                        p.resolve(
                            self.viewports[self.active_viewport].construction_plane(),
                            self.last_point,
                        )
                        .ok()
                    });
            match point {
                Some(p) => next.adjust_point(p),
                None => Err(viboceros_command::CommandError::Usage(
                    "Expected an arc blend adjustment point or ratio",
                )),
            }
        };
        match result {
            Ok(()) => {
                next.finish_adjustment();
                let s = self.arc_blend_session.as_mut().unwrap();
                s.input = next;
                s.candidate = None;
                self.arc_blend_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
}
