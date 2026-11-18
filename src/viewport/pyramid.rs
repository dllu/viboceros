//! Polygon rim and apex wires drawn from the current Pyramid getter.
use super::*;
use viboceros_command::pyramid::Input;

impl Viewport {
    pub(super) fn paint_pyramid_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: Input,
        point: Option<Point3>,
    ) {
        let Some((base, apex)) = input.preview(point) else {
            return;
        };
        let Ok(rim) = base.preview_rim(MAX_DRAFT_MESH_FACES / 2) else {
            return;
        };
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        for (&start, &end) in rim.iter().zip(rim.iter().cycle().skip(1)) {
            if let Some(segment) = self.project_segment(start, end, rect) {
                painter.line_segment(segment, stroke);
            }
            if let Some(segment) = apex.and_then(|apex| self.project_segment(start, apex, rect)) {
                painter.line_segment(segment, stroke);
            }
        }
    }
}
