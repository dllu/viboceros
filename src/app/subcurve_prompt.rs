//! Standalone curve/source, endpoint and numeric-confirmation phases.
use super::*;
use viboceros_document::ObjectId;

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
    pub(super) mode: viboceros_command::subcurve_input::SubcurveMode,
    pub(super) from_midpoint: bool,
    pub(super) hover_parameter: Option<f64>,
    pub(super) locked_forward: Option<bool>,
    pub(super) preview: super::subcurve_preview::Cache,
}
impl SubcurvePrompt {
    pub(super) fn source_current(&self, document: &Document) -> bool {
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

    pub(super) fn hint(&self) -> &'static str {
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
        p.start = None;
        p.length = None;
        p.hover_parameter = None;
        p.locked_forward = None;
        p.preview = Default::default();
        self.active_command = None;
        self.push_log("Surface edge changed; select the source again".into());
        false
    }

    pub(super) fn picking_subcurve_edge(&self) -> bool {
        self.subcurve_prompt
            .as_ref()
            .is_some_and(|p| p.source.is_none())
    }

    pub(super) fn pick_subcurve_edge(&mut self, pick: crate::viewport::EdgePick) -> bool {
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
        p.edge_source = Some((
            self.document
                .object(pick.object)
                .unwrap()
                .geometry_snapshot()
                .clone(),
            self.document.tolerance(),
        ));
        self.active_command = Some(InteractiveCommand::SubCrv {
            start: None,
            copy: p.copy,
        });
        self.document.clear_selection();
        self.component_selection.clear();
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
        true
    }
    pub(super) fn begin_subcurve_prompt(
        &mut self,
        copy: bool,
        mode: viboceros_command::subcurve_input::SubcurveMode,
        from_midpoint: bool,
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
        self.subcurve_prompt = Some(SubcurvePrompt {
            source,
            edge: None,
            edge_source: None,
            start: None,
            length: None,
            copy,
            mode,
            from_midpoint,
            hover_parameter: None,
            locked_forward: None,
            preview: super::subcurve_preview::Cache::default(),
        });
        self.active_command = source.map(|_| InteractiveCommand::SubCrv { start: None, copy });
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
            self.active_command = Some(InteractiveCommand::SubCrv {
                start: None,
                copy: prompt.copy,
            });
            let _ = self.document.select_command_results([id]);
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
        if input.is_empty() {
            self.cancel_interactive_command(true);
            return true;
        }
        if let Some((name, value)) = input.trim_start_matches('_').split_once('=')
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
        if let Some((name, value)) = input.trim_start_matches('_').split_once('=')
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
        if let Some((name, value)) = input.trim_start_matches('_').split_once('=')
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
                    self.active_command = Some(InteractiveCommand::SubCrv { start, copy });
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
                Ok(0.) => self.cancel_interactive_command(true),
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
                        Some(Ok(total)) if length.abs() <= total => {
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
                                        self.document.clear_selection();
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
    pub(super) fn accept_standalone_subcurve_point(&mut self, point: Point3) -> Option<bool> {
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
            let parameter = curve.closest_parameter(point, self.document.tolerance())?;
            let Some(start) = prompt.start else {
                prompt.start = Some(parameter);
                self.subcurve_prompt = Some(prompt.clone());
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy: prompt.copy,
                });
                self.push_log(prompt.hint().into());
                return Ok(());
            };
            let mut input = if let Some(length) = prompt.length {
                format!(
                    "SubCrv Numeric={start},{length},{parameter} Copy={}",
                    if prompt.copy { "Yes" } else { "No" }
                )
            } else {
                // Use point syntax so standalone orientation follows the public
                // command policy, while Parameter= retains directed math edits.
                let start_point = curve.evaluate(start)?;
                format!(
                    "SubCrv {} {} Copy={}",
                    format_model_point(start_point),
                    format_model_point(point),
                    if prompt.copy { "Yes" } else { "No" }
                )
            };
            input.push_str(&format!(" Mode={}", prompt.mode.option()));
            input.push_str(&format!(
                " FromMidpoint={}",
                if prompt.from_midpoint { "Yes" } else { "No" }
            ));
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
