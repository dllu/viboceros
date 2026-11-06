//! Temporary curve selection shares the existing length, direction and preview helpers.
use super::*;
mod input;
use crate::app::uv_subcurve_input::{CurveRange, PendingSubcurve};
use viboceros_command::{curve_reference::CapturedCurve, subcurve_input};

impl VibocerosApp {
    pub(in crate::app) fn picking_curve_boolean_subcurve_edge(&self) -> bool {
        self.curve_boolean_prompt
            .as_ref()
            .and_then(|p| p.subcurves.pending.as_ref())
            .is_some_and(|p| p.object.is_none())
    }
    pub(in crate::app) fn pick_curve_boolean_subcurve(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
    ) -> bool {
        let Some(pending) = self
            .curve_boolean_prompt
            .as_ref()
            .and_then(|p| p.subcurves.pending.as_ref())
        else {
            return false;
        };
        if pending.object.is_some() {
            return true;
        }
        match CapturedCurve::capture(&self.document, id, edge) {
            Ok(reference) => {
                let pending = self
                    .curve_boolean_prompt
                    .as_mut()
                    .unwrap()
                    .subcurves
                    .pending
                    .as_mut()
                    .unwrap();
                pending.object = Some(id);
                pending.source = Some(reference.source().0.clone());
                pending.edge_reference = edge.map(|_| reference);
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: None,
                    copy: true,
                    delete: false,
                });
                self.log_curve_boolean();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    pub(in crate::app) fn discard_curve_boolean_subcurve(&mut self) -> bool {
        let Some(prompt) = self.curve_boolean_prompt.as_mut() else {
            return false;
        };
        if prompt.subcurves.pending.take().is_none() {
            return false;
        }
        self.active_command = None;
        self.point_constraint = None;
        self.point_filter = None;
        self.command_input.clear();
        self.log_curve_boolean();
        true
    }
    pub(in crate::app) fn accept_curve_boolean_subcurve_point(
        &mut self,
        point: Point3,
    ) -> Option<bool> {
        let pending = self
            .curve_boolean_prompt
            .as_ref()?
            .subcurves
            .pending
            .as_ref()?
            .clone();
        let id = pending.object?;
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            if !pending.current(&self.document) {
                return Err(viboceros_command::CommandError::SubcurveInputStale);
            }
            let curve = pending.curve(&self.document).unwrap();
            let parameter = curve.closest_parameter(point, self.document.tolerance())?;
            let Some(start) = pending.start else {
                return Ok((None, parameter));
            };
            let parameters = if let Some(length) = pending.length {
                if let Some(forward) = pending.locked_forward {
                    subcurve_input::interval_in_direction(
                        curve,
                        start,
                        length,
                        forward,
                        self.document.tolerance(),
                    )?
                } else {
                    subcurve_input::interval(
                        curve,
                        start,
                        parameter,
                        length,
                        self.document.tolerance(),
                    )?
                }
            } else {
                subcurve_input::point_parameters(
                    curve,
                    start,
                    parameter,
                    pending.locked_forward,
                    true,
                )?
            };
            let Some(parameters) = parameters else {
                return Ok((Some(None), parameter));
            };
            let source = viboceros_command::curve_boolean::Source::Subcurve {
                object: id,
                edge: pending.edge(),
                parameters,
                wrap_edge: pending.length.is_some(),
            };
            // Validate the exact same restriction which the command will use.
            source.validate(&self.document)?;
            Ok((Some(Some(parameters)), parameter))
        })();
        match result {
            Ok((None, parameter)) => {
                self.curve_boolean_prompt
                    .as_mut()
                    .unwrap()
                    .subcurves
                    .pending
                    .as_mut()
                    .unwrap()
                    .start = Some(parameter);
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy: true,
                    delete: false,
                });
            }
            Ok((Some(parameters), _)) => {
                if let Some(parameters) = parameters {
                    self.curve_boolean_prompt
                        .as_mut()
                        .unwrap()
                        .subcurves
                        .ranges
                        .push(CurveRange {
                            object: id,
                            parameters,
                            source: pending.source.unwrap(),
                            edge_reference: pending.edge_reference,
                            edge_wrap: pending.length.is_some(),
                        });
                }
                self.discard_curve_boolean_subcurve();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.log_curve_boolean();
        Some(true)
    }
    pub(in crate::app) fn update_curve_boolean_subcurve_hover(&mut self, point: Point3) -> bool {
        let Some(p) = self
            .curve_boolean_prompt
            .as_mut()
            .and_then(|p| p.subcurves.pending.as_mut())
        else {
            return false;
        };
        if p.object.is_none() {
            return false;
        }
        p.hover_parameter = p.preview.parameter(
            p.source.as_ref(),
            p.edge(),
            point,
            self.document.tolerance(),
        );
        true
    }
    pub(in crate::app) fn curve_boolean_subcurve_preview(
        &mut self,
    ) -> Option<crate::app::subcurve_preview::Preview> {
        let pending = self
            .curve_boolean_prompt
            .as_mut()?
            .subcurves
            .pending
            .as_mut()?;
        pending.preview.temporary_retaining_orientation(
            pending.source.as_ref(),
            pending.edge(),
            [pending.start?, pending.hover_parameter?],
            pending.length,
            pending.locked_forward,
            self.document.tolerance(),
        )
    }
}
