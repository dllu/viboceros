//! Focused component-extraction benchmark, not a whole-kernel comparison.
use std::hint::black_box;
use std::time::Instant;
use viboceros_geometry::{Point3, Tolerance, TriangleMesh};

fn main() {
    for count in [500usize, 2_000, 8_000, 32_000] {
        let mut vertices = Vec::with_capacity(3 * count);
        let mut faces = Vec::with_capacity(count);
        for i in 0..count {
            let x = (3 * i) as f64;
            for [x, y, z] in [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]] {
                vertices.push(Point3::try_new(x, y, z).unwrap());
            }
            faces.push([(3 * i) as u32, (3 * i + 1) as u32, (3 * i + 2) as u32]);
        }
        let mesh = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();
        let started = Instant::now();
        for _ in 0..5 {
            assert_eq!(black_box(mesh.disjoint_pieces()).len(), count);
        }
        println!(
            "{count} components: {:.3} ms per extraction",
            started.elapsed().as_secs_f64() * 200.
        );
    }
}
