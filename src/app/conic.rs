//! Edit-free Conic getters; only the completed definition enters document history.
use super::*;
use viboceros_command::conic::{Definition, Input, ReferenceInput, ReferenceStage, Slot};

pub(super) struct Session {
    pub(super) input: Input,
    pub(super) reference: Option<ReferenceInput>,
    subcurve: bool,
}
impl Session {
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.reference.as_ref().map(ReferenceInput::cursor)
    }
}
impl VibocerosApp {
    pub(super) fn picking_conic_curve(&self) -> bool {
        self.conic_session
            .as_ref()
            .is_some_and(|s| s.input.direction_request().is_some() && s.reference.is_none())
    }
    pub(super) fn validate_conic_curve(&mut self) {
        if self
            .conic_session
            .as_ref()
            .and_then(|s| s.reference.as_ref())
            .is_some_and(|r| !r.current(&self.document))
        {
            self.conic_session.as_mut().unwrap().reference = None;
            self.component_selection.clear_choices();
            self.push_log("Conic reference changed or became unavailable; select it again".into());
            self.conic_prompt();
        }
    }
    pub(super) fn pick_conic_curve(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_conic_curve() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a curve or surface edge".to_owned())
            .and_then(|id| {
                ReferenceInput::capture(
                    &self.document,
                    id,
                    edge,
                    self.conic_session.as_ref().unwrap().subcurve,
                )
                .map_err(|e| e.to_string())
            });
        match result {
            Ok(reference) => {
                let s = self.conic_session.as_mut().unwrap();
                s.reference = Some(reference);
                s.subcurve = false;
                self.component_selection.clear_choices();
                self.conic_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn resolve_conic_point(
        &self,
        point: Point3,
    ) -> Result<Point3, viboceros_geometry::GeometryError> {
        let Some(s) = &self.conic_session else {
            return Ok(point);
        };
        if let Some(r) = &s.reference {
            return r.sample(point).map(|(point, _)| point);
        }
        s.input
            .point_constraint()
            .map_or(Ok(point), |c| c.resolve(point))
    }
    pub(super) fn accept_conic_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .conic_session
            .as_ref()
            .and_then(Session::cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(p) = point {
            self.accept_drafting_point(p);
            true
        } else {
            false
        }
    }
    fn conic_prompt(&mut self) {
        let Some(s) = &self.conic_session else {
            return;
        };
        let phase = if self.picking_conic_curve() {
            "select curve or surface edge; SubCrv".to_owned()
        } else if let Some(r) = &s.reference {
            match r.stage() {
                ReferenceStage::TrimStart => "start of temporary subcurve; FullCurve".into(),
                ReferenceStage::TrimEnd => "end of temporary subcurve or length; FullCurve".into(),
                ReferenceStage::Endpoint => "point on reference curve".into(),
            }
        } else {
            match s.input.slot() {
                Slot::Start => "start; Tangent, Perpendicular".into(),
                Slot::End => {
                    if s.input.point_constraint().is_some() || s.input.markers().len() == 2 {
                        "end; Vertical".into()
                    } else {
                        "end; Apex, Tangent, Perpendicular".into()
                    }
                }
                Slot::Apex => "apex".into(),
                Slot::Shape => s.input.options().rho.map_or_else(
                    || "curvature point or rho (0 < rho < 1)".into(),
                    |r| format!("curvature point or rho <{r}>"),
                ),
            }
        };
        self.active_command = Some(InteractiveCommand::Conic {
            anchor: s.input.anchor(),
        });
        self.drafting_plane = s.input.mouse_plane();
        self.push_log(format!("Conic: {phase}; Esc cancels"));
    }
    fn finish_conic(&mut self, definition: Definition) -> bool {
        let [start, apex, end] = definition.controls;
        self.try_execute_command(&format!(
            "Conic {} {} {} {}",
            format_model_point(start),
            format_model_point(end),
            format_model_point(apex),
            definition.rho
        ))
    }
    pub(super) fn accept_conic_point(&mut self, point: Point3) -> Option<bool> {
        self.conic_session.as_ref()?;
        self.validate_conic_curve();
        if self.picking_conic_curve() {
            self.conic_prompt();
            return Some(false);
        }
        let normal = self.viewports[self.active_viewport]
            .construction_plane()
            .z_axis();
        let frame = self.viewports[self.active_viewport].construction_plane();
        let s = self.conic_session.as_mut().unwrap();
        s.input.set_initial_plane(frame);
        let trimming = s
            .reference
            .as_ref()
            .is_some_and(|r| r.stage() != ReferenceStage::Endpoint);
        let result = if let Some(r) = &mut s.reference {
            if r.stage() != ReferenceStage::Endpoint {
                r.trim_point(point, self.document.tolerance())
                    .map(|()| None)
                    .map_err(|e| e.to_string())
            } else {
                r.sample(point)
                    .and_then(|(p, t)| s.input.endpoint(p, t, normal))
                    .map(|()| {
                        s.reference = None;
                        None
                    })
                    .map_err(|e| e.to_string())
            }
        } else {
            s.input.point(point).map_err(|e| e.to_string())
        };
        Some(match result {
            Ok(Some(definition)) => self.finish_conic(definition),
            Ok(None) => {
                self.conic_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                if trimming {
                    self.conic_prompt();
                } else {
                    self.cancel_interactive_command(false);
                }
                false
            }
        })
    }
    pub(super) fn try_start_conic(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Conic")
        }) {
            return false;
        }
        let mut getter = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.conic_options(),
            self.document.tolerance(),
        );
        if !words.all(|w| getter.option(w)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.conic_session = Some(Session {
            input: getter,
            reference: None,
            subcurve: false,
        });
        self.conic_prompt();
        true
    }
    pub(super) fn try_continue_conic(&mut self, input: &str) -> bool {
        if self.conic_session.is_none() {
            return false;
        }
        self.validate_conic_curve();
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.conic_session.as_mut().unwrap();
        if s.input.option(word) {
            s.reference = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.conic_prompt();
            return true;
        }
        if self.picking_conic_curve() {
            if word.eq_ignore_ascii_case("SubCrv") {
                self.conic_session.as_mut().unwrap().subcurve = true;
                self.conic_prompt();
                return true;
            }
            let words = word.split_whitespace().collect::<Vec<_>>();
            let pick = match words.as_slice() {
                [key, id] if key.eq_ignore_ascii_case("SelID") => {
                    id.parse().ok().map(|id| (id, None))
                }
                [one] => one.parse().ok().map(|id| (id, None)).or_else(|| {
                    let (k, v) = one.split_once('=')?;
                    if k.eq_ignore_ascii_case("Curve") {
                        v.parse().ok().map(|id| (id, None))
                    } else if k.eq_ignore_ascii_case("Edge") {
                        viboceros_command::curve_reference::parse_edge(v)
                            .map(|(id, e)| (id, Some(e)))
                    } else {
                        None
                    }
                }),
                _ => None,
            };
            if let Some((id, edge)) = pick {
                return self.pick_conic_curve(Some(id), edge);
            }
            if self
                .commands
                .recognizes(word.split_whitespace().next().unwrap_or(""))
            {
                return false;
            }
            self.conic_prompt();
            return true;
        }
        let s = self.conic_session.as_mut().unwrap();
        if let Some(r) = &mut s.reference {
            if word.is_empty()
                || word.eq_ignore_ascii_case("Enter")
                || word.eq_ignore_ascii_case("FullCurve")
            {
                r.full_curve();
                self.conic_prompt();
                return true;
            }
            if r.stage() == ReferenceStage::TrimEnd
                && let Some(n) = viboceros_drafting::PointInput::parse_length_with_units(
                    input,
                    self.document.units(),
                )
            {
                match n.map_err(|e| e.to_string()).and_then(|n| {
                    r.length(n, self.document.tolerance())
                        .map_err(|e| e.to_string())
                }) {
                    Ok(()) => self.conic_prompt(),
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
                return true;
            }
            return false;
        }
        if s.input.slot() != Slot::Shape {
            return false;
        }
        let result = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.input.options().rho.is_none() {
                self.cancel_interactive_command(true);
                return true;
            }
            s.input.enter()
        } else if let Some(n) = viboceros_drafting::PointInput::parse_length_with_units(
            input,
            &viboceros_geometry::LengthUnitSystem::Unset,
        ) {
            match n {
                Ok(n) => s.input.number(n),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        } else {
            return false;
        };
        match result {
            Ok(Some(d)) => {
                self.finish_conic(d);
            }
            Ok(None) => self.conic_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
}
