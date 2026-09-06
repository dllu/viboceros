use super::*;
use crate::{NurbsCurve, NurbsSurface};

fn point(i: usize, j: usize) -> Point3 {
    Point3::try_new(
        (i as f64 * 1.7).sin() * 4.,
        (j as f64 * 2.3).cos() * 3.,
        ((i + 3 * j) as f64).sin() * 7.,
    )
    .unwrap()
}
fn at(net: &Net, u: f64, v: f64) -> Point3 {
    let eval = |values: &[H], t| {
        let mut work = values.to_vec();
        for level in 1..work.len() {
            for i in 0..work.len() - level {
                work[i] = blend(work[i], work[i + 1], t);
            }
        }
        work[0]
    };
    let rows = net
        .controls
        .chunks_exact(net.degrees[0] + 1)
        .map(|row| eval(row, u))
        .collect::<Vec<_>>();
    net.project(eval(&rows, v)).unwrap()
}

#[test]
fn local_blossoming_matches_native_evaluation_for_every_unclamped_curve_span() {
    for degree in 1..=9 {
        let count = degree + 4;
        let controls = (0..count)
            .map(|i| {
                WeightedPoint3::try_new(
                    point(i, 0),
                    if i % 4 == 1 {
                        -0.05
                    } else {
                        1. + i as f64 / 5.
                    },
                )
                .unwrap()
            })
            .collect();
        let knots = (0..count + degree + 1).map(|i| i as f64 * 0.37).collect();
        let curve = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
        let mut budget = Budget::default();
        for span in degree..count {
            let mut net =
                Net::new([degree, 0], &curve.control_points()[span - degree..=span]).unwrap();
            net.extract_axis(0, curve.knots(), span, &mut budget)
                .unwrap();
            for t in [0., 0.07, 0.25, 0.51, 0.89, 1.] {
                let parameter = (1. - t) * curve.knots()[span] + t * curve.knots()[span + 1];
                let expected = curve.evaluate(parameter).unwrap();
                assert!(
                    at(&net, t, 0.).distance_to(expected).unwrap() < 2e-12,
                    "degree {degree}, span {span}, t {t}"
                );
            }
        }
    }
}

#[test]
fn tensor_blossoming_matches_native_rational_evaluation_without_global_knot_insertion() {
    for p in 1..=5 {
        for q in 1..=4 {
            let (nu, nv) = (p + 3, q + 2);
            let controls = (0..nv)
                .flat_map(|j| {
                    (0..nu).map(move |i| {
                        WeightedPoint3::try_new(point(i, j), 1. + (i + j) as f64 / 5.).unwrap()
                    })
                })
                .collect();
            let s = NurbsSurface::try_new_rational(
                p,
                q,
                nu,
                nv,
                controls,
                (0..nu + p + 1).map(|i| i as f64 * 0.31).collect(),
                (0..nv + q + 1).map(|i| i as f64 * 0.57).collect(),
            )
            .unwrap();
            let mut budget = Budget::default();
            for v in q..nv {
                for u in p..nu {
                    let controls = (v - q..=v)
                        .flat_map(|j| (u - p..=u).map(move |i| (i, j)))
                        .map(|(i, j)| s.control_points()[j * nu + i])
                        .collect::<Vec<_>>();
                    let mut net = Net::new([p, q], &controls).unwrap();
                    net.extract_axis(0, s.knots_u(), u, &mut budget).unwrap();
                    net.extract_axis(1, s.knots_v(), v, &mut budget).unwrap();
                    for a in [0., 0.13, 0.68, 1.] {
                        for b in [0., 0.27, 0.91, 1.] {
                            let expected = s
                                .evaluate(
                                    (1. - a) * s.knots_u()[u] + a * s.knots_u()[u + 1],
                                    (1. - b) * s.knots_v()[v] + b * s.knots_v()[v + 1],
                                )
                                .unwrap();
                            assert!(
                                at(&net, a, b).distance_to(expected).unwrap() < 3e-12,
                                "degree {p},{q}; span {u},{v}; parameter {a},{b}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn tensor_subdivision_retains_exact_geometry_in_both_child_coordinate_maps() {
    let controls = (0..4)
        .flat_map(|v| {
            (0..5).map(move |u| {
                WeightedPoint3::try_new(point(u, v), if (u + v) % 3 == 0 { -0.01 } else { 1. })
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    for axis in 0..2 {
        let source = Net::new([4, 3], &controls).unwrap();
        let expected = (0..=8)
            .flat_map(|v| (0..=8).map(move |u| (u as f64 / 8., v as f64 / 8.)))
            .map(|(u, v)| ((u, v), at(&source, u, v)))
            .collect::<Vec<_>>();
        let (left, right) = source.split(axis);
        for ((u, v), expected) in expected {
            let mut parameter = [u, v];
            let net = if parameter[axis] <= 0.5 {
                parameter[axis] *= 2.;
                &left
            } else {
                parameter[axis] = 2. * parameter[axis] - 1.;
                &right
            };
            assert!(
                at(net, parameter[0], parameter[1])
                    .distance_to(expected)
                    .unwrap()
                    < 5e-12
            );
        }
    }
}

#[test]
fn resource_budgets_fail_before_unbounded_control_allocation_or_subdivision_work() {
    assert_eq!(
        Budget::default().initial(MAX_INITIAL_CONTROLS + 1),
        Err(GeometryError::BoundingBoxDidNotConverge)
    );
    assert_eq!(
        Budget::default().charge(MAX_WORK + 1),
        Err(GeometryError::BoundingBoxDidNotConverge)
    );
}
