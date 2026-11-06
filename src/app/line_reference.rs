//! Immutable Line reference capture and ordinary/edge viewport picking.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
#[derive(Clone, Default)]
pub(super) struct Session {
    pub source: Option<CapturedCurve>,
    pub pointer: Option<egui::Pos2>,
    pub binding: Option<(ObjectId, Option<usize>)>,
}
impl VibocerosApp {
    pub(super) fn picking_line_curve(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .line_input
                .is_some_and(viboceros_command::line::Input::wants_curve)
            && self
                .line_reference
                .as_ref()
                .is_none_or(|s| s.binding.is_none())
    }
    pub(super) fn release_line_reference(&mut self) {
        if let Some(id) = self
            .line_reference
            .as_ref()
            .and_then(|s| s.source.as_ref())
            .map(CapturedCurve::object_id)
        {
            let _ = self.document.select_object(id, SelectionMode::Remove);
        }
    }
    pub(super) fn validate_line_reference(&mut self) {
        if self
            .line_reference
            .as_ref()
            .and_then(|s| s.source.as_ref())
            .is_none_or(|s| s.current(&self.document))
        {
            return;
        }
        self.release_line_reference();
        let old = self.line_input.unwrap();
        let mut input = viboceros_command::line::Input::new(old.plane());
        input.option("Extension").unwrap();
        if old.both_sides() {
            input.option("BothSides").unwrap();
        }
        self.line_input = Some(input);
        self.line_reference.as_mut().unwrap().source = None;
        self.line_accepted_point = None;
        self.component_selection.clear_choices();
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log("Line reference changed or became unavailable; pick it again".into());
        self.line_prompt();
    }
    pub(super) fn pick_line_reference_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        if !self.picking_line_curve() {
            return false;
        }
        let result = CapturedCurve::capture(&self.document, id, edge).and_then(|source| {
            self.line_input
                .as_mut()
                .unwrap()
                .extension(source.curve_ref(), point)?;
            Ok(source)
        });
        self.component_selection.clear_choices();
        match result {
            Ok(source) => {
                self.line_reference.as_mut().unwrap().source = Some(source);
                self.last_point = self.line_input.unwrap().anchor();
                self.line_accepted_point = self.last_point;
                let _ = self.document.select_object(id, SelectionMode::Replace);
                self.line_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                if self.command_file_job.is_some() {
                    self.command_file_step_failed = true;
                }
                false
            }
        }
    }
    pub(super) fn pick_line_reference_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.line_reference.is_none() {
            return false;
        }
        self.validate_line_reference();
        if self.picking_line_curve() {
            let point = id.and_then(|id| {
                self.viewports[self.active_viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    edge,
                    self.line_reference.as_ref()?.pointer?,
                    self.document.tolerance(),
                )
            });
            if let (Some(id), Some(point)) = (id, point) {
                self.pick_line_reference_at(id, edge, point);
            } else {
                self.push_log("Line Extension: pick a curve near an end".into());
            }
        }
        true
    }
    pub(super) fn try_continue_line_reference(&mut self, text: &str) -> bool {
        if !self
            .line_input
            .is_some_and(viboceros_command::line::Input::wants_curve)
        {
            return false;
        }
        let words = text.split_whitespace().collect::<Vec<_>>();
        let Some((key, value)) = words
            .first()
            .and_then(|s| s.trim_start_matches('_').split_once('='))
        else {
            return false;
        };
        let reference = if key.eq_ignore_ascii_case("Curve") {
            value.parse().ok().map(|id| (id, None))
        } else if key.eq_ignore_ascii_case("Edge") {
            viboceros_command::curve_reference::parse_edge(value).map(|(id, e)| (id, Some(e)))
        } else {
            return false;
        };
        if words.len() == 1
            && let Some(binding) = reference
        {
            self.line_reference.as_mut().unwrap().binding = Some(binding);
            self.push_log("Line Extension: point near the chosen curve end".into());
            return true;
        }
        let point = viboceros_drafting::PointInput::parse_with_units(
            &words[1..].join(" "),
            self.document.units(),
        )
        .and_then(Result::ok)
        .and_then(|p| {
            p.resolve(
                self.viewports[self.active_viewport].construction_plane(),
                self.last_point,
            )
            .ok()
        });
        if let (Some((id, edge)), Some(point)) = (reference, point) {
            self.line_reference.as_mut().unwrap().binding = None;
            self.pick_line_reference_at(id, edge, point);
        } else {
            self.push_log(
                "Error: use Curve=id|Edge=id,index followed by a point near the chosen end".into(),
            );
        }
        true
    }
}
