//! Source admission and endpoint picks never create document objects.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn pick_uv_subcurve_source(
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
            pending.source = Some(
                self.document
                    .object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .clone(),
            );
            self.active_command = Some(InteractiveCommand::SubCrv {
                start: None,
                copy: true,
                delete: false,
            });
            self.intersection_prompt = Some(prompt);
            self.log_intersection_prompt();
        } else {
            self.push_log("Select one curve to temporarily shorten".into());
        }
        true
    }

    pub(in crate::app) fn accept_uv_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        let mut prompt = self.intersection_prompt.clone()?;
        let pending = prompt.uv_subcurves.pending.as_ref()?.clone();
        let object = pending.object?;
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            if !pending.current(&self.document) {
                return Err(viboceros_command::CommandError::SubcurveInputStale);
            }
            let curve =
                pending
                    .curve(&self.document)
                    .ok_or(viboceros_command::CommandError::Usage(
                        "Select an existing curve",
                    ))?;
            let parameter = curve.closest_parameter(point, self.document.tolerance())?;
            if let Some(start) = pending.start {
                if pending.length.is_none()
                    && !curve.is_closed()?
                    && let Some(forward) = pending.locked_forward
                    && (start == parameter || (parameter > start) != forward)
                {
                    prompt.uv_subcurves.pending = None;
                    self.active_command = None;
                    self.drafting_plane = None;
                    return Ok(());
                }
                let parameters = if let Some(length) = pending.length {
                    let result = if let Some(forward) = pending.locked_forward {
                        viboceros_command::subcurve_input::interval_in_direction(
                            curve,
                            start,
                            length,
                            forward,
                            self.document.tolerance(),
                        )
                    } else {
                        viboceros_command::subcurve_input::interval(
                            curve,
                            start,
                            parameter,
                            length,
                            self.document.tolerance(),
                        )
                    };
                    let Some(parameters) = result? else {
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
                prompt.uv_subcurves.ranges.push(CurveRange {
                    object,
                    parameters,
                    source: self
                        .document
                        .object(object)
                        .unwrap()
                        .geometry_snapshot()
                        .clone(),
                    edge_reference: pending.edge_reference.clone(),
                    edge_wrap: pending.length.is_some(),
                });
                if let Some(order) = prompt.uv_subcurves.extraction_order.as_mut() {
                    order.push_range(
                        prompt.uv_subcurves.ranges.len() - 1,
                        self.document.selected_object_ids(),
                    );
                }
                prompt.uv_subcurves.pending = None;
                self.active_command = None;
                self.drafting_plane = None;
            } else {
                prompt.uv_subcurves.pending.as_mut().unwrap().start = Some(parameter);
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy: true,
                    delete: false,
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
