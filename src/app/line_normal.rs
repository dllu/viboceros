//! Read-only reference and projected base getter for Line Normal.
use super::*;
use viboceros_command::line::{
    Input, Mode,
    normal::{Component, Reference},
};
#[derive(Clone, Default)]
pub(super) struct Session {
    pub source: Option<Reference>,
    pub pointer: Option<egui::Pos2>,
    binding: Option<(ObjectId, Component)>,
}
impl VibocerosApp {
    pub(super) fn picking_line_normal(&self) -> bool {
        self.plane_prompt.is_none()
            && self.line_input.is_some_and(Input::wants_normal_origin)
            && self
                .line_normal
                .as_ref()
                .is_some_and(|s| s.source.is_none() && s.binding.is_none())
    }
    pub(super) fn release_line_normal(&mut self) {
        if let Some(id) = self
            .line_normal
            .as_ref()
            .and_then(|s| s.source.as_ref())
            .map(Reference::object_id)
        {
            let _ = self.document.select_object(id, SelectionMode::Remove);
        }
    }
    pub(super) fn validate_line_normal(&mut self) {
        if self
            .line_normal
            .as_ref()
            .and_then(|s| s.source.as_ref())
            .is_none_or(|s| s.current(&self.document))
        {
            return;
        }
        self.release_line_normal();
        let old = self.line_input.unwrap();
        let mut input = Input::new(old.plane());
        input.option("Normal").unwrap();
        if old.both_sides() {
            input.option("BothSides").unwrap();
        }
        input
            .option(if old.ignore_trims() {
                "IgnoreTrims=Yes"
            } else {
                "IgnoreTrims=No"
            })
            .unwrap();
        self.line_input = Some(input);
        self.line_normal = Some(Default::default());
        self.line_accepted_point = None;
        self.component_selection.clear_choices();
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log("Line normal reference changed or became unavailable; pick it again".into());
        self.line_prompt();
    }
    pub(super) fn pick_line_normal_at(
        &mut self,
        id: ObjectId,
        component: Component,
        point: Point3,
    ) -> bool {
        if !self.picking_line_normal() {
            return false;
        }
        match Reference::capture(
            &self.document,
            id,
            component,
            point,
            self.line_input.unwrap().ignore_trims(),
        ) {
            Ok(source) => {
                self.line_normal.as_mut().unwrap().source = Some(source);
                self.line_input.as_mut().unwrap().observe_cursor(point);
                let _ = self.document.select_object(id, SelectionMode::Replace);
                self.component_selection.clear_choices();
                self.line_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
    pub(super) fn pick_line_normal_object(&mut self, id: Option<ObjectId>) -> bool {
        if !self.picking_line_normal() {
            return false;
        }
        if let Some(id) = id {
            let point = self.document.object(id).and_then(|o| {
                self.viewports[self.active_viewport].curve_pick_point(
                    o.geometry(),
                    None,
                    self.line_normal.as_ref()?.pointer?,
                    self.document.tolerance(),
                )
            });
            if let Some(point) = point {
                self.pick_line_normal_at(id, Component::Object, point);
            }
        }
        true
    }
    pub(super) fn line_normal_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.line_input?.wants_normal_origin().then_some(())?;
        self.line_normal.as_ref()?.source.as_ref()?.curve()
    }
    pub(super) fn line_normal_surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let input = self.line_input?;
        input.wants_normal_origin().then_some(())?;
        self.line_normal
            .as_ref()?
            .source
            .as_ref()?
            .surface_target(input.ignore_trims())
    }
    pub(super) fn accept_line_normal_parameter(&mut self, parameter: f64) -> bool {
        let Some(curve) = self.line_normal_curve() else {
            return false;
        };
        // Cursor parameters belong to the displayed NURBS. Resolve its point
        // on the original analytic source before taking curvature.
        match curve.evaluate(parameter) {
            Ok(point) => self.accept_line_point(point),
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
    pub(super) fn accept_line_normal_base(&mut self, point: Point3) -> Option<bool> {
        if !self.line_input.is_some_and(Input::wants_normal_origin) {
            return None;
        }
        if let Some((id, component)) = self.line_normal.as_mut()?.binding.take() {
            return Some(self.pick_line_normal_at(id, component, point));
        }
        let Some(source) = self.line_normal.as_ref()?.source.as_ref() else {
            self.push_log("Line Normal: select a curve or surface first".into());
            return Some(false);
        };
        let input = self.line_input.as_mut().unwrap();
        let result = source
            .location(point, input.ignore_trims())
            .and_then(|location| input.normal(location));
        match result {
            Ok(()) => {
                self.line_accepted_point = self.line_input.unwrap().anchor();
                self.line_prompt();
                Some(true)
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                Some(false)
            }
        }
    }
    pub(super) fn try_continue_line_normal(&mut self, text: &str) -> bool {
        if !self.line_input.is_some_and(Input::wants_normal_origin) {
            return false;
        }
        let mut words = text.split_whitespace();
        let Some(binding) = words
            .next()
            .and_then(viboceros_command::line::normal::parse_binding)
        else {
            return false;
        };
        let rest = words.collect::<Vec<_>>().join(" ");
        self.line_normal.as_mut().unwrap().binding = Some(binding);
        if !rest.is_empty() {
            let point =
                viboceros_drafting::PointInput::parse_with_units(&rest, self.document.units())
                    .and_then(Result::ok)
                    .and_then(|p| {
                        p.resolve(
                            self.viewports[self.active_viewport].construction_plane(),
                            self.last_point,
                        )
                        .ok()
                    });
            if let Some(point) = point {
                self.accept_line_normal_base(point);
            } else {
                self.push_log("Error: expected a reference selection point".into());
            }
        } else {
            self.push_log("Line Normal: reference selection point".into());
        }
        true
    }
    pub(super) fn line_normal_prompt(&self) -> Option<&'static str> {
        (self.line_input?.mode() == Mode::Normal && self.line_input?.wants_normal_origin()).then(
            || {
                if self
                    .line_normal
                    .as_ref()
                    .is_some_and(|s| s.source.is_some())
                {
                    "start point on the reference"
                } else {
                    "select a curve or surface; IgnoreTrims=Yes|No"
                }
            },
        )
    }
}
