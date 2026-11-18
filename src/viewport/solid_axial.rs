//! Circular solid drafts from the current point getter.
use super::*;
use viboceros_command::solid_axial::{Input, Shape};

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
            let u = wall.domain_u();
            let v = wall.domain_v();
            let shape = definition.kind.shape();
            let bands = [
                (shape != Shape::Cone).then_some(*v.start()),
                (shape != Shape::TruncatedCone || definition.end_radius > 0.).then_some(*v.end()),
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
