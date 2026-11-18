//! Standalone curve/source, endpoint and numeric-confirmation phases.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_document::ObjectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Temporary {
    Offset,
    Radius,
}

#[derive(Clone, Debug)]
pub(super) struct SubcurvePrompt {
    pub(super) source: Option<ObjectId>,
    pub(super) edge: Option<usize>,
    edge_source: Option<(
        viboceros_document::GeometrySnapshot,
        viboceros_geometry::Tolerance,
    )>,
    pub(super) start: Option<f64>,
    pub(super) length: Option<f64>,
    pub(super) copy: bool,
    pub(super) delete: bool,
    pub(super) temporary: Option<Temporary>,
    temporary_reference: Option<CapturedCurve>,
    pub(super) mode: viboceros_command::subcurve_input::SubcurveMode,
    pub(super) from_midpoint: bool,
    pub(super) hover_parameter: Option<f64>,
    pub(super) locked_forward: Option<bool>,
    pub(super) preview: super::subcurve_preview::Cache,
}
impl SubcurvePrompt {
    fn new(
        copy: bool,
        mode: viboceros_command::subcurve_input::SubcurveMode,
        from_midpoint: bool,
        delete: bool,
    ) -> Self {
        Self {
            source: None,
            edge: None,
            edge_source: None,
            start: None,
            length: None,
            copy,
            delete,
            temporary: None,
            temporary_reference: None,
            mode,
            from_midpoint,
            hover_parameter: None,
            locked_forward: None,
            preview: Default::default(),
        }
    }

    pub(super) fn source_current(&self, document: &Document) -> bool {
        if let Some(reference) = &self.temporary_reference {
            return reference.current(document);
        }
        self.edge_source
            .as_ref()
            .is_none_or(|(snapshot, tolerance)| {
                *tolerance == document.tolerance()
                    && self.source.is_some_and(|id| {
                        document.is_object_selectable(id)
                            && document.object(id).is_some_and(|o| {
                                snapshot.shares_storage_with(o.geometry_snapshot())
                            })
                    })
            })
    }

    pub(super) fn temporary_reference(&self) -> Option<&CapturedCurve> {
        self.temporary_reference.as_ref()
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.temporary.is_some() && self.source.is_none() {
            return "Select a curve or surface edge to temporarily shorten";
        }
        if self.delete {
            return match (self.source, self.start, self.length) {
                (None, _, _) => "DeleteSubCrv: select one curve; Esc cancels",
                (Some(_), None, _) => {
                    "DeleteSubCrv: pick the start of the portion to delete; Esc cancels"
                }
                (Some(_), Some(_), None) => {
                    "DeleteSubCrv: pick the end or enter a length; Esc cancels"
                }
                _ => "DeleteSubCrv: confirm the length direction; a new number replaces the length",
            };
        }

        if self.from_midpoint && self.source.is_some() {
            return if self.start.is_none() {
                "SubCrv: pick the midpoint on the curve (Esc cancels)"
            } else {
                "SubCrv: pick a symmetric end or enter the half-length (Esc cancels)"
            };
        }
        if self.start.is_some() && self.locked_forward.is_some() {
            return "SubCrv: pick the locked-side end or enter a length; Direction=Free unlocks";
        }
        match (self.source, self.start, self.length) {
            (None, _, _) => "SubCrv: select one curve or surface edge; Esc cancels",
            (Some(_), None, _) => "SubCrv: pick the subcurve start (Esc cancels)",
            (Some(_), Some(_), None) => {
                "SubCrv: pick the directed subcurve end or type a length (Esc cancels)"
            }
            (Some(_), Some(_), Some(_)) => {
                "SubCrv: confirm the length direction; a new number replaces the length"
            }
        }
    }
}
impl VibocerosApp {
    pub(super) fn validate_subcurve_source(&mut self) -> bool {
        if self
            .subcurve_prompt
            .as_ref()
            .is_none_or(|p| p.source_current(&self.document))
        {
            return true;
        }
        let p = self.subcurve_prompt.as_mut().unwrap();
        p.source = None;
        p.edge = None;
        p.edge_source = None;
        p.temporary_reference = None;
        p.start = None;
        p.length = None;
        p.hover_parameter = None;
        p.locked_forward = None;
        p.preview = Default::default();
        self.active_command = None;
        let message = if p.delete || p.temporary.is_some() {
            "Source curve changed; select the source again"
        } else {
            "Surface edge changed; select the source again"
        };
        self.push_log(message.into());
        false
    }

