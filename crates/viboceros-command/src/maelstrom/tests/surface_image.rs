//! Samples physical patches in both directions. Native surface fitting can
//! move parameters tangentially; boundary witnesses stay on matching boundaries.
use super::*;

fn sample(s: &NurbsSurface, u: Real, v: Real) -> Point3 {
    s.evaluate(s.parameter_at_u(u).unwrap(), s.parameter_at_v(v).unwrap())
        .unwrap()
}

fn witness(
    s: &NurbsSurface,
    target: Point3,
    fractions: [Real; 2],
    epsilon: Real,
    label: &str,
) -> Real {
    let bounds = [s.domain_u(), s.domain_v()];
    let fixed = fractions.map(|f| f == 0. || f == 1.);
    let mut uv = [
        s.parameter_at_u(fractions[0]).unwrap(),
        s.parameter_at_v(fractions[1]).unwrap(),
    ];
    let mut best = s.evaluate(uv[0], uv[1]).unwrap();
    for _ in 0..20 {
        if best.distance_to(target).unwrap() <= epsilon * 0.01 {
            break;
        }
        let (point, du, dv) = s.evaluate_with_derivatives(uv[0], uv[1]).unwrap();
        let residual = point.vector_to(target).unwrap();
        let a = du.dot(du).unwrap();
        let b = du.dot(dv).unwrap();
        let c = dv.dot(dv).unwrap();
        let x = du.dot(residual).unwrap();
        let y = dv.dot(residual).unwrap();
        let delta = match fixed {
            [true, true] => break,
            [true, false] => [0., y / c],
            [false, true] => [x / a, 0.],
            [false, false] => {
                let determinant = a * c - b * b;
                [(c * x - b * y) / determinant, (a * y - b * x) / determinant]
            }
        };
        assert!(
            delta.iter().all(|d| d.is_finite()),
            "{label}: singular witness refinement"
        );
        let mut accepted = false;
        for i in 0..8 {
            let next = std::array::from_fn(|j| {
                (uv[j] + delta[j] * 0.5_f64.powi(i)).clamp(*bounds[j].start(), *bounds[j].end())
            });
            let point = s.evaluate(next[0], next[1]).unwrap();
            if point.distance_to(target).unwrap() < best.distance_to(target).unwrap() {
                uv = next;
                best = point;
                accepted = true;
                break;
            }
        }
        if !accepted {
            break;
        }
    }
    near(best, target, epsilon, label);
    best.distance_to(target).unwrap()
}

pub(super) fn compare(
    actual: &Geometry,
    expected: &Value,
    source: &Geometry,
    source_record: &Value,
    epsilon: Real,
    label: &str,
) {
    let Geometry::Brep(actual) = actual else {
        panic!("expected a B-rep")
    };
    let Geometry::Brep(source) = source else {
        panic!("expected source B-rep")
    };
    assert!(actual.is_closed() && actual.is_solid(), "{label}");
    assert_eq!(
        actual.faces().len(),
        expected["surfaces"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.vertices().len(),
        expected["vertices"].as_array().unwrap().len()
    );
    for vertex in expected["vertices"].as_array().unwrap() {
        assert!(
            actual
                .vertices()
                .iter()
                .any(|v| v.point().distance_to(p(vertex)).unwrap() <= epsilon),
            "{label}: vertex"
        );
    }
    for (index, record) in source_record["surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let original = surface(record);
        let mut correspondence = None;
        for (k, face) in source.faces().iter().enumerate() {
            for swap in [false, true] {
                for flip_u in [false, true] {
                    for flip_v in [false, true] {
                        let uv = |mut u: Real, mut v: Real| {
                            if swap {
                                std::mem::swap(&mut u, &mut v);
                            }
                            [
                                if flip_u { 1. - u } else { u },
                                if flip_v { 1. - v } else { v },
                            ]
                        };
                        if [[0., 0.], [1., 0.], [0., 1.], [1., 1.]]
                            .into_iter()
                            .all(|[u, v]| {
                                let [a, b] = uv(u, v);
                                sample(&original, u, v)
                                    .distance_to(sample(face.surface(), a, b))
                                    .unwrap()
                                    < 1e-10
                            })
                        {
                            correspondence = Some((k, swap, flip_u, flip_v));
                        }
                    }
                }
            }
        }
        let (k, swap, flip_u, flip_v) = correspondence.expect("source face correspondence");
        let native = surface(&expected["surfaces"][index]);
        let rust = actual.faces()[k].surface();
        for j in 0..=8 {
            for i in 0..=8 {
                let (u, v) = (i as Real / 8., j as Real / 8.);
                let (mut a, mut b) = (u, v);
                if swap {
                    std::mem::swap(&mut a, &mut b);
                }
                if flip_u {
                    a = 1. - a;
                }
                if flip_v {
                    b = 1. - b;
                }
                let native_point = p(&expected["samples"][index * 81 + j * 9 + i]);
                witness(rust, native_point, [a, b], epsilon, label);
                witness(&native, sample(rust, a, b), [u, v], epsilon, label);
            }
        }
    }
}
