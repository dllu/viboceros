//! Draw and resolve the active Polyline draft without document edits.
use super::*;
use viboceros_command::polyline_input::{Input, Segment};
use viboceros_geometry::UnitVector3;

const HELPER_CAPTURE_PIXELS: f32 = 12.;

#[derive(Clone, Copy, Debug)]
pub struct PolylinePreview<'a> {
    pub input: &'a Input,
    pub direction_lock: Option<UnitVector3>,
}

impl Viewport {
    pub(crate) fn resolve_polyline_point(
        &self,
        draft: PolylinePreview<'_>,
        point: Point3,
    ) -> Option<Point3> {
        let anchor = draft.input.anchor()?;
        let project = |direction: UnitVector3| {
            let axis = direction.as_vector();
            let distance = anchor.vector_to(point).ok()?.dot(axis).ok()?;
            anchor.translated(axis.scaled(distance).ok()?).ok()
        };
        if let Some(direction) = draft.direction_lock {
            return project(direction);
        }
        if !draft.input.helper_tracking() {
            return None;
        }

        let rect = self.screen_rect()?;
        let cursor = self.project(point, rect)?;
        draft
            .input
            .helper_directions()
            .into_iter()
            .flatten()
            .find_map(|direction| {
                let tracked = project(direction)?;
                let snapped = self.project(tracked, rect)?;
                (cursor.distance(snapped) <= HELPER_CAPTURE_PIXELS).then_some(tracked)
            })
    }

    pub(super) fn paint_active_polyline(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        draft: PolylinePreview<'_>,
        cursor: Option<Point3>,
        tolerance: Tolerance,
    ) {
        for &segment in draft.input.segments() {
            self.paint_polyline_segment(painter, rect, segment);
        }
        let Some(anchor) = draft.input.anchor() else {
            return;
        };
        self.paint_draft_points(painter, rect, &[anchor]);
        if let Some(point) = cursor {
            let preview = draft.input.preview(point, tolerance);
            if let Some([start, end]) = preview.guide
                && let Some(line) = self.project_segment(start, end, rect)
            {
                painter.line_segment(line, Stroke::new(1., Color32::from_gray(140)));
            }
            if let Some(next) = preview.next {
                self.paint_polyline_segment(painter, rect, next);
            }
            if let Some(closing) = preview.closing {
                self.paint_polyline_segment(painter, rect, closing);
            }
            if draft.direction_lock.is_some() || draft.input.helper_tracking() {
                let directions = match draft.direction_lock {
                    Some(direction) => [Some(direction), None, None],
                    None => draft.input.helper_directions(),
                };
                for direction in directions.into_iter().flatten() {
                    if let Ok(point) = anchor.translated(direction.as_vector())
                        && let Some([a, b]) = self.project_segment(anchor, point, rect)
                        && let Some(line) = super::screen::clip_line_to_rect(a, b, rect, true)
                    {
                        painter.line_segment(line, Stroke::new(1., Color32::from_gray(140)));
                    }
                }
            }
        } else if let Some(closing) = draft.input.persistent_preview(tolerance) {
            self.paint_polyline_segment(painter, rect, closing);
        }
    }

    fn paint_polyline_segment(&self, painter: &egui::Painter, rect: Rect, segment: Segment) {
        match segment {
            Segment::Line(line) => {
                if let Some(points) = self.project_segment(line.start(), line.end(), rect) {
                    painter.line_segment(points, Stroke::new(2., Color32::from_rgb(20, 115, 190)));
                }
            }
            Segment::Arc(arc) => self.paint_arc_preview(painter, rect, arc, None),
        }
    }
}