    pub(super) fn picking_subcurve_edge(&self) -> bool {
        self.subcurve_prompt
            .as_ref()
            .is_some_and(|p| !p.delete && p.source.is_none())
            || self.picking_curve_offset_edges()
            || self.picking_uv_subcurve_edge()
            || self.picking_curve_boolean_subcurve_edge()
    }

    pub(super) fn pick_subcurve_edge(&mut self, pick: crate::viewport::EdgePick) -> bool {
        if self.picking_curve_offset_edges() {
            self.pick_curve_offset_edges(
                [pick],
                viboceros_command::component_selection::ComponentAction::Add,
            );
            return true;
        }
        if self.picking_curve_boolean_subcurve_edge() {
            return self.pick_curve_boolean_subcurve(pick.object, Some(pick.edge));
        }
        if self.picking_uv_subcurve_edge() {
            return self.pick_uv_subcurve_edge(pick);
        }
        if !self.picking_subcurve_edge() {
            return false;
        }
        let valid = self.document.is_object_selectable(pick.object)
            && self.document.object(pick.object).is_some_and(|o| {
                viboceros_command::curve_reference::resolve(
                    o.geometry(),
                    Some(pick.edge),
                    self.document.tolerance(),
                )
                .is_some()
            });
        if !valid {
            self.push_log("Select an available surface edge".into());
            return true;
        }
        let p = self.subcurve_prompt.as_mut().unwrap();
        p.source = Some(pick.object);
        p.edge = Some(pick.edge);
        if p.temporary.is_some() {
            match CapturedCurve::capture(&self.document, pick.object, Some(pick.edge)) {
                Ok(reference) => p.temporary_reference = Some(reference),
                Err(error) => {
                    p.source = None;
                    p.edge = None;
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            }
        } else {
            p.edge_source = Some((
                self.document
                    .object(pick.object)
                    .unwrap()
                    .geometry_snapshot()
                    .clone(),
                self.document.tolerance(),
            ));
        }
        self.active_command = Some(InteractiveCommand::SubCrv {
            start: None,
            copy: p.copy,
            delete: p.delete,
        });
        if p.temporary.is_none() {
            self.document.clear_selection();
            self.component_selection.clear();
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
        true
    }
    pub(in crate::app) fn begin_offset_subcurve_prompt(
        &mut self,
        reference: Option<(ObjectId, Option<usize>)>,
    ) {
        self.begin_temporary_subcurve_prompt(reference, Temporary::Offset);
    }

    pub(super) fn begin_radius_subcurve_prompt(
        &mut self,
        reference: Option<(ObjectId, Option<usize>)>,
    ) {
        self.begin_temporary_subcurve_prompt(reference, Temporary::Radius);
    }

    fn begin_temporary_subcurve_prompt(
        &mut self,
        reference: Option<(ObjectId, Option<usize>)>,
        parent: Temporary,
    ) {
        self.subcurve_prompt = Some(SubcurvePrompt {
            temporary: Some(parent),
            ..SubcurvePrompt::new(true, Default::default(), false, false)
        });
        self.point_filter = None;
        self.point_constraint = None;
        self.active_command = None;
        self.command_input.clear();
        match reference {
            Some((object, Some(edge))) => {
                self.pick_subcurve_edge(crate::viewport::EdgePick { object, edge });
            }
            Some((object, None)) => {
                self.pick_subcurve_source(Some(object));
            }
            None => self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into()),
        }
    }

    pub(super) fn begin_subcurve_prompt(
        &mut self,
        copy: bool,
        mode: viboceros_command::subcurve_input::SubcurveMode,
        from_midpoint: bool,
        delete: bool,
    ) {
        let sources = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let source = if let [id] = sources.as_slice() {
            Some(*id)
        } else {
            None
        };
        let mut prompt = SubcurvePrompt::new(copy, mode, from_midpoint, delete);
        prompt.source = source;
        if delete {
            prompt.edge_source = source.map(|id| {
                (
                    self.document
                        .object(id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone(),
                    self.document.tolerance(),
                )
            });
        }
        self.subcurve_prompt = Some(prompt);
        self.active_command = source.map(|_| InteractiveCommand::SubCrv {
            start: None,
            copy,
            delete,
        });
        if source.is_none() {
            self.document.clear_selection();
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
    }
    pub(super) fn pick_subcurve_source(&mut self, id: Option<ObjectId>) -> bool {
        let Some(prompt) = self.subcurve_prompt.as_mut() else {
            return false;
        };
        if prompt.source.is_some() {
            return false;
        }
        if let Some(id) = id.filter(|id| {
            self.document.is_object_selectable(*id)
                && self
                    .document
                    .object(*id)
                    .is_some_and(|o| o.geometry().curve_ref().is_some())
        }) {
            prompt.source = Some(id);
            if prompt.temporary.is_some() {
                match CapturedCurve::capture(&self.document, id, None) {
                    Ok(reference) => prompt.temporary_reference = Some(reference),
                    Err(error) => {
                        prompt.source = None;
                        self.push_log(format!("Error: {error}"));
                        return true;
                    }
                }
            } else if prompt.delete {
                prompt.edge_source = Some((
                    self.document
                        .object(id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone(),
                    self.document.tolerance(),
                ));
            }
            self.active_command = Some(InteractiveCommand::SubCrv {
                start: None,
                copy: prompt.copy,
                delete: prompt.delete,
            });
            if prompt.temporary.is_none() {
                let _ = self.document.select_command_results([id]);
            }
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
        true
    }
    pub(super) fn continue_subcurve_prompt(&mut self, input: &str) -> bool {
        if self.subcurve_prompt.is_some() && self.try_continue_component_choice(input) {
            return true;
        }
        if !self.validate_subcurve_source() {
            return true;
        }
        let Some(prompt) = self.subcurve_prompt.clone() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-', '\'']);
        if let Some(parent) = prompt.temporary {
            if word.eq_ignore_ascii_case("Cancel") {
                self.cancel_interactive_command(true);
                return true;
            }
            if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
                self.complete_temporary_subcurve(parent, None)
                    .expect("empty temporary portion");
                return true;
            }
            if prompt.source.is_none() {
                let id = if word.eq_ignore_ascii_case("SelLast") {
                    self.document
                        .objects()
                        .filter(|object| object.geometry().curve_ref().is_some())
                        .filter(|object| self.document.is_object_selectable(object.id()))
                        .last()
                        .map(|object| object.id())
                } else if let Some((key, value)) = word.split_once(char::is_whitespace)
                    && key.eq_ignore_ascii_case("SelID")
                {
                    value.trim().parse().ok()
                } else {
                    None
                };
                if let Some(id) = id {
                    self.pick_subcurve_source(Some(id));
                    return true;
                }
            }
        }
        if input.is_empty() {
            self.cancel_interactive_command(true);
            return true;
        }
        if !prompt.delete
            && prompt.temporary.is_none()
            && let Some((name, value)) = input.trim_start_matches('_').split_once('=')
            && name.eq_ignore_ascii_case("FromMidpoint")
        {
            let flag = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                Some(true)
            } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                Some(false)
            } else {
                None
            };
            if let Some(flag) = flag {
                self.subcurve_prompt.as_mut().unwrap().from_midpoint = flag;
                self.commands.set_subcurve_options(None, None, Some(flag));
                self.command_input.clear();
                self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
            } else {
                self.push_log("FromMidpoint expects Yes or No".into());
            }
            return true;
        }
        if !prompt.delete
            && prompt.temporary.is_none()
            && let Some((name, value)) = input.trim_start_matches('_').split_once('=')
            && name.eq_ignore_ascii_case("Mode")
        {
            if let Some(mode) = viboceros_command::subcurve_input::SubcurveMode::parse(value) {
                self.subcurve_prompt.as_mut().unwrap().mode = mode;
                self.commands.set_subcurve_options(None, Some(mode), None);
                self.command_input.clear();
                self.push_log(format!("SubCrv Mode={}", mode.option()));
            } else {
                self.push_log("Mode expects Shorten or MarkEnds".into());
            }
            return true;
        }
        if !prompt.delete
            && prompt.temporary.is_none()
            && let Some((name, value)) = input.trim_start_matches('_').split_once('=')
            && name.eq_ignore_ascii_case("Copy")
        {
            let copy = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                Some(true)
            } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                Some(false)
            } else {
                None
            };
            if let Some(copy) = copy {
                self.subcurve_prompt.as_mut().unwrap().copy = copy;
                self.commands.set_subcurve_options(Some(copy), None, None);
                if let Some(InteractiveCommand::SubCrv { start, .. }) = self.active_command {
                    self.active_command = Some(InteractiveCommand::SubCrv {
                        start,
                        copy,
                        delete: false,
                    });
                }
                self.command_input.clear();
                self.push_log(format!("SubCrv Copy={}", if copy { "Yes" } else { "No" }));
            } else {
                self.push_log("Copy expects Yes or No".into());
            }
            return true;
        }
        if prompt.source.is_none() {
            if let Some((name, value)) = input.trim_start_matches('_').split_once('=')
                && name.eq_ignore_ascii_case("Edge")
                && let Some((object, edge)) = value.split_once(',')
                && let (Ok(object), Ok(edge)) = (object.parse(), edge.parse())
            {
                self.pick_subcurve_edge(crate::viewport::EdgePick { object, edge });
                self.command_input.clear();
                return true;
            }
            if let Ok(id) = input.parse::<ObjectId>() {
                self.pick_subcurve_source(Some(id));
                self.command_input.clear();
                return true;
            }
        } else if prompt.start.is_some()
            && let Some(source) = prompt.source
            && let Some(quantity) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            )
        {
            match quantity {
                Ok(0.) => {
                    if let Some(parent) = prompt.temporary {
                        self.complete_temporary_subcurve(parent, None)
                            .expect("empty temporary portion");
                    } else {
                        self.cancel_interactive_command(true);
                    }
                }
                Ok(length) => {
                    let total = self
                        .document
                        .object(source)
                        .and_then(|o| {
                            viboceros_command::curve_reference::resolve(
                                o.geometry(),
                                prompt.edge,
                                self.document.tolerance(),
                            )
                        })
                        .map(|c| c.curve().length(self.document.tolerance()));
                    match total {
                        Some(Ok(total))
                            if length.abs()
                                <= total
                                    + if prompt.delete {
                                        self.document.tolerance().absolute()
                                    } else {
                                        0.
                                    } =>
                        {
                            self.subcurve_prompt.as_mut().unwrap().length = Some(length.abs());
                            self.command_input.clear();
                            if !prompt.from_midpoint
                                && let Some(forward) = prompt.locked_forward
                            {
                                let resolved = viboceros_command::curve_reference::resolve(
                                    self.document.object(source).unwrap().geometry(),
                                    prompt.edge,
                                    self.document.tolerance(),
                                )
                                .unwrap();
                                let curve = resolved.curve();
                                match viboceros_command::subcurve_input::locked_numeric_requires_confirmation(
                                    curve, prompt.start.unwrap(), length.abs(), forward, self.document.tolerance(),
                                ) {
                                    Ok(true) => {
                                        self.subcurve_prompt.as_mut().unwrap().locked_forward = None;
                                        if prompt.temporary.is_none() {
                                            self.document.clear_selection();
                                        }
                                        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
                                        return true;
                                    }
                                    Ok(false) => {}
                                    Err(error) => {
                                        self.push_log(format!("Error: {error}"));
                                        return true;
                                    }
                                }
                            }
                            if prompt.from_midpoint || prompt.locked_forward.is_some() {
                                let point = viboceros_command::curve_reference::resolve(
                                    self.document.object(source).unwrap().geometry(),
                                    prompt.edge,
                                    self.document.tolerance(),
                                )
                                .unwrap()
                                .curve()
                                .evaluate(prompt.start.unwrap());
                                match point {
                                    Ok(point) => {
                                        self.accept_standalone_subcurve_point(point);
                                    }
                                    Err(error) => self.push_log(format!("Error: {error}")),
                                }
                                return true;
                            }
                            self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
                        }
                        Some(Err(error)) => self.push_log(format!("Error: {error}")),
                        _ => self.push_log("Curve length exceeds the whole source curve".into()),
                    }
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        } else if self.try_continue_point_constraint(input)
            || self.try_continue_point_filter(input)
            || self.try_continue_point_input(input)
        {
            return true;
        }
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_interactive_command(false);
            return false;
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
        self.command_input.clear();
        true
    }
    fn complete_temporary_subcurve(
        &mut self,
        parent: Temporary,
        portion: Option<(CapturedCurve, viboceros_geometry::Curve3)>,
    ) -> Result<(), viboceros_command::CommandError> {
        match parent {
            Temporary::Radius => self.return_to_radius(portion)?,
            Temporary::Offset => {
                if let Some((reference, curve)) = portion {
                    self.add_curve_offset_portion(&reference, curve);
                } else {
                    self.return_to_curve_offset_sources();
                }
            }
        }
        Ok(())
    }

    pub(super) fn accept_temporary_subcurve_parameter(&mut self, parameter: f64) -> bool {
        let Some(reference) = self
            .subcurve_prompt
            .as_ref()
            .and_then(SubcurvePrompt::temporary_reference)
        else {
            return false;
        };
        let result = reference
            .curve_ref()
            .parameter_from_nurbs(parameter)
            .and_then(|t| Ok((reference.curve_ref().evaluate(t)?, t)));
        match result {
            Ok((point, t)) => {
                self.accept_subcurve_input(point, Some(t));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn accept_standalone_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        self.accept_subcurve_input(point, None)
    }

    fn accept_subcurve_input(&mut self, point: Point3, parameter: Option<f64>) -> Option<bool> {
        if !self.validate_subcurve_source() {
            return Some(false);
        }
        let mut prompt = self.subcurve_prompt.clone()?;
        let source = prompt.source?;
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            let resolved = self
                .document
                .object(source)
                .and_then(|o| {
                    viboceros_command::curve_reference::resolve(
                        o.geometry(),
                        prompt.edge,
                        self.document.tolerance(),
                    )
                })
                .ok_or(viboceros_command::CommandError::Usage(
                    "Select an existing curve",
                ))?;
            let curve = resolved.curve();
            let parameter = parameter.map_or_else(
                || curve.closest_parameter(point, self.document.tolerance()),
                Ok,
            )?;
            let Some(start) = prompt.start else {
                prompt.start = Some(parameter);
                self.subcurve_prompt = Some(prompt.clone());
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy: prompt.copy,
                    delete: prompt.delete,
                });
                self.push_log(prompt.hint().into());
                return Ok(());
            };
            if let Some(parent) = prompt.temporary {
                use viboceros_command::{curve_reference, subcurve_input};
                let tolerance = self.document.tolerance();
                let piece = if let Some(length) = prompt.length {
                    if let Some(forward) = prompt.locked_forward {
                        subcurve_input::locked_piece(curve, start, length, forward, tolerance)?
                    } else {
                        subcurve_input::piece(curve, start, parameter, length, tolerance)?
                    }
                } else if start == parameter {
                    None
                } else {
                    subcurve_input::point_parameters(
                        curve,
                        start,
                        parameter,
                        prompt.locked_forward,
                        false,
                    )?
                    .map(|range| {
                        curve_reference::prepare_subcurve(
                            curve,
                            range,
                            prompt.edge.is_none() && curve.is_closed().unwrap_or(false),
                        )
                    })
                    .transpose()?
                };
                let portion = piece.map(|piece| {
                    let reference = prompt
                        .temporary_reference
                        .clone()
                        .expect("temporary input has a curve");
                    (reference, piece)
                });
                self.complete_temporary_subcurve(parent, portion)?;
                return Ok(());
            }
            let name = if prompt.delete {
                "DeleteSubCrv"
            } else {
                "SubCrv"
            };
            let mut input = if let Some(length) = prompt.length {
                format!("{name} Numeric={start},{length},{parameter}")
            } else {
                let start_point = curve.evaluate(start)?;
                format!(
                    "{name} {} {}",
                    format_model_point(start_point),
                    format_model_point(point)
                )
            };
            if !prompt.delete {
                input.push_str(&format!(
                    " Copy={} Mode={} FromMidpoint={}",
                    if prompt.copy { "Yes" } else { "No" },
                    prompt.mode.option(),
                    if prompt.from_midpoint { "Yes" } else { "No" }
                ));
            }
            if let Some(forward) = prompt.locked_forward {
                input.push_str(&format!(
                    " Locked={}",
                    if forward { "Forward" } else { "Backward" }
                ));
            }
            if let Some(edge) = prompt.edge {
                input.push_str(&format!(" Edge={source},{edge}"));
            }
            self.document.select_command_results([source])?;
            self.commands.execute(&mut self.document, &input)?;
            self.subcurve_prompt = None;
            self.active_command = None;
            self.drafting_plane = None;
            self.push_log(format!("> {input}"));
            Ok(())
        })();
        match result {
            Ok(()) => Some(true),
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                Some(false)
            }
        }
    }
}
