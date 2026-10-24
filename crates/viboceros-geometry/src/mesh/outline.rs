//! Mesh coverage outlines using iOverlay's integer polygon arrangement.
use super::*;
use crate::transform::orientation::determinant_sign;
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::overlay::{FloatOverlay, OverlayOptions},
};
use std::cmp::Ordering;

/// A geometric projection, independent of viewport or document state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshOutlineProjection {
    /// Orthogonal projection onto the frame's XY plane.
    Parallel(Frame3),
    /// Central projection from an eye onto the frame's XY plane.
    /// A triangle crossing the plane through the eye parallel to this plane
    /// has an unbounded image and cannot be returned as a finite polyline.
    Perspective { plane: Frame3, eye: Point3 },
}

impl TriangleMesh {
    /// Boundaries of the union of the projected face images, including holes.
    /// Face orientation, unwelded seams and overlapping sheets do not cancel
    /// coverage. Output contours retain collinear boundary vertices and have
    /// outer CCW / inner CW orientation in the projection frame. Sources and
    /// their stored channels remain unchanged. Empty area produces no curves.
    ///
    /// The i64 overlay arrangement avoids a pairwise face Boolean loop. Its
    /// adaptive integer grid is finer than binary64 coordinate precision for
    /// ordinary model extents; this is not an exact-arithmetic union API.
    pub fn outlines(
        &self,
        projection: MeshOutlineProjection,
        validation: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        if self.triangles.len() > 1_000_000 {
            return Err(GeometryError::TooManyMeshFaces);
        }
        let (plane, eye) = match projection {
            MeshOutlineProjection::Parallel(plane) => (plane, None),
            MeshOutlineProjection::Perspective { plane, eye } => {
                let eye = plane.coordinates_of(eye)?;
                if eye[2] == 0. {
                    return Err(GeometryError::MeshOutlineCrossesEyePlane);
                }
                (plane, Some(eye))
            }
        };
        // Reuse OpenNURBS' shortest spatial diagonal instead of the renderer's
        // fixed AC triangles. Native perspective outlines have an additional
        // frustum-dependent quad coverage change; that gap is recorded in
        // the independent SDK captures and must not be hidden by flattening.
        let triangulated = self
            .faces
            .iter()
            .any(|face| face.is_quad())
            .then(|| self.triangulate_quads(Tolerance::MESH_VALIDATION))
            .transpose()?;
        let triangles = triangulated
            .as_ref()
            .map_or(self.triangles(), |(mesh, _)| mesh.triangles());
        // Only referenced vertices participate: an unused vertex on the eye
        // plane must not invalidate an otherwise bounded projection.
        let mut projected = vec![None; self.vertices.len()];
        let mut paths = Vec::with_capacity(self.triangles.len());
        for triangle in triangles {
            let mut path = Vec::with_capacity(3);
            let mut side = None;
            for &raw in triangle {
                let index = raw as usize;
                let (point, depth) = match projected[index] {
                    Some(value) => value,
                    None => {
                        let point = self.vertices[index];
                        let value = if let Some(eye) = eye {
                            let [x, y, z] = plane.coordinates_of(point)?;
                            let depth = eye[2] - z;
                            if !depth.is_finite() {
                                return Err(GeometryError::MeshOutlineCoordinateRange);
                            }
                            if depth == 0. {
                                return Err(GeometryError::MeshOutlineCrossesEyePlane);
                            }
                            let t = eye[2] / depth;
                            if !t.is_finite() || t == 0. {
                                return Err(GeometryError::MeshOutlineCoordinateRange);
                            }
                            (
                                [t.mul_add(x - eye[0], eye[0]), t.mul_add(y - eye[1], eye[1])],
                                depth,
                            )
                        } else {
                            (plane.projected_coordinates_of(point)?, 1.)
                        };
                        // iOverlay's documented binary64 input range. Reject
                        // unsupported values before its infallible adapter.
                        if value
                            .0
                            .iter()
                            .any(|x| !x.is_finite() || x.abs() > 2_f64.powi(500))
                        {
                            return Err(GeometryError::MeshOutlineCoordinateRange);
                        }
                        projected[index] = Some(value);
                        value
                    }
                };
                if side.is_some_and(|s| s != depth.is_sign_positive()) {
                    return Err(GeometryError::MeshOutlineCrossesEyePlane);
                }
                side = Some(depth.is_sign_positive());
                path.push(point);
            }
            match orientation(&path) {
                Ordering::Equal => continue,
                Ordering::Less => path.swap(1, 2),
                Ordering::Greater => {}
            }
            paths.push(path);
        }
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let mut options = OverlayOptions::<f64, i64>::default();
        options.preserve_input_collinear = true;
        options.preserve_output_collinear = true;
        let mut overlay =
            FloatOverlay::<[f64; 2], i64>::from_subj_custom(&paths, options, Default::default());
        let shapes = overlay.overlay(OverlayRule::Subject, FillRule::NonZero);
        shapes
            .into_iter()
            .flatten()
            .map(|path| {
                let mut points = path
                    .into_iter()
                    .map(|[x, y]| plane.point_at([x, y, 0.]))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(first) = points.first().copied() {
                    points.push(first);
                }
                Polyline3::try_new(points, validation)
            })
            .collect()
    }
}

fn orientation(path: &[[f64; 2]]) -> Ordering {
    let [a, b, c] = [path[0], path[1], path[2]];
    let left = (b[0] - a[0]) * (c[1] - a[1]);
    let right = (b[1] - a[1]) * (c[0] - a[0]);
    let determinant = left - right;
    if determinant.abs() > (left.abs() + right.abs()) * (8. * f64::EPSILON) {
        determinant.total_cmp(&0.)
    } else {
        // Shared exact binary64 determinant handles cancellation and underflow.
        determinant_sign([[a[0], a[1], 1.], [b[0], b[1], 1.], [c[0], c[1], 1.]])
    }
}
