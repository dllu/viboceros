//! Refine a display-triangle hit against the actual surface and viewing line.
use super::*;
use viboceros_geometry::NurbsSurface;

impl Viewport {
    pub(super) fn refine_surface_pick(
        &self,
        surface: &NurbsSurface,
        mut uv: [Real; 2],
        pointer: Pos2,
        rect: Rect,
    ) -> Option<[Real; 2]> {
        let (origin, direction) = self.drafting_local_view_line(pointer, rect)?;
        let ray_frame =
            Frame3::try_from_normal(origin, direction, Tolerance::NUMERICAL_VALIDATION).ok()?;
        let residual = |point: Point3| -> Option<[Real; 2]> {
            let [x, y, z] = point.to_array();
            let local =
                Point3::try_new(x - self.target.x, y - self.target.y, z - self.target.z).ok()?;
            let [x, y, _] = ray_frame.coordinates_of(local).ok()?;
            Some([x, y])
        };
        let along = |vector: Vector3| {
            Some([
                vector.dot(ray_frame.x_axis().as_vector()).ok()?,
                vector.dot(ray_frame.y_axis().as_vector()).ok()?,
            ])
        };
        let domain_u = surface.domain_u();
        let domain_v = surface.domain_v();
        for _ in 0..12 {
            let (point, u, v) = surface.evaluate_with_derivatives(uv[0], uv[1]).ok()?;
            let [x, y] = residual(point)?;
            let distance = x.hypot(y);
            if distance == 0. {
                break;
            }
            let [ux, uy] = along(u)?;
            let [vx, vy] = along(v)?;
            let scale_u = ux.hypot(uy);
            let scale_v = vx.hypot(vy);
            let (ux, uy, vx, vy) = (ux / scale_u, uy / scale_u, vx / scale_v, vy / scale_v);
            let determinant = ux * vy - uy * vx;
            if !determinant.is_finite() || determinant.abs() < 1e-12 {
                break;
            }
            let step_u = (vx * y - vy * x) / determinant / scale_u;
            let step_v = (uy * x - ux * y) / determinant / scale_v;

            let next = [1., 0.5, 0.25, 0.125].into_iter().find_map(|step| {
                let next = [
                    (uv[0] + step * step_u).clamp(*domain_u.start(), *domain_u.end()),
                    (uv[1] + step * step_v).clamp(*domain_v.start(), *domain_v.end()),
                ];
                if next == uv {
                    return None;
                }
                let point = surface.evaluate(next[0], next[1]).ok()?;
                let [x, y] = residual(point)?;
                (x.hypot(y) < distance).then_some(next)
            });
            let Some(next) = next else { break };
            uv = next;
        }
        Some(uv)
    }
}
