//! Nested source, direction and numeric input routing.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn continue_curve_boolean_subcurve(&mut self, text: &str) -> bool {
        let Some(prompt) = self.curve_boolean_prompt.as_ref().filter(|p| p.selecting()) else {
            return false;
        };
        if prompt.subcurves.pending.is_none() {
            if text.trim_start_matches('_').eq_ignore_ascii_case("SubCrv") {
                self.curve_boolean_prompt
                    .as_mut()
                    .unwrap()
                    .subcurves
                    .pending = Some(PendingSubcurve::default());
                self.command_input.clear();
                self.log_curve_boolean();
                return true;
            }
            return false;
        }
        if text.is_empty() || text.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            return self.discard_curve_boolean_subcurve();
        }
        let pending = prompt.subcurves.pending.as_ref().unwrap();
        if pending.object.is_none() {
            if let Ok(id) = text.parse::<ObjectId>() {
                self.pick_curve_boolean_subcurve(id, None);
                self.command_input.clear();
                return true;
            }
            if let Some((_, value)) = text
                .split_once('=')
                .filter(|(key, _)| key.trim_start_matches('_').eq_ignore_ascii_case("Edge"))
                && let Some((id, edge)) = viboceros_command::curve_reference::parse_edge(value)
            {
                self.pick_curve_boolean_subcurve(id, Some(edge));
                self.command_input.clear();
                return true;
            }
        } else {
            if !pending.current(&self.document) {
                self.discard_curve_boolean_subcurve();
                self.push_log("Temporary source changed; select it again".into());
                return true;
            }
            if let Some(start) = pending.start {
                if let Some(direction) = subcurve_input::direction_option(text) {
                    let value = direction.and_then(|locked| {
                        if !locked
                            || (text.trim_start_matches('_').eq_ignore_ascii_case("D")
                                && pending.locked_forward.is_some())
                        {
                            return Ok(None);
                        }
                        let hover = pending
                            .hover_parameter
                            .ok_or("Move along the curve before locking direction")?;
                        subcurve_input::cursor_forward(
                            pending.curve(&self.document).unwrap(),
                            start,
                            hover,
                        )
                        .map(Some)
                        .map_err(|_| "Invalid curve direction")
                    });
                    match value {
                        Ok(forward) => {
                            self.curve_boolean_prompt
                                .as_mut()
                                .unwrap()
                                .subcurves
                                .pending
                                .as_mut()
                                .unwrap()
                                .locked_forward = forward
                        }
                        Err(error) => self.push_log(error.into()),
                    }
                    self.command_input.clear();
                    return true;
                }
                if let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                    text,
                    self.document.units(),
                ) {
                    match length {
                        Ok(0.) => {
                            self.discard_curve_boolean_subcurve();
                        }
                        Ok(length) => {
                            let curve = pending.curve(&self.document).unwrap();
                            if curve
                                .length(self.document.tolerance())
                                .is_ok_and(|total| length.abs() <= total)
                            {
                                let forward = pending.locked_forward;
                                let anchor = curve.evaluate(start).unwrap();
                                let confirmation = forward
                                    .map(|f| {
                                        subcurve_input::locked_numeric_requires_confirmation(
                                            curve,
                                            start,
                                            length.abs(),
                                            f,
                                            self.document.tolerance(),
                                        )
                                    })
                                    .transpose();
                                let confirmation = match confirmation {
                                    Ok(value) => value.unwrap_or(false),
                                    Err(error) => {
                                        self.push_log(format!("Error: {error}"));
                                        return true;
                                    }
                                };
                                self.curve_boolean_prompt
                                    .as_mut()
                                    .unwrap()
                                    .subcurves
                                    .pending
                                    .as_mut()
                                    .unwrap()
                                    .length = Some(length.abs());
                                if confirmation {
                                    self.curve_boolean_prompt
                                        .as_mut()
                                        .unwrap()
                                        .subcurves
                                        .pending
                                        .as_mut()
                                        .unwrap()
                                        .locked_forward = None;
                                } else if forward.is_some() {
                                    self.accept_curve_boolean_subcurve_point(anchor);
                                }
                            } else {
                                self.push_log("Curve length exceeds the whole source curve".into());
                            }
                        }
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                    self.command_input.clear();
                    self.log_curve_boolean();
                    return true;
                }
            }
            if self.try_continue_point_constraint(text)
                || self.try_continue_point_filter(text)
                || self.try_continue_point_input(text)
            {
                return true;
            }
        }
        if self
            .commands
            .recognizes(text.split_whitespace().next().unwrap_or(""))
        {
            return false;
        }
        self.log_curve_boolean();
        self.command_input.clear();
        true
    }
}
