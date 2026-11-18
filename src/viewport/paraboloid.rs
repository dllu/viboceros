//! Live paraboloid wall, rim and focus marker.
use super::*;
use viboceros_command::paraboloid::Input;

impl Viewport {
    pub(super) fn paint_paraboloid_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: Input,
        cursor: Option<Point3>,
    ) {
        for point in input.markers().into_iter().flatten() {
            self.paint_draft_points(painter, rect, &[point]);
        }
        let Some(definition) = cursor.and_then(|point| input.preview(point)) else {
            return;
        };
        let Ok(wall) = definition.wall() else {
            return;
        };
        let u = wall.domain_u();
        let v = wall.domain_v();
        if let Ok(rim) = wall.isocurve_u(*v.end()) {
            self.paint_curve_draft(painter, rect, &rim);
        }
        for fraction in [0.0, 0.25, 0.5, 0.75] {
            let parameter = (u.end() - u.start()).mul_add(fraction, *u.start());
            if let Ok(meridian) = wall.isocurve_v(parameter) {
                self.paint_curve_draft(painter, rect, &meridian);
            }
        }
        if let Some(focus) = definition.focus_marker() {
            self.paint_draft_points(painter, rect, &[focus]);
        }
    }
}
