//! Hover uses the cached visible mesh, including flat appearance and creases.
use super::*;
use viboceros_geometry::SubD;

impl Viewport {
    pub(super) fn subd_direction_marker(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        id: ObjectId,
        source_face: usize,
        staged: &SubD,
    ) -> Option<Marker> {
        let object = document.object(id)?;
        let Geometry::SubD(original) = object.geometry() else {
            return None;
        };
        let sign = if original.faces().get(source_face)? == staged.faces().get(source_face)? {
            1.
        } else {
            -1.
        };
        let display = self.display_cache.borrow_mut().get(
            object,
            document.tolerance(),
            document.subd_control_net(),
        );
        let mesh = display.mesh()?;
        let (_, face, point) = self.mesh_face_pick_with_point(pointer, rect, mesh)?;
        let point = point?;
        if *display.face_sources()?.get(face)? != source_face {
            return None;
        }
        let offset: usize = mesh.faces()[..face]
            .iter()
            .map(|f| if f.is_triangle() { 1 } else { 2 })
            .sum();
        let count = if mesh.faces()[face].is_triangle() {
            1
        } else {
            2
        };
        let p = NaVector3::from(point.to_array());
        let normals = display.normals();
        let mut best: Option<(Real, NaVector3<Real>)> = None;
        for triangle in offset..offset + count {
            let [a, b, c] = mesh
                .triangle_points(triangle)?
                .map(|p| NaVector3::from(p.to_array()));
            let (ab, ac, ap) = (b - a, c - a, p - a);
            let (d00, d01, d11) = (ab.dot(&ab), ab.dot(&ac), ac.dot(&ac));
            let determinant = d00 * d11 - d01 * d01;
            if determinant <= 0. {
                continue;
            }
            let v = (d11 * ap.dot(&ab) - d01 * ap.dot(&ac)) / determinant;
            let w = (d00 * ap.dot(&ac) - d01 * ap.dot(&ab)) / determinant;
            let u = 1. - v - w;
            if [u, v, w].iter().any(|x| *x < -1e-7 || *x > 1. + 1e-7) {
                continue;
            }
            let n = normals.get(triangle)?;
            let Some(n) = (n[0] * u + n[1] * v + n[2] * w).try_normalize(Real::EPSILON) else {
                continue;
            };
            // A folded quad can project inside both triangles. Use the
            // triangle containing the 3D hit, rather than the first projection.
            let error = (ap - ab * v - ac * w).norm_squared();
            if best.is_none_or(|(previous, _)| error < previous) {
                best = Some((error, n * sign));
            }
        }
        let (_, n) = best?;
        Some(Marker {
            object: id,
            point,
            axes: [
                None,
                None,
                Some(viboceros_geometry::Vector3::try_new(n.x, n.y, n.z).ok()?),
            ],
            surface: false,
        })
    }
}
