//! Constrained point picks on the frozen extrusion rail.
use super::*;
use crate::viewport::surface_extrusion::{SurfaceExtrusionDraft, SurfacePathPreview};
use viboceros_drafting::PointInput;
use viboceros_geometry::NurbsCurve;

mod boundary_preview;

pub(super) struct PathPoints {
    rail: ObjectId,
    snapshot: viboceros_document::GeometrySnapshot,
    curve: NurbsCurve,
    reverse: bool,
    start: Option<f64>,
    draft: Option<SurfaceExtrusionDraft>,
    boundary_preview: Option<boundary_preview::Preview>,
}

impl PathPoints {
    fn is_current(&self, document: &Document) -> bool {
        document.is_object_selectable(self.rail)
            && document
                .object(self.rail)
                .is_some_and(|rail| rail.geometry_snapshot() == &self.snapshot)
    }
}

impl SurfacePathSession {
    pub(in crate::app) fn hint(&self) -> &'static str {
        match self.points.as_ref().map(|points| points.start) {
            None if matches!(self.boundary, Boundary::Selecting) => "Select a boundary surface",
            None if matches!(self.boundary, Boundary::Picked { .. }) => {
                "Select path curve near start. DeleteInput / SubCurve / SplitAtTangents / Reverse / PathLength"
            }
            None => {
                "Select path curve near start. Solid / DeleteInput / SubCurve / SplitAtTangents / ToBoundary / Reverse / PathLength"
            }
            Some(None) => "Starting point on curve. Press Enter for whole path curve",
            Some(Some(_)) => "Ending point on curve",
        }
    }

    pub(in crate::app) fn point_curve(&self, document: &Document) -> Option<&NurbsCurve> {
        let points = self.points.as_ref()?;
        points.is_current(document).then_some(&points.curve)
    }

    pub(in crate::app) fn point_markers(&self) -> &[f64] {
        self.points
            .as_ref()
            .map_or(&[], |points| points.start.as_slice())
    }

    pub(in crate::app) fn selected_rail(&self, document: &Document) -> Option<ObjectId> {
        let points = self.points.as_ref()?;
        points.is_current(document).then_some(points.rail)
    }

    pub(in crate::app) fn preview(&self, document: &Document) -> Option<SurfacePathPreview<'_>> {
        let points = self.points.as_ref()?;
        if !self.sources.is_current(document)
            || !points.is_current(document)
            || !self.boundary.is_current(document)
        {
            return None;
        }
        Some(SurfacePathPreview {
            draft: points.draft.as_ref()?,
            curve: &points.curve,
            start: points.start?,
            reverse: points.reverse,
            solid: self.options.solid,
            split_at_tangents: self.options.split_at_tangents,
        })
    }
}

impl VibocerosApp {
    pub(super) fn begin_surface_path_points(
        &mut self,
        rail: ObjectId,
        curve: NurbsCurve,
        reverse: bool,
    ) {
        if !matches!(curve.is_closed(), Ok(false)) {
            self.push_log("Error: Select an open path curve".into());
            return;
        }
        let session = self.surface_path_prompt.as_mut().unwrap();
        let draft = matches!(session.boundary, Boundary::None)
            .then(|| SurfaceExtrusionDraft::capture(&self.document, &session.sources.targets))
            .flatten();
        let boundary_preview = session.boundary.face().and_then(|boundary| {
            boundary_preview::Preview::capture(&self.document, &session.sources.targets, boundary)
        });
        session.points = Some(PathPoints {
            rail,
            snapshot: self
                .document
                .object(rail)
                .unwrap()
                .geometry_snapshot()
                .clone(),
            curve,
            reverse,
            start: None,
            draft,
            boundary_preview,
        });
        self.log_surface_path_prompt();
    }

    pub(super) fn continue_surface_path_points(&mut self, input: &str) -> bool {
        let points = self
            .surface_path_prompt
            .as_ref()
            .unwrap()
            .points
            .as_ref()
            .unwrap();
        if input.is_empty()
            || input
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Enter")
        {
            if points.start.is_some() {
                self.cancel_current_prompt_or_selection();
            } else {
                self.finish_surface_path_points(None);
            }
        } else {
            let Some(parsed) = PointInput::parse_with_units(input, self.document.units()) else {
                return false;
            };
            let relative_base = points
                .start
                .and_then(|t| points.curve.evaluate(t).ok())
                .or(self.last_point);
            let plane = self.viewports[self.active_viewport].construction_plane();
            let parameter = parsed
                .and_then(|point| point.resolve(plane, relative_base))
                .map_err(|error| error.to_string())
                .and_then(|point| {
                    points
                        .curve
                        .closest_parameter(point, self.document.tolerance())
                        .map_err(|error| error.to_string())
                });
            match parameter {
                Ok(parameter) => {
                    self.accept_surface_path_parameter(parameter);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.command_input.clear();
        true
    }

    pub(in crate::app) fn accept_surface_path_parameter(&mut self, parameter: f64) -> bool {
        let Some(session) = self.surface_path_prompt.as_mut() else {
            return false;
        };
        let Some(points) = session.points.as_mut() else {
            return false;
        };
        if !session.boundary.is_current(&self.document) {
            self.reset_surface_path_boundary();
            return true;
        }
        if !points.is_current(&self.document) {
            self.push_log("Error: path curve changed; restart ExtrudeSrfAlongCrv".into());
            return true;
        }
        if let Some(start) = points.start {
            let distinct = points
                .curve
                .evaluate(start)
                .ok()
                .zip(points.curve.evaluate(parameter).ok())
                .is_some_and(|(a, b)| !a.is_near(b, self.document.tolerance()));
            if distinct {
                self.finish_surface_path_points(Some([start, parameter]));
            } else {
                self.push_log("Pick a distinct ending point on the curve".into());
            }
        } else {
            points.start = Some(parameter);
            self.log_surface_path_prompt();
        }
        true
    }

    fn finish_surface_path_points(&mut self, path_range: Option<[f64; 2]>) {
        let session = self.surface_path_prompt.as_ref().unwrap();
        let points = session.points.as_ref().unwrap();
        if !points.is_current(&self.document) {
            self.push_log("Error: path curve changed; restart ExtrudeSrfAlongCrv".into());
            return;
        }
        self.finish_surface_path(
            points.rail,
            SurfacePathOptions {
                reverse: Some(points.reverse),
                path_length: None,
                path_range,
                ..session.options
            },
        );
    }
}
