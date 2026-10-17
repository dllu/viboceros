//! Ellipse construction uses the same edit-free getter for previews and commits.
use super::*;
use viboceros_command::{
    curve_reference::CapturedCurve,
    ellipse_input::{Input, Mode, Slot},
};

pub(super) struct Session {
    pub(super) input: Input,
    pub(super) cursor: Option<Point3>,
    curve: Option<CapturedCurve>,
    postselected: bool,
}
impl Session {
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.input
            .requests_curve()
            .then_some(self.curve.as_ref())
            .flatten()
            .map(CapturedCurve::curve)
    }
}
impl VibocerosApp {
    pub(super) fn release_ellipse_picks(&mut self) {
        if let Some(s) = &self.ellipse_session
            && s.postselected
            && let Some(curve) = &s.curve
            && self.document.is_selected(curve.object_id())
        {
            let _ = self
                .document
                .select_object(curve.object_id(), SelectionMode::Remove);
        }
    }
    pub(super) fn picking_ellipse_curve(&self) -> bool {
        self.ellipse_session
            .as_ref()
            .is_some_and(|s| s.input.requests_curve() && s.curve.is_none())
    }
    fn bind_ellipse_preselection(&mut self) {
        if !self.picking_ellipse_curve() {
            return;
        }
        let selected = self.document.selected_object_ids().collect::<Vec<_>>();
        if let [id] = selected.as_slice()
            && self
                .document
                .object(*id)
                .is_some_and(|o| o.geometry().curve_ref().is_some())
            && let Ok(curve) = CapturedCurve::capture(&self.document, *id, None)
        {
            self.ellipse_session.as_mut().unwrap().curve = Some(curve);
        }
    }
    pub(super) fn pick_ellipse_curve(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_ellipse_curve() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a curve or surface edge".to_owned())
            .and_then(|id| {
                CapturedCurve::capture(&self.document, id, edge).map_err(|e| e.to_string())
            });
        match result {
            Ok(curve) => {
                let id = curve.object_id();
                let postselected = edge.is_none() && !self.document.is_selected(id);
                let s = self.ellipse_session.as_mut().unwrap();
                s.curve = Some(curve);
                s.postselected = postselected;
                if edge.is_none() {
                    self.document
                        .select_object(id, SelectionMode::Add)
                        .expect("selectable Ellipse guide");
                }
                self.component_selection.clear_choices();
                self.ellipse_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    pub(super) fn validate_ellipse_curve(&mut self) {
        if self
            .ellipse_session
            .as_ref()
            .and_then(|s| s.curve.as_ref())
            .is_some_and(|c| !c.current(&self.document))
        {
            self.release_ellipse_picks();
            let s = self.ellipse_session.as_mut().unwrap();
            s.curve = None;
            s.cursor = None;
            s.postselected = false;
            let deformable = s.input.deformable();
            s.input = Input::new(
                s.input.options(),
                self.viewports[self.active_viewport].construction_plane(),
                self.document.tolerance(),
            );
            if deformable {
                s.input.option("Deformable").expect("Ellipse output option");
            }
            s.input.option("AroundCurve").expect("first Ellipse option");
            self.point_filter = None;
            self.point_constraint = None;
            self.component_selection.clear_choices();
            self.push_log("Ellipse guide changed or became unavailable; select it again".into());
            self.ellipse_prompt();
        }
    }
    pub(super) fn resolve_ellipse_point(
        &mut self,
        point: Point3,
    ) -> Result<Point3, viboceros_geometry::GeometryError> {
        self.validate_ellipse_curve();
        let Some(s) = &self.ellipse_session else {
            return Ok(point);
        };
        if let Some(curve) = &s.curve
            && s.input.requests_curve()
        {
            return viboceros_command::curve_reference::perpendicular_frame(
                curve.curve_ref(),
                point,
            )
            .map(|frame| frame.origin());
        }
        s.input.resolve_point(point)
    }
    pub(super) fn accept_ellipse_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .ellipse_session
            .as_ref()
            .and_then(Session::cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(point) = point {
            self.accept_filtered_drafting_point(point, false);
            true
        } else {
            false
        }
    }
    pub(super) fn try_start_ellipse(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            ["Ellipse", "Ell"]
                .iter()
                .any(|n| w.trim_start_matches(['_', '-']).eq_ignore_ascii_case(n))
        }) {
            return false;
        }
        let mut input = Input::new(
            self.commands.ellipse_options(),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.ellipse_session = Some(Session {
            input,
            cursor: None,
            curve: None,
            postselected: false,
        });
        self.bind_ellipse_preselection();
        self.ellipse_prompt();
        true
    }
    fn ellipse_prompt(&mut self) {
        let Some(s) = &self.ellipse_session else {
            return;
        };
        let input = s.input;
        let slot = if self.picking_ellipse_curve() {
            "select curve or surface edge"
        } else if s.input.requests_curve() && s.curve.is_some() {
            "center on curve"
        } else {
            match input.slot() {
                Slot::PointCount => "number of points",
                Slot::First => {
                    if input.mode() == Mode::FromFoci {
                        "first focus"
                    } else if input.mode() == Mode::Corner {
                        "first corner"
                    } else if input.mode() == Mode::Diameter {
                        "first axis start"
                    } else {
                        "center"
                    }
                }
                Slot::FirstAxis => "first axis end",
                Slot::SecondAxis => "second axis end",
                Slot::OppositeCorner if input.centered_corner() => "corner",
                Slot::OppositeCorner => "opposite corner",
                Slot::SecondFocus => "second focus",
                Slot::ThroughPoint => "point on ellipse",
                Slot::Complete => "complete",
            }
        };
        self.active_command = Some(InteractiveCommand::Ellipse {
            center: input.anchor(),
            first_axis: input.reference(),
        });
        self.drafting_plane = Some(input.mouse_plane());
        let options = if input.slot() == Slot::First {
            "; Default, Vertical, Corner, Diameter, FromFoci, AroundCurve"
        } else {
            ""
        };
        let foci = if input.mode() == Mode::FromFoci {
            format!(
                "; MarkFoci={}",
                if input.options().mark_foci {
                    "Yes"
                } else {
                    "No"
                }
            )
        } else {
            String::new()
        };
        let length = input
            .locked_length()
            .map_or(String::new(), |v| format!("; length={v}"));
        let vertical = if input.vertical() && input.mode() == Mode::Diameter {
            " Vertical"
        } else {
            ""
        };
        let output = if input.deformable() {
            format!("; Deformable; PointCount={}", input.options().point_count)
        } else if input.slot() == Slot::First {
            "; Deformable".into()
        } else {
            String::new()
        };
        self.push_log(format!(
            "Ellipse {}{vertical}: {slot}{options}{foci}{length}{output}; Esc cancels",
            input.mode().name()
        ));
    }
    pub(super) fn try_continue_ellipse(&mut self, text: &str) -> bool {
        if self.ellipse_session.is_none() {
            return false;
        }
        self.validate_ellipse_curve();
        let picking = self.picking_ellipse_curve();
        let s = self.ellipse_session.as_mut().unwrap();
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.input.point_count_pending() {
                s.input.accept_point_count_default();
                self.bind_ellipse_preselection();
                self.ellipse_prompt();
            } else if s.input.locked_length().is_some() {
                if let Some(cursor) = s.cursor {
                    self.accept_filtered_drafting_point(cursor, false);
                } else {
                    self.push_log(
                        "Ellipse: move the pointer to choose the locked axis direction".into(),
                    );
                }
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        let prior_mode = s.input.mode();
        match s.input.option(text) {
            Ok(true) => {
                s.cursor = None;
                if prior_mode != s.input.mode() {
                    self.release_ellipse_picks();
                    let s = self.ellipse_session.as_mut().unwrap();
                    s.curve = None;
                    s.postselected = false;
                }
                self.bind_ellipse_preselection();
                self.point_filter = None;
                self.point_constraint = None;
                self.ellipse_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
            Ok(false) => {
                if picking {
                    let words = word.split_whitespace().collect::<Vec<_>>();
                    let reference = match words.as_slice() {
                        [key, id] if key.eq_ignore_ascii_case("SelID") => {
                            id.parse().ok().map(|id| (id, None))
                        }
                        [one] => one.parse().ok().map(|id| (id, None)).or_else(|| {
                            let (k, v) = one.split_once('=')?;
                            if k.eq_ignore_ascii_case("Edge") {
                                viboceros_command::curve_reference::parse_edge(v)
                                    .map(|(id, edge)| (id, Some(edge)))
                            } else if k.eq_ignore_ascii_case("Curve") {
                                v.parse().ok().map(|id| (id, None))
                            } else {
                                None
                            }
                        }),
                        _ => None,
                    };
                    if let Some((id, edge)) = reference {
                        self.pick_ellipse_curve(Some(id), edge);
                        self.command_input.clear();
                        return true;
                    }
                    if self
                        .commands
                        .recognizes(word.split_whitespace().next().unwrap_or(""))
                    {
                        return false;
                    }
                    self.ellipse_prompt();
                    self.command_input.clear();
                    return true;
                }
                if self
                    .commands
                    .recognizes(word.split_whitespace().next().unwrap_or(""))
                {
                    return false;
                }
                if s.input.point_count_pending() {
                    let result = text
                        .parse::<f64>()
                        .map_err(|_| {
                            "PointCount requires an integer from 4 through 1000".to_owned()
                        })
                        .and_then(|n| s.input.number(n).map_err(|e| e.to_string()));
                    match result {
                        Ok(()) => {
                            self.bind_ellipse_preselection();
                            self.ellipse_prompt();
                        }
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                    self.command_input.clear();
                    return true;
                }
                let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                    text,
                    self.document.units(),
                ) else {
                    return false;
                };
                match value {
                    Ok(value) => match s.input.number(value) {
                        Ok(_) => {
                            self.bind_ellipse_preselection();
                            self.ellipse_prompt();
                        }
                        Err(e) => self.push_log(format!("Error: {e}")),
                    },
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn accept_ellipse_point(&mut self, point: Point3) -> Option<bool> {
        self.ellipse_session.as_ref()?;
        self.validate_ellipse_curve();
        if self.picking_ellipse_curve() {
            self.ellipse_prompt();
            return Some(false);
        }
        let s = self.ellipse_session.as_mut().unwrap();
        if let Some(curve) = &s.curve
            && s.input.requests_curve()
        {
            match viboceros_command::curve_reference::perpendicular_frame(curve.curve_ref(), point)
            {
                Ok(frame) => {
                    s.input.around_curve(frame);
                    s.cursor = None;
                    self.ellipse_prompt();
                    return Some(true);
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return Some(false);
                }
            }
        }
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(Some(_)) => {
                let command = if let Some(curve) = &s.curve {
                    s.input
                        .command_with_guide(curve.object_id(), curve.source().1, s.postselected)
                } else {
                    s.input.command()
                }
                .expect("completed ellipse");
                self.try_execute_command(&command)
            }
            Ok(None) => {
                s.cursor = None;
                self.ellipse_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
}
