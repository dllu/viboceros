//! Retained quads can have a flat corner while their oriented boundary has area.
use super::*;

pub(super) fn validate(mesh: &TriangleMesh, tolerance: Tolerance) -> Result<(), GeometryError> {
    for (index, &face) in mesh.faces.iter().enumerate() {
        let Err(error) = face_records::validate_face(&mesh.vertices, face, index, tolerance) else {
            continue;
        };
        let MeshFace::Quad(corners) = face else {
            return Err(error);
        };
        let points = corners.map(|i| mesh.vertices[i as usize]);
        if points
            .iter()
            .enumerate()
            .any(|(i, p)| points[..i].iter().any(|q| p.is_near(*q, tolerance)))
        {
            return Err(error);
        }
        let Some(projected) = project_mesh_hole_boundary(&points)? else {
            return Err(error);
        };
        // Weak convexity permits a collinear corner, but never a folded,
        // self-crossing or backtracking polygon. A nonzero projected boundary
        // is already required by project_mesh_hole_boundary.
        if (0..4).any(|i| {
            mesh_hole_cross(projected[i], projected[(i + 1) % 4], projected[(i + 2) % 4])
                < -64. * Real::EPSILON
        }) {
            return Err(error);
        }
    }
    Ok(())
}
