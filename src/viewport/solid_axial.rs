//! Circular solid drafts from the current point getter.
use super::*;
use viboceros_command::solid_axial::{Input, Shape};
use viboceros_geometry::NurbsSurface;

impl Viewport {
    pub(super) fn paint_solid_axial_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: Input,
        cursor: Option<Point3>,
        tolerance: Tolerance,
    ) {
        let definition = input.preview_definition(cursor);
        if let Some(mesh) = definition
            .filter(|d| d.kind.is_mesh())
            .filter(|d| {
                d.mesh_face_count()
                    .is_some_and(|count| count <= MAX_DRAFT_MESH_FACES)
            })
            .and_then(|d| d.mesh(tolerance).ok())
        {
            self.paint_mesh_draft(painter, rect, &mesh);
            return;
        }
        if let Some(definition) = definition
            && let Ok(wall) = definition.wall()
        {
            let shape = definition.kind.shape();
            for wall in std::iter::once(wall).chain(definition.second_wall()) {
                self.paint_axial_wall(
                    painter,
                    rect,
                    &wall,
                    shape != Shape::Cone,
                    shape != Shape::TruncatedCone || definition.end_radius > 0.,
                );
            }
        } else {
            let base = cursor
                .and_then(|point| input.preview_circle(point))
                .or(input.circle);
            for rim in base.into_iter().chain(input.preview_tube_circle(cursor)) {
                if let Ok(circle) = Circle3::try_from_frame(
                    rim.frame.origin(),
                    rim.radius,
                    rim.frame.x_axis(),
                    rim.frame.z_axis(),
                    Tolerance::NUMERICAL_VALIDATION,
                ) {
                    self.paint_curve_draft(painter, rect, &circle);
                }
            }
        }
    }

    fn paint_axial_wall(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        wall: &NurbsSurface,
        draw_start: bool,
        draw_end: bool,
    ) {
        let u = wall.domain_u();
        let v = wall.domain_v();
        let bands = [
            draw_start.then_some(*v.start()),
            draw_end.then_some(*v.end()),
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
    }
}
