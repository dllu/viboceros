//! Cone and cylinder drafts from the current point getter.
use super::*;
use viboceros_command::solid_axial::{Input, Kind};

impl Viewport {
    pub(super) fn paint_solid_axial_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: Input,
        cursor: Option<Point3>,
    ) {
        if let Some(definition) = input.preview_definition(cursor)
            && let Ok(wall) = definition.wall()
        {
            let u = wall.domain_u();
            let v = wall.domain_v();
            let bands = [
                if definition.kind == Kind::Cylinder {
                    Some(*v.start())
                } else {
                    None
                },
                Some(*v.end()),
            ];
            for v in bands.into_iter().flatten() {
                if let Ok(rim) = wall.isocurve_u(v) {
                    self.paint_curve_draft(painter, rect, &rim);
                }
            }
            for fraction in [0., 0.25, 0.5, 0.75] {
                let u = (u.end() - u.start()).mul_add(fraction, *u.start());
                if let Ok(line) = wall.isocurve_v(u) {
                    self.paint_curve_draft(painter, rect, &line);
                }
            }
        } else if let Some(base) = cursor
            .and_then(|point| input.preview_circle(point))
            .or(input.circle)
            && let Ok(circle) = Circle3::try_from_frame(
                base.frame.origin(),
                base.radius,
                base.frame.x_axis(),
                base.frame.z_axis(),
                Tolerance::NUMERICAL_VALIDATION,
            )
        {
            self.paint_curve_draft(painter, rect, &circle);
        }
    }
}
