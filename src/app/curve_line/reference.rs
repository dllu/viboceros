//! Curve/edge mouse hits and deterministic command-file bindings.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn pick_curve_line_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        self.validate_curve_line();
        let Some(s) = &mut self.curve_line else {
            return false;
        };
        let pair = s.input.stage() == Stage::SecondOfTwo;
        let result = s.input.pick_curve(&self.document, id, edge, point);
        self.component_selection.clear_choices();
        match result {
            Ok(Some(d)) => self.finish_line_definition(d),
            Ok(None) => {
                self.line_accepted_point = self.curve_line.as_ref().unwrap().input.anchor();
                self.last_point = self.line_accepted_point.or(self.last_point);
                self.curve_line_prompt();
                true
            }
            Err(e) => {
                self.curve_line_error(e);
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
    pub(in crate::app) fn pick_curve_line_base_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_curve_line() {
            return false;
        }
        let point = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.curve_line.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(point)) = (id, point) {
            self.pick_curve_line_at(id, edge, point);
            self.line_accepted_point = None;
        }
        true
    }
    pub(in crate::app) fn try_curve_line_binding(&mut self, text: &str) -> bool {
        let Some(s) = &self.curve_line else {
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
                self.curve_line_error("Line contact requires a curve or edge");
                return true;
            }
        };
        self.curve_line.as_mut().unwrap().binding = Some((id, edge));
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
                self.curve_line.as_mut().unwrap().binding = None;
                self.pick_curve_line_at(id, edge, p);
                self.line_accepted_point = None;
            } else {
                self.curve_line_error("expected a point near the curve contact");
            }
        } else {
            self.push_log("Point near the selected curve contact".into());
        }
        true
    }
}
