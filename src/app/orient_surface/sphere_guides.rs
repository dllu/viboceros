//! Read-only sphere construction guides retain the outer Splop source owner.
use super::*;
use viboceros_command::{
    curve_reference::{CapturedCurve, perpendicular_frame},
    point_fitting::{self, PointFitKind},
    splop::ReferenceInput,
};
pub(super) enum Guide {
    FitPoints,
    Around {
        curve: Option<CapturedCurve>,
        center: bool,
    },
}
impl Session {
    pub(crate) fn sphere_cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        match &self.sphere_guide {
            Some(Guide::Around {
                curve: Some(curve),
                center: false,
            }) => Some(curve.curve()),
            _ => None,
        }
    }
}
impl VibocerosApp {
    pub(crate) fn picking_splop_fit_points(&self) -> bool {
        self.orient_surface_session
            .as_ref()
            .is_some_and(|s| matches!(s.sphere_guide, Some(Guide::FitPoints)))
    }
    pub(crate) fn picking_splop_reference_curve(&self) -> bool {
        self.orient_surface_session
            .as_ref()
            .is_some_and(|s| matches!(s.sphere_guide, Some(Guide::Around { curve: None, .. })))
    }
    pub(super) fn start_splop_reference_guide(&mut self, word: &str) -> bool {
        let Some(s) = self.orient_surface_session.as_mut().filter(|s| {
            s.kind == Kind::Splop && s.sphere_input.is_some_and(|input| input.at_start())
        }) else {
            return false;
        };
        if word.eq_ignore_ascii_case("Tangent") {
            self.point_filter = None;
            self.point_constraint = None;
            self.start_splop_tangent();
            self.command_input.clear();
            return true;
        }
        let guide = if word.eq_ignore_ascii_case("FitPoints") {
            Guide::FitPoints
        } else if word.eq_ignore_ascii_case("AroundCurve") {
            Guide::Around {
                curve: None,
                center: false,
            }
        } else {
            return false;
        };
        s.guided_reference = true;
        s.sphere_guide = Some(guide);
        self.document.clear_selection();
        self.point_filter = None;
        self.point_constraint = None;
        if self.picking_splop_fit_points() {
            let mut prompt = object_selection::PendingObjectCommand::selecting(
                point_fitting::selection_prompt(PointFitKind::Sphere),
            );
            prompt.command_override = Some("Splop FitPoints".into());
            self.object_prompt = Some(prompt);
            self.active_command = None;
        }
        self.orient_surface_prompt();
        self.command_input.clear();
        true
    }
    pub(crate) fn continue_splop_fit_points(&mut self, input: &str) -> bool {
        if !self.picking_splop_fit_points() {
            return false;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        let words = word.split_whitespace().collect::<Vec<_>>();
        if let [name, id] = words.as_slice()
            && name.eq_ignore_ascii_case("SelID")
        {
            match id.parse() {
                Ok(id) => self.select_prompt_objects([id], SelectionMode::Add),
                Err(_) => self.push_log("Error: invalid reference point object ID".into()),
            }
            self.command_input.clear();
            return true;
        }
        if !word.is_empty() && !word.eq_ignore_ascii_case("Enter") {
            return false;
        }
        let points = PointFitKind::Sphere.selected_points(&self.document);
        match ReferenceInput::fit_points(&points) {
            Ok(definition) => {
                self.object_prompt = None;
                self.orient_surface_session.as_mut().unwrap().sphere_guide = None;
                self.active_command = Some(Kind::Splop.command(Some(definition.frame.origin())));
                self.accept_splop_reference_result(Ok(Some(definition)));
            }
            Err(e) => self.push_log(format!("Error fitting reference sphere: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn splop_reference_guide_prompt(&mut self) -> bool {
        let Some(s) = self.orient_surface_session.as_ref() else {
            return false;
        };
        let prompt = match &s.sphere_guide {
            Some(Guide::FitPoints) => Some(
                "Select points or visible grips for the reference sphere; Enter fits, Esc cancels",
            ),
            Some(Guide::Around { curve: None, .. }) => {
                Some("Select a curve or surface edge for the reference sphere")
            }
            Some(Guide::Around { center: false, .. }) => Some("Reference sphere center on curve"),
            _ => None,
        };
        if let Some(prompt) = prompt {
            self.push_log(format!("Splop: {prompt}"));
            true
        } else {
            false
        }
    }
    pub(crate) fn pick_splop_reference_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_splop_reference_curve() {
            return false;
        }
        let result = id
            .ok_or(viboceros_command::CommandError::Usage(
                "Select a curve or surface edge",
            ))
            .and_then(|id| CapturedCurve::capture(&self.document, id, edge));
        match result {
            Ok(curve) => {
                if edge.is_none() {
                    let _ = self
                        .document
                        .select_object(curve.object_id(), SelectionMode::Add);
                }
                self.orient_surface_session.as_mut().unwrap().sphere_guide = Some(Guide::Around {
                    curve: Some(curve),
                    center: false,
                });
                self.component_selection.clear_choices();
                self.orient_surface_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn continue_splop_reference_curve_pick(&mut self, word: &str) -> bool {
        if !self.picking_splop_reference_curve() {
            return false;
        }
        let parts = word.split_whitespace().collect::<Vec<_>>();
        let pick = match parts.as_slice() {
            [name, id] if name.eq_ignore_ascii_case("SelID") => {
                id.parse().ok().map(|id| (id, None))
            }
            [one] => one.parse().ok().map(|id| (id, None)).or_else(|| {
                let (key, value) = one.split_once('=')?;
                if key.eq_ignore_ascii_case("Curve") {
                    value.parse().ok().map(|id| (id, None))
                } else if key.eq_ignore_ascii_case("Edge") {
                    viboceros_command::curve_reference::parse_edge(value)
                        .map(|(id, i)| (id, Some(i)))
                } else {
                    None
                }
            }),
            _ => None,
        };
        if let Some((id, edge)) = pick {
            self.pick_splop_reference_curve(Some(id), edge);
            self.command_input.clear();
            true
        } else {
            false
        }
    }
    pub(super) fn splop_reference_curve_frame(
        &self,
        point: Point3,
    ) -> Result<Option<Frame3>, viboceros_command::CommandError> {
        let Some(s) = &self.orient_surface_session else {
            return Ok(None);
        };
        match &s.sphere_guide {
            Some(Guide::Around {
                curve: Some(curve),
                center: false,
            }) if curve.current(&self.document) => {
                Ok(Some(perpendicular_frame(curve.curve_ref(), point)?))
            }
            _ => Ok(None),
        }
    }
    pub(super) fn validate_splop_reference_curve(&mut self) -> bool {
        let stale=self.orient_surface_session.as_ref().is_some_and(|s|matches!(&s.sphere_guide,Some(Guide::Around {curve:Some(curve),..})if !curve.current(&self.document)));
        if !stale {
            return true;
        }
        self.document.clear_selection();
        if !self.refresh_splop_reference_history() {
            return false;
        }
        let s = self.orient_surface_session.as_mut().unwrap();
        s.sphere_guide = Some(Guide::Around {
            curve: None,
            center: false,
        });
        s.sphere_input = Some(ReferenceInput::new(s.plane));
        s.phase = Phase::Base;
        self.active_command = Some(Kind::Splop.command(None));
        self.drafting_plane = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log("Reference curve changed or became unavailable; select it again".into());
        self.orient_surface_prompt();
        false
    }
    pub(crate) fn refresh_splop_reference_history(&mut self) -> bool {
        let Some(s) = self.orient_surface_session.as_mut() else {
            return false;
        };
        if !self.document.history_group_is_current(&s.group) {
            match self.document.begin_history_group("Splop") {
                Ok(group) => s.group = group,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return false;
                }
            }
        }
        true
    }
    pub(crate) fn accept_splop_tangent_circle(
        &mut self,
        circle: viboceros_geometry::Circle3,
    ) -> bool {
        if !self.refresh_splop_reference_history() {
            return false;
        }
        let definition = viboceros_command::circle_input::CircleDefinition::from_circle(circle);
        if definition.is_err() {
            return self.accept_splop_reference_result(definition.map(Some));
        }
        self.tangent_session = None;
        self.active_command = Some(Kind::Splop.command(Some(circle.center())));
        self.accept_splop_reference_result(definition.map(Some))
    }
    pub(super) fn accept_splop_reference_curve_center(&mut self, point: Point3) -> Option<bool> {
        let s = self.orient_surface_session.as_ref()?;
        let Some(Guide::Around {
            curve: Some(curve),
            center: false,
        }) = &s.sphere_guide
        else {
            return None;
        };
        let result = perpendicular_frame(curve.curve_ref(), point);
        match result {
            Ok(frame) => {
                let s = self.orient_surface_session.as_mut().unwrap();
                if let Some(Guide::Around { center, .. }) = &mut s.sphere_guide {
                    *center = true;
                }
                s.sphere_input = Some(ReferenceInput::around_frame(frame));
                s.phase = Phase::Reference;
                self.active_command = Some(Kind::Splop.command(Some(frame.origin())));
                self.drafting_plane = Some(frame);
                self.orient_surface_prompt();
                Some(true)
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                Some(false)
            }
        }
    }
    pub(crate) fn accept_splop_reference_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .orient_surface_session
            .as_ref()
            .and_then(Session::sphere_cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(point) = point {
            self.accept_filtered_drafting_point(point, false);
            true
        } else {
            false
        }
    }
}
