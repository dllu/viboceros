//! Live point placement in one document transaction, with command-local Undo.
use super::*;
use viboceros_document::{Geometry, ObjectId};

pub(super) struct PointsSession {
    ids: Vec<ObjectId>,
    previous_last: Option<Point3>,
}

impl VibocerosApp {
    pub(super) fn apply_points_point(&mut self, point: Point3) -> bool {
        if self.points_session.is_none() {
            if let Err(error) = self.document.begin_transaction("Points") {
                self.push_log(format!("Error: {error}"));
                return false;
            }
            self.points_session = Some(PointsSession {
                ids: vec![],
                previous_last: self.last_point,
            });
        }
        match self.document.add_geometry(Geometry::Point(point)) {
            Ok(id) => {
                self.points_session.as_mut().unwrap().ids.push(id);
                self.push_log(format!(
                    "Point: {} (Undo removes the last point; Enter or Esc finishes)",
                    format_model_point(point)
                ));
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn try_continue_points(&mut self, input: &str) -> bool {
        if self.active_command != Some(InteractiveCommand::Points)
            || self.plane_prompt.is_some()
            || self.object_prompt.is_some()
        {
            return false;
        }
        if input.is_empty() {
            self.cancel_interactive_command(false);
            return true;
        }
        if !input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Undo")
        {
            return false;
        }
        if let Some(session) = self.points_session.as_mut()
            && let Some(id) = session.ids.last().copied()
        {
            match self.document.delete_object(id) {
                Ok(()) => {
                    session.ids.pop();
                    self.last_point = session
                        .ids
                        .last()
                        .and_then(|id| self.document.object(*id))
                        .and_then(|o| {
                            if let Geometry::Point(p) = o.geometry() {
                                Some(*p)
                            } else {
                                None
                            }
                        })
                        .or(session.previous_last);
                    self.push_log("Removed the last point in Points".into());
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        } else {
            self.push_log("No points to undo in this Points session".into());
        }
        self.command_input.clear();
        true
    }

    pub(super) fn finish_points_session(&mut self) {
        let Some(session) = self.points_session.take() else {
            return;
        };
        let count = session.ids.len();
        let result = if count == 0 {
            self.document.rollback_transaction()
        } else {
            self.document.commit_transaction()
        };
        match result {
            Ok(_) => self.push_log(format!("Finished Points: {count} point objects")),
            Err(error) => self.push_log(format!("Error finishing Points: {error}")),
        }
    }
}
