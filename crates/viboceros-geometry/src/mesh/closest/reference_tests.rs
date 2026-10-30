//! Independent exact arithmetic covers region boundaries and finite-range loss.
use super::*;
use crate::mesh::closest_point_on_triangle;

fn point(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn close(actual: Point3, expected: Point3, context: &str) {
    for (a, b) in actual.to_array().into_iter().zip(expected.to_array()) {
        let ulp = [b.next_down(), b.next_up()]
            .into_iter()
            .filter(|x| x.is_finite())
            .map(|x| (x - b).abs())
            .fold(0., f64::max);
        assert!(
            (a - b).abs() <= 64. * ulp,
            "{context}: {actual:?} != {expected:?}; coordinate bound {}",
            64. * ulp
        );
    }
}

#[test]
fn all_closed_voronoi_regions_match_exact_points_across_scales_and_orders() {
    for scale in [
        2f64.powi(-700),
        2f64.powi(-20),
        1.,
        2f64.powi(20),
        2f64.powi(700),
    ] {
        let at = |p: [f64; 3]| point(p.map(|x| x * scale));
        let vertices = [[0., 0., 0.], [4., 0., 0.], [4., 2., 0.]].map(at);
        for order in super::tests::ORDERS {
            let [a, b, c] = order.map(|i| vertices[i]);
            for q in [
                [-1., -1., 1.],
                [5., -1., 1.],
                [5., 3., 1.],
                [2., -1., 1.],
                [1., 2., 1.],
                [5., 1., 1.],
                [2., 0., 1.],
                [2., 1., 1.],
                [4., 1., 1.],
                [3., 0.5, 1.],
                [0., 0., 0.],
                [4., 0., 0.],
                [4., 2., 0.],
            ] {
                let target = at(q);
                let expected = exact_triangle(target, a, b, c).unwrap();
                close(
                    closest_point_on_triangle(target, a, b, c).unwrap(),
                    expected,
                    &format!("scale={scale}, order={order:?}, q={q:?}"),
                );
            }
        }
    }
}

#[test]
fn dyadic_conditioning_holdouts_match_exact_reference() {
    let mut state = 0x274f_46ad_9c02_317bu64;
    let mut number = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 32) as i32 % 257) as f64 / 32.
    };
    let mut checked = 0;
    for exponent in [-700, -80, -20, 0, 20, 80, 700] {
        let scale = 2f64.powi(exponent);
        for aspect in [1., 2f64.powi(-12), 2f64.powi(-28), 2f64.powi(-48)] {
            for sample in 0..8 {
                let origin = std::array::from_fn::<_, 3, _>(|_| number() * scale);
                let u = std::array::from_fn::<_, 3, _>(|_| number() * scale);
                let v = std::array::from_fn::<_, 3, _>(|_| number() * scale * aspect);
                let vertices = [
                    point(origin),
                    point(std::array::from_fn(|i| origin[i] + u[i])),
                    point(std::array::from_fn(|i| origin[i] + u[i] + v[i])),
                ];
                let target = if sample < 4 {
                    point(std::array::from_fn(|i| origin[i] + number() * scale))
                } else {
                    // Uniform box queries almost never exercise the interior
                    // of a thin triangle. Place half the holdouts close to an
                    // interior barycentric point, with a small normal offset.
                    let [a, b, c] = vertices.map(Point3::to_array);
                    let first = std::array::from_fn::<_, 3, _>(|i| (b[i] - a[i]) / scale);
                    let second = std::array::from_fn::<_, 3, _>(|i| (c[i] - a[i]) / scale);
                    let normal = std::array::from_fn::<_, 3, _>(|i| {
                        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
                        first[j] * second[k] - first[k] * second[j]
                    });
                    let norm = normal[0].hypot(normal[1]).hypot(normal[2]);
                    point(std::array::from_fn(|i| {
                        let offset = if norm == 0. {
                            0.
                        } else {
                            normal[i] / norm * (scale * aspect * 0.125)
                        };
                        a[i] + (b[i] - a[i]) * 0.375 + (c[i] - a[i]) * 0.25 + offset
                    }))
                };
                let expected =
                    exact_triangle(target, vertices[0], vertices[1], vertices[2]).unwrap();
                for order in super::tests::ORDERS {
                    let [a, b, c] = order.map(|i| vertices[i]);
                    close(
                        closest_point_on_triangle(target, a, b, c).unwrap(),
                        expected,
                        &format!(
                            "exponent={exponent}, aspect={aspect}, vertices={vertices:?}, target={target:?}"
                        ),
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 1344);
}
