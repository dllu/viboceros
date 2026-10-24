//! Nested command input and numeric length confirmation.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn try_continue_uv_subcurve(
        &mut self,
        prompt: &intersect_two_sets::TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if let Some(pending) = &prompt.uv_subcurves.pending {
            if input.is_empty() {
                self.finish_empty_uv_subcurve();
                return true;
            }
            if pending.object.is_some() && !pending.current(&self.document) {
                self.push_log(format!(
                    "Error: {}",
                    viboceros_command::CommandError::SubcurveInputStale
                ));
                return true;
            }
            if pending.start.is_some()
                && let Some(quantity) = viboceros_drafting::PointInput::parse_length_with_units(
                    input,
                    self.document.units(),
                )
            {
                match quantity {
                    Ok(0.) => self.finish_empty_uv_subcurve(),
                    Ok(length) => {
                        let curve = pending.curve(&self.document);
                        match curve.map(|c| c.length(self.document.tolerance())) {
                            Some(Ok(total)) if length.abs() <= total => {
                                self.intersection_prompt
                                    .as_mut()
                                    .unwrap()
                                    .uv_subcurves
                                    .pending
                                    .as_mut()
                                    .unwrap()
                                    .length = Some(length.abs());
                                self.command_input.clear();
                                if let Some(forward) = pending.locked_forward {
                                    match viboceros_command::subcurve_input::locked_numeric_requires_confirmation(
                                        curve.unwrap(), pending.start.unwrap(), length.abs(), forward, self.document.tolerance(),
                                    ) {
                                        Ok(true) => {
                                            self.intersection_prompt.as_mut().unwrap().uv_subcurves.pending.as_mut().unwrap().locked_forward = None;
                                            self.log_intersection_prompt();
                                            return true;
                                        }
                                        Ok(false) => {}
                                        Err(error) => {
                                            self.push_log(format!("Error: {error}"));
                                            return true;
                                        }
                                    }
                                }
                                if pending.locked_forward.is_some() {
                                    let point = curve.unwrap().evaluate(pending.start.unwrap());
                                    match point {
                                        Ok(point) => {
                                            self.accept_uv_subcurve_point(point);
                                        }
                                        Err(error) => self.push_log(format!("Error: {error}")),
                                    }
                                    return true;
                                }
                                self.log_intersection_prompt();
                            }
                            Some(Err(error)) => self.push_log(format!("Error: {error}")),
                            _ => {
                                self.push_log("Curve length exceeds the whole source curve".into())
                            }
                        }
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                return true;
            }
            if pending.object.is_some()
                && (self.try_continue_point_constraint(input)
                    || self.try_continue_point_filter(input)
                    || self.try_continue_point_input(input))
            {
                return true;
            }
            if pending.object.is_none()
                && self.picking_uv_subcurve_edge()
                && let Some((name, value)) = input.split_once('=')
                && name.trim_start_matches('_').eq_ignore_ascii_case("Edge")
                && let Some((object, edge)) = viboceros_command::curve_reference::parse_edge(value)
            {
                self.pick_uv_subcurve_edge(crate::viewport::EdgePick { object, edge });
                self.command_input.clear();
                return true;
            }
            if pending.object.is_none()
                && let Ok(id) = input.parse::<ObjectId>()
            {
                self.pick_uv_subcurve_source([id]);
                self.command_input.clear();
                return true;
            }
            if !self
                .commands
                .recognizes(input.split_whitespace().next().unwrap_or(""))
            {
                self.push_log(prompt.hint().to_owned());
                self.command_input.clear();
                return true;
            }
            // A different command is handled by the parent getter's cancel path.
            return false;
        }
        if !self.picking_uv_reference()
            && input.trim_start_matches('_').eq_ignore_ascii_case("SubCrv")
        {
            let mut prompt = prompt.clone();
            prompt.uv_subcurves.pending = Some(PendingSubcurve::default());
            self.intersection_prompt = Some(prompt);
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        false
    }

    fn finish_empty_uv_subcurve(&mut self) {
        if let Some(prompt) = self.intersection_prompt.as_mut() {
            prompt.uv_subcurves.pending = None;
        }
        self.active_command = None;
        self.drafting_plane = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        self.log_intersection_prompt();
    }
}
