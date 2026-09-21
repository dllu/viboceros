use super::*;

// Independent global Cox-de Boor basis, not the production local recurrence.
fn basis(knots: &[Real], degree: usize, t: Real, left: bool) -> Vec<Rational> {
    let n = knots.len() - degree - 1;
    let left = t == knots[n] || (left && t > knots[degree]);
    let mut values = knots
        .windows(2)
        .map(|k| {
            rational(
                if if left {
                    k[0] < t && t <= k[1]
                } else {
                    k[0] <= t && t < k[1]
                } {
                    1.
                } else {
                    0.
                },
            )
        })
        .collect::<Vec<_>>();
    let t = rational(t);
    for p in 1..=degree {
        values = (0..values.len() - 1)
            .map(|i| {
                let mut sum = rational(0.);
                if knots[i + p] != knots[i] {
                    sum += (&t - rational(knots[i])) * &values[i]
                        / (rational(knots[i + p]) - rational(knots[i]));
                }
                if knots[i + p + 1] != knots[i + 1] {
                    sum += (rational(knots[i + p + 1]) - &t) * &values[i + 1]
                        / (rational(knots[i + p + 1]) - rational(knots[i + 1]));
                }
                sum
            })
            .collect();
    }
    values
}

fn independent(surface: &NurbsSurface, uv: [Real; 2], sides: [bool; 2]) -> H {
    let u = basis(surface.knots_u(), surface.degree_u(), uv[0], sides[0]);
    let v = basis(surface.knots_v(), surface.degree_v(), uv[1], sides[1]);
    let mut sum: H = std::array::from_fn(|_| rational(0.));
    for (j, v) in v.iter().enumerate() {
        for (i, u) in u.iter().enumerate() {
            let p = surface.control_point(i, j).unwrap();
            let w = u * v * rational(p.weight());
            for (axis, sum) in sum.iter_mut().enumerate() {
                *sum += if axis == 3 {
                    w.clone()
                } else {
                    &w * rational(p.point().to_array()[axis])
                };
            }
        }
    }
    sum
}

fn surface(knots_u: Vec<Real>, knots_v: Vec<Real>, gauge: Real) -> NurbsSurface {
    let nu = knots_u.len() - 3;
    let nv = knots_v.len() - 3;
    NurbsSurface::try_new_rational(
        2,
        2,
        nu,
        nv,
        (0..nv)
            .flat_map(|j| {
                (0..nu).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            i as Real,
                            j as Real,
                            ((i * j + 3 * i + j) % 11) as Real / 4.,
                        )
                        .unwrap(),
                        ((i + 2 * j) % 5 + 1) as Real * gauge,
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots_u,
        knots_v,
    )
    .unwrap()
}

#[test]
fn tensor_isocurves_match_independent_basis_in_both_directions_and_sides() {
    let vectors = [
        vec![-2., -1., 0., 0.25, 0.5, 1., 2., 3.],
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    ];
    for gauge in [1., -2., Real::from_bits(1), 1e300] {
        for (ku, kv) in vectors.iter().zip(vectors.iter().cycle().skip(1)) {
            for origin in [0., 1e12] {
                let surface = surface(
                    ku.iter().map(|t| origin + t).collect(),
                    kv.iter().map(|t| origin + t).collect(),
                    gauge,
                );
                let before = surface.clone();
                for varying in 0..2 {
                    for fixed in [0., 0.125, 0.5, 0.875, 1.] {
                        for left in [false, true] {
                            let image = SurfaceCurve::new(
                                &surface,
                                varying,
                                origin + fixed,
                                left,
                                &mut |_| Ok(()),
                            )
                            .unwrap()
                            .unwrap();
                            for t in [0., 0.125, 0.5, 0.875, 1.] {
                                for end in [false, true] {
                                    // Endpoints approach t from the indicated side.
                                    let (interval, end) = if t == 0. {
                                        ([origin, origin + 1.], false)
                                    } else if t == 1. {
                                        ([origin, origin + 1.], true)
                                    } else if end {
                                        ([origin, origin + t], true)
                                    } else {
                                        ([origin + t, origin + 1.], false)
                                    };
                                    let h = image
                                        .endpoint(interval, end, &mut |_| Ok(()))
                                        .unwrap()
                                        .unwrap();
                                    let mut uv = [origin + fixed; 2];
                                    uv[varying] = origin + t;
                                    let mut sides = [left; 2];
                                    sides[varying] = end;
                                    let expected = independent(&surface, uv, sides);
                                    for i in 0..3 {
                                        assert_eq!(&h[i] / &h[3], &expected[i] / &expected[3]);
                                    }
                                }
                            }
                        }
                    }
                }
                assert_eq!(surface, before);
            }
        }
    }
}

#[test]
fn rounded_interior_isocurves_cannot_claim_zero_uncertainty() {
    for gauge in [1., -2., Real::from_bits(1), 1e300] {
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [(0., 0., 1.), (3., 0., 1.), (0., 1., 2.), (3., 1., 2.)]
                .map(|(x, y, w)| {
                    WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), w * gauge).unwrap()
                })
                .to_vec(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let image = SurfaceCurve::new(&surface, 0, 0.1, false, &mut |_| Ok(()))
            .unwrap()
            .unwrap();
        // The exact y is 2*t/(1+t), with binary64 t=0.1, not its rounded value.
        let y = crate::exact_scalar::scalar(
            &(rational(2.) * rational(0.1) / (rational(1.) + rational(0.1))),
        )
        .unwrap();
        let edge = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0., y, 0.).unwrap(),
                Point3::try_new(3., y, 0.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let a = extract::Spline::new(&edge, false, &mut |_| Ok(()))
            .unwrap()
            .unwrap();
        let b = image.spline([0., 1.], &mut |_| Ok(())).unwrap().unwrap();
        assert_eq!(
            splines_bound(&a, &b, 0., true, &mut |_| Ok(())).unwrap(),
            None
        );
        let bound = image
            .bound(&edge, [0., 1.], true, &mut |_| Ok(()))
            .unwrap()
            .unwrap();
        assert!(bound > 0. && bound < 1e-15);
        let exact = independent(&surface, [0., 0.1], [false; 2]);
        let d = &exact[1] / &exact[3] - rational(y);
        assert!(&d * &d <= rational(bound) * rational(bound));
        for end in [false, true] {
            let point = edge.control_points()[usize::from(end)].point();
            assert_eq!(
                image
                    .point_bound(point, [0., 1.], end, &mut |_| Ok(()))
                    .unwrap(),
                Some(bound)
            );
            assert_eq!(
                image
                    .edge_endpoint_bound(&edge, [0., 1.], [end, end], &mut |_| Ok(()))
                    .unwrap(),
                Some(bound)
            );
        }
    }
}

