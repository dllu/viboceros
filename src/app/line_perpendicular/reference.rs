//! Curve/edge mouse hits and deterministic command-file bindings.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn pick_line_perpendicular_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        self.validate_line_perpendicular();
        let Some(s) = &mut self.line_perpendicular else {
            return false;
        };
        let pair = s.input.stage() == Stage::SecondOfTwo;
        let result = s.input.pick_curve(&self.document, id, edge, point);
        self.component_selection.clear_choices();
        match result {
            Ok(Some(d)) => self.finish_line_definition(d),
            Ok(None) => {
                self.line_accepted_point = self.line_perpendicular.as_ref().unwrap().input.anchor();
                self.last_point = self.line_accepted_point.or(self.last_point);
                self.line_perpendicular_prompt();
                true
            }
            Err(e) => {
                self.perpendicular_error(e);
                if pair {
                    self.cancel_interactive_command(false);
                    if self.command_file_job.is_some() {
                        self.command_file_step_failed = true;
                    }
                }
                false
            }
        }
    }
    pub(in crate::app) fn pick_line_perpendicular_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_line_perpendicular() {
            return false;
        }
        let point = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.line_perpendicular.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(point)) = (id, point) {
            self.pick_line_perpendicular_at(id, edge, point);
            self.line_accepted_point = None;
        }
        true
    }
    pub(in crate::app) fn try_line_perpendicular_binding(&mut self, text: &str) -> bool {
        let Some(s) = &self.line_perpendicular else {
            return false;
        };
        if !s.input.accepts_curve() {
            return false;
        }
        let words = text.split_whitespace().collect::<Vec<_>>();
        let Some((id, component)) = words
            .first()
            .and_then(|w| viboceros_command::line::normal::parse_binding(w))
        else {
            return false;
        };
        let edge = match component {
            viboceros_command::line::normal::Component::Object => None,
            viboceros_command::line::normal::Component::Edge(e) => Some(e),
            _ => {
                self.perpendicular_error("Perpendicular requires a curve or edge");
                return true;
            }
        };
        self.line_perpendicular.as_mut().unwrap().binding = Some((id, edge));
        if words.len() > 1 {
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
            if let Some(p) = point {
                self.line_perpendicular.as_mut().unwrap().binding = None;
                self.pick_line_perpendicular_at(id, edge, p);
                self.line_accepted_point = None;
            } else {
                self.perpendicular_error("expected a point near the curve contact");
            }
        } else {
            self.push_log("Perpendicular: point near the selected curve contact".into());
        }
        true
    }
}
