//! Native-style numeric option prompt, with atomic preview changes.
use super::*;
impl VibocerosApp {
    pub(super) fn continue_arc_blend_radius(&mut self, text: &str) -> bool {
        let s = self.arc_blend_session.as_mut().unwrap();
        if s.radius_prompt {
            if text.is_empty() {
                s.radius_prompt = false;
                self.arc_blend_prompt();
                return true;
            }
            match s.input.option(&format!("RadiusDifference={text}")) {
                Ok(true) => {
                    s.radius_prompt = false;
                    s.candidate = None;
                    self.arc_blend_prompt();
                }
                Ok(false) => unreachable!(),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        if s.input.ready()
            && s.input.active_handle().is_none()
            && text
                .trim_start_matches('_')
                .eq_ignore_ascii_case("RadiusDifference")
        {
            s.radius_prompt = true;
            self.arc_blend_prompt();
            return true;
        }
        false
    }
}
