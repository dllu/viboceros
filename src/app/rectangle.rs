//! Rectangle and FlowAlongSrf Plane share an edit-free point getter.
use super::*;
use viboceros_command::{
    curve_reference::CapturedCurve,
    rectangle_input::{RectangleDefinition, RectangleInput},
};

pub(super) struct InputSession {
    pub(super) input: RectangleInput,
    curve: Option<CapturedCurve>,
}
impl InputSession {
    pub(super) fn new(frame: Frame3) -> Self {
        Self {
            input: RectangleInput::new(frame),
            curve: None,
        }
    }
    fn picking_curve(&self) -> bool {
        self.input.requests_curve() && self.curve.is_none()
    }
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.curve.as_ref().map(CapturedCurve::curve)
    }
    fn point(
        &mut self,
        point: Point3,
    ) -> Result<Option<RectangleDefinition>, viboceros_geometry::GeometryError> {
        if let Some(curve) = &self.curve {
            let frame = RectangleInput::curve_frame(curve.curve_ref(), point)?;
            self.input.around_curve(frame);
            self.curve = None;
            Ok(None)
        } else {
            self.input.point(point)
        }
    }
}
impl VibocerosApp {
    pub(super) fn rectangle_getter(&self) -> Option<&InputSession> {
        self.rectangle_session
            .as_ref()
            .or_else(|| self.surface_flow_session.as_ref()?.plane.as_ref())
    }
    fn rectangle_getter_mut(&mut self) -> Option<&mut InputSession> {
        if self.rectangle_session.is_some() {
            self.rectangle_session.as_mut()
        } else {
            self.surface_flow_session.as_mut()?.plane.as_mut()
        }
    }
    pub(super) fn picking_rectangle_curve(&self) -> bool {
        self.rectangle_getter()
            .is_some_and(InputSession::picking_curve)
    }
    pub(super) fn pick_rectangle_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_rectangle_curve() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a curve or surface edge".to_owned())
            .and_then(|id| {
                CapturedCurve::capture(&self.document, id, edge).map_err(|e| e.to_string())
            });
        match result {
            Ok(curve) => {
                if let Some(session) = &mut self.surface_flow_session {
                    session.postselected = true;
                }
                self.rectangle_getter_mut().unwrap().curve = Some(curve);
                self.component_selection.clear_choices();
                self.rectangle_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn accept_rectangle_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .rectangle_getter()
            .and_then(InputSession::cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(point) = point {
            self.accept_rectangle_point(point);
            true
        } else {
            false
        }
    }
    pub(super) fn validate_rectangle_curve(&mut self) {
        let stale = self
            .rectangle_getter()
            .and_then(|s| s.curve.as_ref())
            .is_some_and(|c| !c.current(&self.document));
        if stale {
            self.rectangle_getter_mut().unwrap().curve = None;
            self.component_selection.clear_choices();
            self.push_log("Rectangle curve changed or became unavailable; select it again".into());
            self.rectangle_prompt();
        }
    }
    fn rectangle_prompt(&mut self) {
        let Some(s) = self.rectangle_getter() else {
            return;
        };
        let anchor = s.input.anchor();
        let frame = s.input.mouse_plane();
        let phase = if s.curve.is_some() {
            "center on curve"
        } else {
            s.input.prompt()
        };
        let name = if self.rectangle_session.is_some() {
            "Rectangle"
        } else {
            "FlowAlongSrf Plane"
        };
        self.push_log(format!("{name}: {phase}; Esc cancels"));
        self.drafting_plane = frame;
        self.active_command = Some(if self.rectangle_session.is_some() {
            InteractiveCommand::Rectangle { first: anchor }
        } else {
            InteractiveCommand::SurfaceFlow { start: anchor }
        });
    }
    fn finish_rectangle(&mut self, definition: RectangleDefinition) -> bool {
        if self.rectangle_session.is_some() {
            // Route the completed construction through the public command so
            // selection, layer attributes and history retain one owner.
            let [a, b, c] = definition.plane_points();
            let command = format!(
                "Rectangle 3Point {} {} {}",
                format_model_point(a),
                format_model_point(b),
                format_model_point(c)
            );
            if let Err(e) = definition.polyline(self.document.tolerance()) {
                self.push_log(format!("Error: {e}"));
                return false;
            }
            self.try_execute_command(&command)
        } else {
            let points = definition.plane_points();
            if let Err(e) = viboceros_command::surface_flow::plane_surface(points) {
                self.push_log(format!("Error: {e}"));
                return false;
            }
            let s = self.surface_flow_session.as_mut().unwrap();
            s.base = Some(viboceros_command::surface_flow::BaseInput::Plane(points));
            s.plane = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.drafting_plane = None;
            self.prepare_surface_flow();
            self.surface_flow_prompt();
            true
        }
    }
    pub(super) fn accept_rectangle_point(&mut self, point: Point3) -> Option<bool> {
        self.rectangle_getter()?;
        self.validate_rectangle_curve();
        if self.picking_rectangle_curve() {
            self.rectangle_prompt();
            return Some(false);
        }
        let frame = self.viewports[self.active_viewport].construction_plane();
        self.rectangle_getter_mut()
            .unwrap()
            .input
            .set_initial_plane(frame);
        let result = self.rectangle_getter_mut().unwrap().point(point);
        Some(match result {
            Ok(Some(definition)) => self.finish_rectangle(definition),
            Ok(None) => {
                self.rectangle_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
    pub(super) fn try_start_rectangle(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            matches!(
                w.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "rectangle" | "rect"
            )
        }) {
            return false;
        }
        let mut getter =
            InputSession::new(self.viewports[self.active_viewport].construction_plane());
        if !words.all(|w| getter.input.option(w)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.rectangle_session = Some(getter);
        self.rectangle_prompt();
        true
    }
    pub(super) fn try_continue_rectangle(&mut self, input: &str) -> bool {
        if self.rectangle_getter().is_none() {
            return false;
        }
        self.validate_rectangle_curve();
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if self.rectangle_getter_mut().unwrap().input.option(word) {
            self.rectangle_getter_mut().unwrap().curve = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.rectangle_prompt();
            return true;
        }
        if self.picking_rectangle_curve() {
            let words = word.split_whitespace().collect::<Vec<_>>();
            let reference = match words.as_slice() {
                [key, id] if key.eq_ignore_ascii_case("SelID") => {
                    id.parse().ok().map(|id| (id, None))
                }
                [one] => one.parse().ok().map(|id| (id, None)).or_else(|| {
                    let (k, v) = one.split_once('=')?;
                    if k.eq_ignore_ascii_case("Edge") {
                        viboceros_command::curve_reference::parse_edge(v)
                            .map(|(id, e)| (id, Some(e)))
                    } else if k.eq_ignore_ascii_case("Curve") {
                        v.parse().ok().map(|id| (id, None))
                    } else {
                        None
                    }
                }),
                _ => None,
            };
            if let Some((id, edge)) = reference {
                return self.pick_rectangle_curve(Some(id), edge);
            }
            self.rectangle_prompt();
            return true;
        }
        let result = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.rectangle_getter_mut().unwrap().input.enter()
        } else if self.rectangle_getter().unwrap().input.anchor().is_none()
            || self.rectangle_getter().unwrap().curve.is_some()
        {
            return false;
        } else if let Some(value) =
            viboceros_drafting::PointInput::parse_length_with_units(input, self.document.units())
        {
            match value {
                Ok(value) => self.rectangle_getter_mut().unwrap().input.number(value),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        } else {
            return false;
        };
        match result {
            Ok(Some(definition)) => {
                self.finish_rectangle(definition);
            }
            Ok(None) => self.rectangle_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
}
