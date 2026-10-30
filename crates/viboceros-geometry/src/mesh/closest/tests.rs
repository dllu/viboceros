//! Analytic nearest points and exact-reference conditioning holdouts.
use super::*;
use crate::mesh::closest_point_on_triangle;
use crate::{MeshFace, MeshProjectionIndex, TriangleMesh};

pub(super) const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [1, 2, 0],
    [2, 0, 1],
    [0, 2, 1],
    [2, 1, 0],
    [1, 0, 2],
];

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}

#[test]
fn long_thin_interiors_retain_the_short_coordinate_in_every_vertex_order() {
    for length in [1e6, 1e8, 2f64.powi(40)] {
        let vertices = [p([0., 0., 0.]), p([length, 0., 0.]), p([length, 2., 0.])];
        let expected = p([length * 0.75, 0.5, 0.]);
        for order in ORDERS {
            let mesh = TriangleMesh::try_from_face_records(
                vertices.to_vec(),
                vec![MeshFace::Triangle(order.map(|i| i as u32))],
            )
            .unwrap();
            let index = MeshProjectionIndex::new(&mesh).unwrap();
            for height in [-3., 0., 3.] {
                let target = p([expected.x(), expected.y(), height]);
                for actual in [
                    mesh.closest_point_on_face(0, target).unwrap(),
                    index.closest_point(target).unwrap().unwrap(),
                ] {
                    assert!(
                        actual.distance_to(expected).unwrap() <= 1e-6,
                        "length={length}, order={order:?}, target={target:?}: {actual:?} != {expected:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn rotated_thin_triangles_keep_analytic_interior_feet() {
    // Integer orthogonal bases keep the expected dyadic foot independent of
    // the triangle query: u.v = u.n = v.n = 0.
    for (u, v, n) in [
        ([1., 0., 0.], [0., 1., 0.], [0., 0., 1.]),
        ([1., 1., 0.], [-1., 1., 2.], [1., -1., 1.]),
        ([2., -1., 2.], [1., 2., 0.], [-4., 2., 5.]),
    ] {
        for length in [2f64.powi(20), 2f64.powi(27)] {
            let origin = [16., -32., 8.];
            let at = |x, y, z| {
                p(std::array::from_fn(|i| {
                    u[i] * x + v[i] * y + n[i] * z + origin[i]
                }))
            };
            let vertices = [at(0., 0., 0.), at(length, 0., 0.), at(length, 2., 0.)];
            let expected = at(length * 0.75, 0.5, 0.);
            for order in ORDERS {
                let [a, b, c] = order.map(|i| vertices[i]);
                for height in [-0.5, 0., 0.5] {
                    let target = at(length * 0.75, 0.5, height);
                    let actual = closest_point_on_triangle(target, a, b, c).unwrap();
                    assert!(
                        actual.distance_to(expected).unwrap() <= 1e-6,
                        "length={length}, order={order:?}, target={target:?}: {actual:?} != {expected:?}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "manual timing and error diagnostic; no timing gate"]
fn closest_query_timing() {
    for (name, length, rotated) in [
        ("ordinary", 4., false),
        ("thin-axis", 1e8, false),
        ("thin-rotated", 1e8, true),
    ] {
        let at = |x, y, z| {
            p(if rotated {
                [x - y + z, x + y - z, 2. * y + z]
            } else {
                [x, y, z]
            })
        };
        let [a, b, c] = [at(0., 0., 0.), at(length, 0., 0.), at(length, 2., 0.)];
        let queries = (0..256)
            .map(|i| {
                let x = length * (0.5 + (i % 8) as f64 / 32.);
                let y = 0.125 + (i / 8) as f64 / 64.;
                (at(x, y, 0.5), at(x, y, 0.))
            })
            .collect::<Vec<_>>();
        for lap in 0..3 {
            let start = std::time::Instant::now();
            let mut error = 0f64;
            for &(q, expected) in &queries {
                let actual = closest_point_on_triangle(std::hint::black_box(q), a, b, c).unwrap();
                error = error.max(std::hint::black_box(actual).distance_to(expected).unwrap());
            }
            println!(
                "CLOSEST_TIMING name={name} lap={lap} queries={} microseconds={} error={error}",
                queries.len(),
                start.elapsed().as_micros()
            );
        }
    }
}
