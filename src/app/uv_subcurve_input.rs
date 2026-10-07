//! Nested, nonmutating SubCrv input for the UV source getter.
use super::*;
use viboceros_document::ObjectId;
mod length_confirmation;

#[derive(Clone, Debug)]
pub(super) struct CurveRange {
    pub(super) object: ObjectId,
    pub(super) parameters: [f64; 2],
}

#[derive(Clone, Debug, Default)]
pub(super) struct SubcurveInputs {
    pub(super) ranges: Vec<CurveRange>,
    pub(super) pending: Option<PendingSubcurve>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PendingSubcurve {
    pub(super) object: Option<ObjectId>,
    pub(super) start: Option<f64>,
    pub(super) length: Option<f64>,
}

impl SubcurveInputs {
    pub(super) fn arguments(&self) -> String {
        self.ranges
            .iter()
            .map(|r| {
                format!(
                    " SubCrv={},{},{}",
                    r.object, r.parameters[0], r.parameters[1]
                )
            })
            .collect()
    }
    pub(super) fn hint(&self) -> Option<&'static str> {
        self.pending.as_ref().map(|p| match (p.object, p.start,p.length) {
            (None, _,_) => "Select curve to temporarily shorten; Enter returns, Esc cancels",
            (Some(_), None,_) => "Pick start of temporary subcurve; Esc cancels",
            (Some(_), Some(_),None) => "Pick directed end or type a curve length; Esc cancels",
            (Some(_), Some(_),Some(_)) => "Confirm the length direction on the curve; type a new length or Enter to return",
        })
    }
}

impl VibocerosApp {
    pub(super) fn try_continue_uv_subcurve(
        &mut self,
        prompt: &intersect_two_sets::TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if let Some(pending) = &prompt.uv_subcurves.pending {
            if input.is_empty() {
                self.finish_empty_uv_subcurve();
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
                        let curve = self
                            .document
                            .object(pending.object.unwrap())
                            .and_then(|o| o.geometry().curve_ref());
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

    pub(super) fn pick_uv_subcurve_source(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> bool {
        let Some(mut prompt) = self.intersection_prompt.clone() else {
            return false;
        };
        let Some(pending) = prompt.uv_subcurves.pending.as_mut() else {
            return false;
        };
        if pending.object.is_some() {
            return true;
        }
        let ids = ids
            .into_iter()
            .filter(|id| {
                self.document.is_object_selectable(*id)
                    && self
                        .document
                        .object(*id)
                        .is_some_and(|o| o.geometry().curve_ref().is_some())
            })
            .collect::<Vec<_>>();
        if let [id] = ids.as_slice() {
            pending.object = Some(*id);
            self.active_command = Some(InteractiveCommand::SubCrv {
                start: None,
                copy: true,
            });
            self.intersection_prompt = Some(prompt);
            self.log_intersection_prompt();
        } else {
            self.push_log("Select one curve to temporarily shorten".into());
        }
        true
    }

    pub(super) fn accept_uv_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        let mut prompt = self.intersection_prompt.clone()?;
        let pending = prompt.uv_subcurves.pending.as_ref()?.clone();
        let object = pending.object?;
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            let curve = self
                .document
                .object(object)
                .and_then(|o| o.geometry().curve_ref())
                .ok_or(viboceros_command::CommandError::Usage(
                    "Select an existing curve",
                ))?;
            let parameter = curve.closest_parameter(point, self.document.tolerance())?;
            if let Some(start) = pending.start {
                let parameters = if let Some(length) = pending.length {
                    let Some(parameters) = length_confirmation::interval(
                        curve,
                        start,
                        parameter,
                        length,
                        self.document.tolerance(),
                    )?
                    else {
                        prompt.uv_subcurves.pending = None;
                        self.active_command = None;
                        self.drafting_plane = None;
                        return Ok(());
                    };
                    parameters
                } else {
                    [start, parameter]
                };
                curve
                    .to_owned()
                    .try_subcurve(parameters[0], parameters[1])?;
                prompt
                    .uv_subcurves
                    .ranges
                    .push(CurveRange { object, parameters });
                prompt.uv_subcurves.pending = None;
                self.active_command = None;
                self.drafting_plane = None;
            } else {
                prompt.uv_subcurves.pending.as_mut().unwrap().start = Some(parameter);
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy: true,
                });
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                if self.active_command.is_none() {
                    self.point_filter = None;
                    self.point_constraint = None;
                }
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
                Some(true)
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                Some(false)
            }
        }
    }
}