#[test]
fn exact_isocurve_restrictions_keep_orientation_and_error_budgets() {
    let s = surface(
        vec![-2., -1., 0., 1., 2., 3.],
        vec![0., 0., 0., 1., 1., 1.],
        1.,
    );
    for varying in 0..2 {
        let image = SurfaceCurve::new(&s, varying, 0.125, false, &mut |_| Ok(()))
            .unwrap()
            .unwrap();
        let curve = if varying == 0 {
            s.isocurve_u(0.125)
        } else {
            s.isocurve_v(0.125)
        }
        .unwrap();
        let part = curve.try_trimmed(0.125..=0.875).unwrap();
        for reverse in [false, true] {
            let (edge, interval) = if reverse {
                (part.reversed().unwrap(), [0.875, 0.125])
            } else {
                (part.clone(), [0.125, 0.875])
            };
            assert!(
                image
                    .bound(&edge, interval, true, &mut |_| Ok(()))
                    .unwrap()
                    .unwrap()
                    < 1e-12
            );
            assert!(
                image
                    .bound(&edge, interval, true, &mut |_| Err(invalid("test budget")))
                    .is_err()
            );
        }
        for interval in [[0.5, 0.5], [-0.1, 0.5], [0.5, Real::NAN]] {
            assert!(
                image
                    .bound(&part, interval, true, &mut |_| Ok(()))
                    .unwrap()
                    .is_none()
            );
        }
    }
    let before = s.clone();
    assert!(SurfaceCurve::new(&s, 0, 0.1, false, &mut |_| Err(invalid("test budget"))).is_err());
    assert!(
        SurfaceCurve::new(&s, 0, Real::NAN, false, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert_eq!(s, before);
}

#[test]
fn tensor_isocurves_reject_zero_and_mixed_denominators() {
    for weights in [[1., 1., -1., -1.], [1., -1., 1., -1.]] {
        let s = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [(0., 0.), (1., 0.), (0., 1.), (1., 1.)]
                .into_iter()
                .zip(weights)
                .map(|((x, y), w)| {
                    WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), w).unwrap()
                })
                .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        assert!(
            SurfaceCurve::new(&s, 0, 0.5, false, &mut |_| Ok(()))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn tensor_degree_limit_does_not_disable_exact_natural_rows() {
    let s = NurbsSurface::try_new_rational(
        17,
        1,
        18,
        2,
        (0..2)
            .flat_map(|v| {
                (0..18).map(move |u| {
                    WeightedPoint3::try_new(Point3::try_new(u as Real, v as Real, 0.).unwrap(), 1.)
                        .unwrap()
                })
            })
            .collect(),
        [vec![0.; 18], vec![1.; 18]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        SurfaceCurve::new(&s, 1, 0., false, &mut |_| Ok(())).unwrap(),
        Some(SurfaceCurve::Natural(_))
    ));
    assert!(
        SurfaceCurve::new(&s, 1, 0.5, false, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
    assert!(
        SurfaceCurve::new(&s, 0, 0.5, false, &mut |_| Ok(()))
            .unwrap()
            .is_none()
    );
}
