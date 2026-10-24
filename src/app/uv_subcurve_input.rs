//! Nested, nonmutating SubCrv input for curve-mapping source getters.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_document::ObjectId;
mod edges;
mod length_confirmation;

#[derive(Clone, Debug)]
pub(super) struct CurveRange {
    pub(super) object: ObjectId,
    pub(super) parameters: [f64; 2],
    pub(super) source: viboceros_document::GeometrySnapshot,
    pub(super) edge_reference: Option<CapturedCurve>,
    pub(super) edge_wrap: bool,
}

impl CurveRange {
    pub(super) fn current(&self, document: &Document) -> bool {
        self.edge_reference.as_ref().map_or_else(
            || {
                document.is_object_selectable(self.object)
                    && document
                        .object(self.object)
                        .is_some_and(|o| o.geometry_snapshot().shares_storage_with(&self.source))
            },
            |reference| reference.current(document),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct SubcurveInputs {
    pub(super) ranges: Vec<CurveRange>,
    pub(super) pending: Option<PendingSubcurve>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PendingSubcurve {
    pub(super) object: Option<ObjectId>,
    pub(super) source: Option<viboceros_document::GeometrySnapshot>,
    pub(super) edge_reference: Option<CapturedCurve>,
    pub(super) start: Option<f64>,
    pub(super) length: Option<f64>,
    pub(super) hover_parameter: Option<f64>,
    pub(super) locked_forward: Option<bool>,
    pub(super) preview: super::subcurve_preview::Cache,
}

impl PendingSubcurve {
    pub(super) fn edge(&self) -> Option<usize> {
        self.edge_reference.as_ref().and_then(|r| r.source().1)
    }
    fn curve<'a>(&'a self, document: &'a Document) -> Option<viboceros_geometry::CurveRef<'a>> {
        self.edge_reference
            .as_ref()
            .map(|r| r.curve_ref())
            .or_else(|| {
                document
                    .object(self.object?)
                    .and_then(|o| o.geometry().curve_ref())
            })
    }
    pub(super) fn current(&self, document: &Document) -> bool {
        if let Some(reference) = &self.edge_reference {
            return reference.current(document);
        }
        self.object.is_some_and(|id| {
            document.is_object_selectable(id)
                && document.object(id).is_some_and(|o| {
                    self.source
                        .as_ref()
                        .is_some_and(|s| s.shares_storage_with(o.geometry_snapshot()))
                })
        })
    }
}

impl SubcurveInputs {
    pub(super) fn arguments(&self) -> String {
        self.ranges
            .iter()
            .map(|r| {
                if let Some(edge) = r.edge_reference.as_ref().and_then(|r| r.source().1) {
                    return format!(
                        " SubCrv={},{},{},{},{}",
                        r.object,
                        if r.edge_wrap { "EdgeWrap" } else { "Edge" },
                        edge,
                        r.parameters[0],
                        r.parameters[1]
                    );
                }
                format!(
                    " SubCrv={},{},{}",
                    r.object, r.parameters[0], r.parameters[1]
                )
            })
            .collect()
    }
    pub(super) fn hint(&self) -> Option<&'static str> {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.start.is_some() && p.locked_forward.is_some())
        {
            return Some("Pick the locked-side end or enter a length; Direction=Free unlocks");
        }
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

    pub(super) fn accept_uv_subcurve_point(&mut self, point: Point3) -> Option<bool> {
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
                        length_confirmation::interval(
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
