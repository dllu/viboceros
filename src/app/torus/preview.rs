//! Live base and tube previews, using the accepted circle and candidate size.
use super::*;

impl Session {
    pub(in crate::app) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }
    pub(in crate::app) fn mesh_preview(&self) -> Option<(Definition, Preferences)> {
        self.mesh_preview
    }
    fn circle(&mut self, frame: Frame3, radius: f64) {
        if let Ok(curve) = Circle3::try_new(
            frame.origin(),
            radius,
            frame.z_axis(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .and_then(Circle3::to_nurbs)
        {
            self.preview.push(curve);
        }
    }
    fn solid_isocurves(&mut self, definition: Definition) {
        let Ok(surface) = definition.surface() else {
            return;
        };
        let u_end = *surface.domain_u().end();
        let v_end = *surface.domain_v().end();
        for fraction in [0., 0.25, 0.5, 0.75] {
            self.preview.extend(surface.isocurve_u(fraction * v_end));
            self.preview.extend(surface.isocurve_v(fraction * u_end));
        }
    }

    pub(super) fn refresh(&mut self, point: Option<Point3>) {
        self.preview.clear();
        self.mesh_preview = None;
        let mut copy = self.input;
        let result = if let Some(point) = point {
            copy.point(point)
        } else {
            copy.enter()
        };
        if result.is_ok()
            && let Some(d) = copy.definition
        {
            if self.input.kind == Kind::Solid {
                self.solid_isocurves(d);
                return;
            }
            let p = self.input.preferences;
            if self.input.kind == Kind::Mesh
                && p.vertical_faces * p.around_faces <= crate::viewport::MAX_DRAFT_MESH_FACES
            {
                self.mesh_preview = Some((d, p));
                return;
            }
            let frame = d.circle.frame;
            let major = d.major_radius();
            self.circle(frame, major);
            self.circle(frame, major + d.tube_radius);
            self.circle(frame, (major - d.tube_radius).abs());
            if let Ok(origin) = frame.point_at([major, 0., 0.])
                && let Ok(tube) = Frame3::try_from_directions(
                    origin,
                    frame.x_axis().as_vector(),
                    frame.z_axis().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
            {
                self.circle(tube, d.tube_radius);
            }
        } else if let Some(c) = point
            .and_then(|p| self.input.preview_circle(p))
            .or(self.input.circle)
        {
            self.circle(c.frame, c.radius);
        }
    }
}
