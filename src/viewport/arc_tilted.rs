//! Tilt rectangle and final arc previews use the command's immutable getter.
use super::*;
impl Viewport {
    pub(super) fn paint_tilted_arc(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        getter: viboceros_command::arc_tilted_input::Input,
        output: Option<viboceros_command::arc_deform_input::Output>,
        cursor: Option<Point3>,
    ) {
        self.paint_draft_points(painter, rect, &[getter.center(), getter.anchor()]);
        let Some(point) = cursor else {
            return;
        };
        if let Some(corners) = getter.orientation_guide(point) {
            for i in 0..4 {
                if let (Some(a), Some(b)) = (
                    self.project_selection_point(corners[i], rect),
                    self.project_selection_point(corners[(i + 1) % 4], rect),
                ) {
                    painter.line_segment([a, b], Stroke::new(1., Color32::from_gray(130)));
                }
            }
        }
        if let Some(arc) = getter.preview(point) {
            self.paint_arc_preview(painter, rect, arc, output);
        }
    }
    pub(super) fn paint_arc_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        arc: viboceros_geometry::CircularArc3,
        output: Option<viboceros_command::arc_deform_input::Output>,
    ) {
        if let Ok(curve) = output.map_or_else(
            || {
                arc.to_nurbs()
                    .map_err(viboceros_command::CommandError::from)
            },
            |o| o.curve(arc),
        ) {
            let mut projected = ProjectedPrimitives::default();
            self.add_projected_nurbs_curve(&mut projected, rect, &curve);
            for segment in projected.segments {
                painter.line_segment(segment, Stroke::new(2., Color32::from_rgb(20, 115, 190)));
            }
        }
    }
}
