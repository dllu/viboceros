//! Screen-space picks for the three transient arc-blend adjustment handles.
use super::*;
use viboceros_command::arc_blend::{Handle, Input};
impl Viewport {
    pub(super) fn arc_blend_ratio_cursor(
        &self,
        input: &Input,
        pointer: Pos2,
        rect: Rect,
        doc: &Document,
        osnap: viboceros_drafting::ObjectSnapOptions,
    ) -> Option<edge_point::EdgePointCursor> {
        let snap = self.object_snap(pointer, rect, doc, osnap);
        let parameter = if let Some(snap) = snap {
            input
                .adjustment_curve()?
                .closest_parameter(snap.point(), Tolerance::NUMERICAL_VALIDATION)
                .ok()?
        } else {
            let (offset, direction) =
                self.drafting_view_line_relative_to(input.handles()?[0], pointer, rect)?;
            input.ratio_parameter_in_view(offset, direction)?
        };
        Some(edge_point::EdgePointCursor::new(parameter, snap))
    }
    pub(super) fn pick_arc_blend_handle(
        &self,
        pointer: Pos2,
        rect: Rect,
        input: &Input,
    ) -> Option<Handle> {
        if input.active_handle().is_some() {
            return None;
        }
        input
            .handles()?
            .into_iter()
            .zip([Handle::Start, Handle::Ratio, Handle::End])
            .filter_map(|(p, h)| {
                Some((
                    self.project_selection_point(p, rect)?.distance_sq(pointer),
                    h,
                ))
            })
            .filter(|(d, _)| *d <= 35. * 35.)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, h)| h)
    }
    pub(super) fn paint_arc_blend_handles(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: &Input,
    ) {
        let Some(points) = input.handles() else {
            return;
        };
        if input.active_handle() == Some(Handle::Ratio)
            && let (Some(a), Some(b)) = (
                self.project_selection_point(points[0], rect),
                self.project_selection_point(points[2], rect),
            )
        {
            painter.line_segment([a, b], Stroke::new(1., Color32::from_gray(140)));
        }
        for (point, handle) in points
            .into_iter()
            .zip([Handle::Start, Handle::Ratio, Handle::End])
        {
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
