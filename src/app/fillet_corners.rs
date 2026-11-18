//! Selection-fixed FilletCorners radius getter and transient wire preview.
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_command::fillet_corners::{parse_radius, prepare};
use viboceros_document::GeometrySnapshot;
use viboceros_drafting::{PointConstraintInput, PointConstraintState};
use viboceros_geometry::{PolyCurve3, Real};

pub(super) struct Session {
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: Tolerance,
    postselected: bool,
    selection_before: Option<Vec<ObjectId>>,
    radius: Option<Real>,
    preview: Vec<PolyCurve3>,
}

impl Session {
    pub(super) fn preview(&self) -> &[PolyCurve3] {
        &self.preview
    }

    fn current(&self, doc: &Document) -> bool {
        doc.tolerance() == self.tolerance
            && doc.selected_object_count() == self.sources.len()
            && doc
                .selected_objects()
                .zip(&self.sources)
                .all(|(object, (id, geometry))| {
                    object.id() == *id && object.geometry_snapshot().shares_storage_with(geometry)
                })
    }
}

impl VibocerosApp {
    pub(super) fn start_fillet_corners(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("FilletCorners")
        }) || words.next().is_some()
            || !self
                .document
                .selected_objects()
                .any(|o| ObjectSelectionFilter::FilletCorners.accepts_object(o))
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.push_log(format!("> {input}"));
        self.begin_fillet_corners(false);
        true
    }

    pub(super) fn begin_fillet_corners(&mut self, postselected: bool) {
        let sources = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::FilletCorners.accepts_object(o))
            .map(|object| (object.id(), object.geometry_snapshot().clone()))
            .collect::<Vec<_>>();
        let selection_before = (!postselected
            && sources.len() != self.document.selected_object_count())
        .then(|| self.document.selected_object_ids().collect());
        if selection_before.is_some()
            && let Err(error) = self
                .document
                .set_transient_selection(sources.iter().map(|(id, _)| *id))
        {
            self.push_log(format!("Error: {error}"));
            return;
        }
        if postselected {
            self.document
                .mark_object_selection_temporary(sources.iter().map(|(id, _)| *id));
        }
        self.fillet_corners_session = Some(Session {
            sources,
            tolerance: self.document.tolerance(),
            postselected,
            selection_before,
            radius: None,
            preview: Vec::new(),
        });
        self.active_command = Some(InteractiveCommand::FilletCorners { anchor: None });
        self.command_input.clear();
        self.log_fillet_corners_prompt();
    }

    pub(super) fn release_fillet_corners(&mut self) {
        if let Some(session) = self.fillet_corners_session.take() {
            if let Some(selection) = session.selection_before {
                self.restore_curve_selection(selection);
            } else if session.postselected {
                self.document.clear_temporary_object_selection();
            }
        }
    }

    fn fillet_corners_default_radius(&self) -> Real {
        self.commands
            .transform_scalar_default("FilletCorners")
            .unwrap()
    }

    fn log_fillet_corners_prompt(&mut self) {
        let prompt = self.active_command.unwrap().prompt();
        self.push_log(format!(
            "{prompt} <{}>",
            self.fillet_corners_default_radius()
        ));
    }

    pub(super) fn validate_fillet_corners(&mut self) {
        if self
            .fillet_corners_session
            .as_ref()
            .is_some_and(|s| !s.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log(
                "FilletCorners sources or tolerance changed. Select the curves again.".into(),
            );
        }
    }

    pub(super) fn update_fillet_corners_preview(&mut self, point: Option<Point3>) -> bool {
        let Some(session) = self.fillet_corners_session.as_mut() else {
            return false;
        };
        let radius = match self.active_command {
            Some(InteractiveCommand::FilletCorners {
                anchor: Some(start),
            }) if self.plane_prompt.is_none() => point
                .filter(|_| self.point_filter.is_none_or(|f| !f.awaiting_source()))
                .and_then(|p| self.point_filter.map_or(Ok(p), |f| f.preview_point(p)).ok())
                .and_then(|p| {
                    self.point_constraint
                        .map_or(Ok(p), |c| c.apply_cursor(p))
                        .ok()
                })
                .and_then(|p| start.distance_to(p).ok()),
            _ => None,
        };
        if session.radius == radius {
            return false;
        }
        session.radius = radius;
        session.preview = radius
            .and_then(|radius| prepare(&self.document, radius).ok())
            .map(|curves| curves.into_iter().map(|(_, curve)| curve).collect())
            .unwrap_or_default();
        true
    }

    pub(super) fn accept_fillet_corners_point(&mut self, point: Point3) -> Option<bool> {
        let Some(InteractiveCommand::FilletCorners { anchor }) = self.active_command else {
            return None;
        };
        if self.plane_prompt.is_some() {
            return None;
        }
        if let Some(start) = anchor {
            Some(match start.distance_to(point) {
                Ok(radius) => self.finish_fillet_corners(radius),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    false
                }
            })
        } else {
            self.active_command = Some(InteractiveCommand::FilletCorners {
                anchor: Some(point),
            });
            self.log_fillet_corners_prompt();
            Some(true)
        }
    }

    pub(super) fn continue_fillet_corners(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::FilletCorners { anchor }) = self.active_command else {
            return false;
        };
        if self.plane_prompt.is_some() {
            return false;
        }
        let radius =
            if input.is_empty() || input.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
                Some(self.fillet_corners_default_radius())
            } else {
                parse_radius(input, self.document.units()).ok()
            };
        if let Some(radius) = radius {
            if let Some(anchor) = anchor {
                let plane = self.viewports[self.active_viewport].construction_plane();
                let mut constraint = self
                    .point_constraint
                    .unwrap_or_else(|| PointConstraintState::new(anchor, plane));
                match constraint.set(PointConstraintInput::Distance(radius), plane) {
                    Ok(()) => {
                        self.point_constraint = Some(constraint);
                        self.push_log(format!("Distance set to {radius}. Pick the second point."));
                    }
                    Err(error) => {
                        self.command_file.fail_dispatch();
                        self.push_log(format!("Error: {error}"));
                    }
                }
                self.command_input.clear();
            } else {
                self.finish_fillet_corners(radius);
            }
            return true;
        }
        if self.try_continue_point_constraint(input)
            || self.try_continue_point_filter(input)
            || self.try_continue_point_input(input)
        {
            return true;
        }
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
            || input
                .split_whitespace()
                .next()
                .is_some_and(|n| self.commands.recognizes(n))
        {
            return false;
        }
        self.command_input.clear();
        self.command_file.fail_dispatch();
        self.push_log("Enter a valid radius or distance point".into());
        true
    }

    fn finish_fillet_corners(&mut self, radius: Real) -> bool {
        self.validate_fillet_corners();
        let Some(session) = &self.fillet_corners_session else {
            return false;
        };
        let input = format!("FilletCorners {radius}");
        let result = if session.postselected {
            self.commands
                .execute_postselected(&mut self.document, &input, Default::default())
        } else {
            self.commands.execute(&mut self.document, &input)
        };
        self.command_input.clear();
        match result {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(message);
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
