//! Conservative coordinate-exact boundary comparison for convex faces in
//! axis-aligned planes. Opposite orientations stay in separate layers, so
//! overlapping caps cannot cancel. Unsupported/uncertain faces return None.
use std::collections::BTreeMap;
use viboceros_geometry::TriangleMesh;

type Point = [u64; 3];
type Key = (usize, u64, bool, Point, Point);
fn bits(x: f64) -> u64 {
    if x == 0. { 0 } else { x.to_bits() }
}

pub fn signature(mesh: &TriangleMesh) -> Option<BTreeMap<Key, i32>> {
    let mut edges = BTreeMap::new();
    for face in mesh.faces() {
        let points = face
            .indices()
            .iter()
            .map(|&i| mesh.vertices()[i as usize].to_array())
            .collect::<Vec<_>>();
        let axis = (0..3).find(|&k| points.iter().all(|p| p[k] == points[0][k]))?;
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut orientation = None;
        for i in 0..points.len() {
            let (a, b, c) = (
                points[i],
                points[(i + 1) % points.len()],
                points[(i + 2) % points.len()],
            );
            let (x, y, z, w) = (b[u] - a[u], b[v] - a[v], c[u] - a[u], c[v] - a[v]);
            let cross = x.mul_add(w, -y * z);
            if !cross.is_finite()
                || cross.abs() <= 32. * f64::EPSILON * ((x * w).abs() + (y * z).abs())
            {
                return None;
            }
            let positive = cross > 0.;
            if orientation.is_some_and(|sign| sign != positive) {
                return None;
            }
            orientation = Some(positive);
        }
        let orientation = orientation?;
        for i in 0..points.len() {
            let a = points[i].map(bits);
            let b = points[(i + 1) % points.len()].map(bits);
            let (first, second, sign) = if a < b { (a, b, 1) } else { (b, a, -1) };
            *edges
                .entry((axis, bits(points[0][axis]), orientation, first, second))
                .or_default() += sign;
        }
    }
    edges.retain(|_, count| *count != 0);
    Some(edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{MeshFace, Point3};
    fn mesh(points: &[[f64; 3]], faces: Vec<MeshFace>) -> TriangleMesh {
        TriangleMesh::try_from_face_records(
            points
                .iter()
                .copied()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            faces,
        )
        .unwrap()
    }
    #[test]
    fn an_interior_fan_preserves_the_oriented_boundary_certificate() {
        let points = [
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [0.5, 0.5, 0.],
        ];
        let quad = mesh(&points, vec![MeshFace::Quad([0, 1, 2, 3])]);
        let fan = mesh(
            &points,
            vec![
                MeshFace::Triangle([0, 1, 4]),
                MeshFace::Triangle([1, 2, 4]),
                MeshFace::Triangle([2, 3, 4]),
                MeshFace::Triangle([3, 0, 4]),
            ],
        );
        assert_eq!(signature(&quad), signature(&fan));
    }
    #[test]
    fn opposite_or_duplicate_faces_cannot_cancel_into_a_valid_single_layer() {
        let p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        let a = mesh(&p, vec![MeshFace::Triangle([0, 1, 2])]);
        let opposed = mesh(
            &p,
            vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 1])],
        );
        let duplicate = mesh(&p, vec![MeshFace::Triangle([0, 1, 2]); 2]);
        assert_ne!(signature(&a), signature(&opposed));
        assert_ne!(signature(&a), signature(&duplicate));
        assert!(!signature(&opposed).unwrap().is_empty());
    }
    #[test]
    fn unsupported_or_degenerate_faces_do_not_receive_certificates() {
        for p in [
            [[0., 0., 0.], [1., 1., 0.], [0., 1., 1.]],
            [[0., 0., 0.], [1., 1., 0.], [2., 2., 0.]],
            [[0., 0., 0.], [1e308, 0., 0.], [0., 1e308, 0.]],
        ] {
            assert!(signature(&mesh(&p, vec![MeshFace::Triangle([0, 1, 2])])).is_none());
        }
    }
}
