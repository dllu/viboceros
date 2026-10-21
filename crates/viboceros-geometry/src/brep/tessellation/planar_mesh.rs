//! Editable planar regions reuse constrained patches and rectangular grids.
use super::*;
use crate::{Frame3, MAX_MESH_PATCH_POINTS, MeshPatchOptions, Polyline3};

impl Brep {
    /// Mesh each planar face with sixteen samples per trim span. Rectangular
    /// regions use a four-by-four quad grid; other regions share constrained
    /// patch triangulation, including their inner boundaries.
    pub fn planar_polygon_mesh(&self, tolerance: Tolerance) -> Result<TriangleMesh, GeometryError> {
        let mut parts = Vec::new();
        let mut samples = 0usize;
        for (index, face) in self.faces.iter().enumerate() {
            let plane = planar_surface_plane(&face.surface, tolerance)?
                .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: index })?;
            samples = samples
                .checked_add(face.loops.len())
                .filter(|n| *n <= MAX_MESH_PATCH_POINTS)
                .ok_or(GeometryError::MeshPatchLimit)?;
            for boundary in &face.loops {
                samples = boundary.trims.iter().try_fold(samples, |total, trim| {
                    trim.curve
                        .spans()
                        .count()
                        .checked_mul(16)
                        .and_then(|n| total.checked_add(n))
                        .filter(|n| *n <= MAX_MESH_PATCH_POINTS)
                        .ok_or(GeometryError::MeshPatchLimit)
                })?;
            }
            let loops = face
                .loops
                .iter()
                .map(|boundary| {
                    let mut points = sample_trim_loop(boundary, 16)?
                        .into_iter()
                        .map(|uv| face.surface.evaluate(uv.x(), uv.y()))
                        .collect::<Result<Vec<_>, _>>()?;
                    if points.len() < 3 {
                        return Err(GeometryError::UnsupportedBrepTrimTessellation { face: index });
                    }
                    points.push(points[0]);
                    Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let outer = face
                .loops
                .iter()
                .position(|l| l.loop_type == BrepLoopType::Outer)
                .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: index })?;
            let holes = loops
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != outer)
                .map(|(_, p)| p.clone())
                .collect::<Vec<_>>();
            let mut mesh = if holes.is_empty() {
                rectangular_grid(&loops[outer], tolerance)?
            } else {
                None
            }
            .map(Ok)
            .unwrap_or_else(|| {
                TriangleMesh::try_patch(
                    &[],
                    &[],
                    &holes,
                    MeshPatchOptions {
                        outer_boundary: Some(&loops[outer]),
                        ..Default::default()
                    },
                    tolerance,
                )
            })?;
            let dot = mesh.polygon_face_normals()?[0]
                .as_vector()
                .dot(plane.normal.as_vector())?;
            if (dot < 0.) != face.reversed {
                mesh = mesh.reversed();
            }
            parts.push(mesh);
        }
        TriangleMesh::try_append(&parts.iter().collect::<Vec<_>>())
    }
}

fn rectangular_grid(
    boundary: &Polyline3,
    tolerance: Tolerance,
) -> Result<Option<TriangleMesh>, GeometryError> {
    let reduced = boundary.try_reduced(tolerance.absolute())?;
    let [a, b, c, d, _] = reduced.vertices() else {
        return Ok(None);
    };
    let x = a.vector_to(*b)?;
    let y = a.vector_to(*d)?;
    let x_length = x.length()?;
    let y_length = y.length()?;
    let xn = x.normalized_nonzero()?;
    let yn = y.normalized_nonzero()?;
    if xn.as_vector().dot(yn.as_vector())?.abs() > tolerance.angular()
        || b.translated(y)?.distance_to(*c)? > tolerance.absolute()
    {
        return Ok(None);
    }
    let frame = Frame3::try_from_unit_directions(*a, xn, yn, tolerance)?;
    TriangleMesh::try_plane_grid(
        frame,
        [0., x_length],
        [0., y_length],
        4,
        4,
        Tolerance::MESH_VALIDATION,
    )
    .map(Some)
}
