//! Authored rectangular walls around two openings separated by a narrow strip.
use viboceros_geometry::{MeshFace, Point3, Tolerance, TriangleMesh};

pub fn narrow_openings(scale: f64) -> TriangleMesh {
    let gap = 2.0_f64.powi(-43);
    let rings = [
        [[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
        [[0.1, 0.2], [0.5, 0.2], [0.5, 0.8], [0.1, 0.8]],
        [[0.5 + gap, 0.2], [0.9, 0.2], [0.9, 0.8], [0.5 + gap, 0.8]],
    ];
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for (ring_index, ring) in rings.into_iter().enumerate() {
        let base = vertices.len() as u32;
        for z in [0., 2.] {
            vertices.extend(ring.map(|[x, y]| Point3::try_new(scale * x, scale * y, z).unwrap()));
        }
        for side in 0..4 {
            let next = (side + 1) % 4;
            faces.push(MeshFace::Quad(if ring_index == 0 {
                [base + side, base + next, base + next + 4, base + side + 4]
            } else {
                [base + side, base + side + 4, base + next + 4, base + next]
            }));
        }
    }
    let colors = (0..vertices.len())
        .map(|i| [i as u8, 100, 200, 255])
        .collect();
    let uv = vertices
        .iter()
        .map(|p| [p.x() / scale, p.y() / scale])
        .collect();
    TriangleMesh::try_new_faces(vertices, faces, Tolerance::MESH_VALIDATION)
        .unwrap()
        .try_with_vertex_colors(Some(colors))
        .unwrap()
        .try_with_texture_coordinates(Some(uv))
        .unwrap()
}

pub fn expected_volume(scale: f64) -> f64 {
    (0.52 + 0.6 * 2.0_f64.powi(-43)) * scale * scale * 2.
}
