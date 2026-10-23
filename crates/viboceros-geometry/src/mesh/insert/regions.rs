//! Tessellate one inserted boundary while preserving its oriented vertex order.
use super::*;

pub(super) fn tile(
    points: Vec<Point3>,
    tolerance: Tolerance,
) -> Result<Vec<MeshFace>, GeometryError> {
    let projected = project_mesh_hole_boundary(&points)?
        .ok_or_else(|| invalid("degenerate inserted mesh region"))?;
    if points.len() == 3 {
        return Ok(vec![MeshFace::Triangle([0, 1, 2])]);
    }
    let strict_quad = points.len() == 4
        && (0..4).all(|i| {
            mesh_hole_cross(projected[i], projected[(i + 1) % 4], projected[(i + 2) % 4])
                > 64. * Real::EPSILON
        });
    if strict_quad {
        return Ok(vec![MeshFace::Quad([0, 1, 2, 3])]);
    }
    let triangles = triangulate_projected_mesh_hole(&projected)?
        .ok_or_else(|| invalid("cannot triangulate inserted mesh region"))?;
    let region = TriangleMesh::try_new_faces(
        points,
        triangles.into_iter().map(MeshFace::Triangle).collect(),
        tolerance,
    )?;
    let (region, _) = region.quadrangulate_preserving_triangle_quality(tolerance)?;
    Ok(region.faces().to_vec())
}
