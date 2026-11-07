//! Numeric and point entry keep the accepted parent getter outside model history.
use super::*;
impl VibocerosApp {
    pub(in crate::app::arc_blend) fn continue_arc_blend_subcurve(&mut self, text: &str) -> bool {
        let s = self.arc_blend_session.as_ref().unwrap();
        let word = text.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("SubCrv") && s.subcurve.is_none() && !s.input.ready() {
            let s = self.arc_blend_session.as_mut().unwrap();
            s.subcurve = Some(Editor::default());
            s.binding = None;
            s.candidate = None;
            self.arc_blend_prompt();
            return true;
        }
        let Some(p) = s.subcurve.as_ref() else {
            return false;
        };
        if text.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("Cancel")
        {
            self.discard_arc_subcurve();
            return true;
        }
        if p.reference.is_none() {
            let binding = text
                .split_whitespace()
                .next()
                .and_then(viboceros_command::line::normal::parse_binding);
            if let Some((id, kind)) = binding {
                let edge = match kind {
                    viboceros_command::line::normal::Component::Object => None,
                    viboceros_command::line::normal::Component::Edge(e) => Some(e),
                    _ => {
                        self.push_log("Select a curve or edge".into());
                        return true;
                    }
                };
                self.pick_arc_subcurve_reference(id, edge, None);
            } else if let Ok(id) = text.parse::<ObjectId>() {
                self.pick_arc_subcurve_reference(id, None, None);
            } else {
                self.push_log("Select curve to temporarily shorten".into());
            }
            return true;
        }
        let source = p.reference.as_ref().unwrap();
        let c = source.curve_ref();
        if let Some(direction) = subcurve_input::direction_option(text) {
            let result = direction.map_err(str::to_owned).and_then(|locked| {
                if !locked {
                    return Ok(None);
                }
                let start = p.start.ok_or("Pick the start before locking direction")?;
                let hover = p
                    .hover
                    .ok_or("Move along the curve before locking direction")?;
                subcurve_input::cursor_forward(c, start, hover)
                    .map(Some)
                    .map_err(|e| e.to_string())
            });
            match result {
                Ok(locked) => {
                    self.arc_blend_session
                        .as_mut()
                        .unwrap()
                        .subcurve
                        .as_mut()
                        .unwrap()
                        .locked = locked;
                    self.arc_blend_prompt();
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            };
            return true;
        }
        if let Some(start) = p.start
            && let Some(quantity) =
                viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
        {
            let result = quantity.map_err(|e| e.to_string()).and_then(|v| {
                let length = v.abs();
                if length
                    > c.length(self.document.tolerance())
                        .map_err(|e| e.to_string())?
                {
                    return Err("Curve length exceeds the whole source curve".into());
                }
                Ok(length)
            });
            match result {
                Ok(0.) => {
                    self.discard_arc_subcurve();
                }
                Ok(length) => {
                    let locked = p.locked;
                    let confirmation = locked
                        .map(|forward| {
                            subcurve_input::locked_numeric_requires_confirmation(
                                c,
                                start,
                                length,
                                forward,
                                self.document.tolerance(),
                            )
                        })
                        .transpose();
                    match confirmation {
                        Ok(required) => {
                            let p = self
                                .arc_blend_session
                                .as_mut()
                                .unwrap()
                                .subcurve
                                .as_mut()
                                .unwrap();
                            p.length = Some(length);
                            if required == Some(true) {
                                p.locked = None;
                            }
                            if required == Some(false) {
                                self.accept_arc_subcurve_native_parameter(start);
                            } else {
                                self.arc_blend_prompt();
                            }
                        }
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        let point = viboceros_drafting::PointInput::parse_with_units(text, self.document.units())
            .and_then(Result::ok)
            .and_then(|p| {
                p.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    self.last_point,
                )
                .ok()
            });
        match point.map(|p| c.closest_parameter(p, self.document.tolerance())) {
            Some(Ok(t)) => self.accept_arc_subcurve_native_parameter(t),
            Some(Err(e)) => self.push_log(format!("Error: {e}")),
            None => self.push_log("Pick a point on the temporary source curve".into()),
        }
        true
    }
}
