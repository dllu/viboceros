//! NURBS blend control nets and unbounded shape-handle line constraints.
use super::*;
use viboceros_command::blend_curve::{Handle, Input};
impl Viewport {
    pub(super) fn blend_curve_shape_parameter(
        &self,
        input: &Input,
        pointer: Pos2,
        rect: Rect,
        doc: &Document,
        osnap: viboceros_drafting::ObjectSnapOptions,
    ) -> Option<Real> {
        let constraint = input.shape_constraint()?;
        let point = if let Some(snap) = self.object_snap(pointer, rect, doc, osnap) {
            constraint.resolve(snap.point()).ok()?
        } else {
            let (offset, direction) =
                self.drafting_view_line_relative_to(constraint.anchor, pointer, rect)?;
            constraint.project_view_line(offset, direction)?
        };
        constraint
            .anchor
            .vector_to(point)
            .ok()?
            .dot(constraint.direction?.as_vector())
            .ok()
    }
    pub(super) fn pick_blend_curve_handle(
        &self,
        pointer: Pos2,
        rect: Rect,
        input: &Input,
    ) -> Option<Handle> {
        if input.active_handle().is_some() {
            return None;
        }
        input
            .control_points()
            .into_iter()
            .filter_map(|(h, p)| {
                Some((
                    self.project_selection_point(p, rect)?.distance_sq(pointer),
                    h,
                ))
            })
            .filter(|(d, _)| *d <= 12. * 12.)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, h)| h)
    }
    pub(super) fn paint_blend_curve_handles(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: &Input,
    ) {
        if let Some(curve) = input.preview() {
            for pair in curve.control_points().windows(2) {
                if let (Some(a), Some(b)) = (
                    self.project_selection_point(pair[0].point(), rect),
                    self.project_selection_point(pair[1].point(), rect),
                ) {
                    painter.line_segment([a, b], Stroke::new(1., Color32::from_gray(135)));
                }
            }
        }
        for (handle, point) in input.control_points() {
            if let Some(p) = self.project_selection_point(point, rect) {
                painter.circle_filled(
                    p,
                    4.,
                    if input.active_handle() == Some(handle) {
                        Color32::YELLOW
                    } else {
                        Color32::WHITE
                    },
                );
                painter.circle_stroke(p, 4., Stroke::new(1., Color32::from_rgb(20, 115, 190)));
            }
        }
    }
}
