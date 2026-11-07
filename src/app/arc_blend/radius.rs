//! Native-style numeric option prompt, with atomic preview changes.
use super::*;
use viboceros_command::arc_blend::Handle;
impl VibocerosApp {
    pub(super) fn continue_arc_blend_radius(&mut self, text: &str) -> bool {
        let s = self.arc_blend_session.as_mut().unwrap();
        if s.radius_prompt {
            let value = if text.is_empty() {
                Ok(s.input.last_radius_difference())
            } else {
                viboceros_command::arc_blend::parse_number(text, self.document.units())
            };
            let result = value.and_then(|value| {
                if s.input.active_handle() == Some(Handle::Ratio) {
                    s.input.remember_radius_difference(value)
                } else {
                    s.input
                        .option(&format!("RadiusDifference={value}"))
                        .map(|_| ())
                }
            });
            match result {
                Ok(()) => {
                    s.radius_prompt = false;
                    s.candidate = None;
                    self.arc_blend_prompt();
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        if s.input.ready()
            && matches!(s.input.active_handle(), None | Some(Handle::Ratio))
            && text
                .trim_start_matches('_')
                .eq_ignore_ascii_case("RadiusDifference")
        {
            s.radius_prompt = true;
            s.candidate = None;
            self.arc_blend_prompt();
            return true;
        }
        false
    }
}
