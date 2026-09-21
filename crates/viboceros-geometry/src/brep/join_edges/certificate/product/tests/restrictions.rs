use super::*;

fn restricted(a: &NurbsCurve, b: &NurbsCurve, interval: [Real; 2], limit: Real) -> Option<Real> {
    let mut budget = Budget(MAX_WORK);
    restricted_curve_bound(a, b, interval, limit, true, |n| budget.charge(n)).unwrap()
}

#[test]
fn paired_endpoint_bounds_evaluate_unclamped_edges_exactly() {
    let a = curve(
        &[[0.; 3], [2., 4., 0.], [4., 0., 0.]],
        &[1.; 3],
        2,
        &[-2., -1., 0., 1., 2., 3.],
    );
    let b = curve(
        &[[1., 2., 0.], [3., 2., 0.]],
        &[1.; 2],
        1,
        &[0., 0., 1., 1.],
    );
    for end in [false, true] {
        assert_eq!(
            curve_endpoint_bound(&a, &b, [0., 1.], [end, end], 0., |_| Ok(())).unwrap(),
            Some(0.)
        );
        assert_eq!(
            curve_endpoint_bound(&a, &b, [0., 1.], [end, !end], 2., |_| Ok(())).unwrap(),
            Some(2.)
        );
        assert_eq!(
            curve_endpoint_bound(&a, &b, [0., 1.], [end, !end], 2_f64.next_down(), |_| Ok(()))
                .unwrap(),
            None
        );
    }
    assert!(
        curve_endpoint_bound(&a, &b, [0., 1.], [false, false], 0., |_| Err(invalid(
            "test budget"
        )))
        .is_err()
    );
}

#[test]
fn rounded_trim_controls_are_not_an_exact_boundary_certificate() {
    for gauge in [1., -2., Real::from_bits(1), 1e300] {
        let b = curve(&[[0.; 3], [3., 0., 0.]], &[gauge; 2], 1, &[0., 0., 1., 1.]);
        let a = b.try_trimmed(0.1..=0.2).unwrap();
        assert_eq!(restricted(&a, &b, [0.1, 0.2], 0.), None);
        let bound = restricted(&a, &b, [0.1, 0.2], 1e-15).unwrap();
        assert!(bound > 0. && bound < 1e-15);
        for i in 0..=32 {
            let t = rational(0.1) + rational(i as Real / 32.) * (rational(0.2) - rational(0.1));
            let (a, b) = (exact_value(&a, &t), exact_value(&b, &t));
            let squared: Rational = a
                .iter()
                .zip(b)
                .map(|(a, b)| {
                    let d = a - b;
                    &d * &d
                })
                .sum();
            assert!(squared <= rational(bound) * rational(bound));
        }
        for end in [false, true] {
            let p = a.control_points()[usize::from(end)].point();
            let b = restricted_endpoint_bound(p, &b, [0.1, 0.2], end, 1e-15, |_| Ok(()))
                .unwrap()
                .unwrap();
            assert!(b > 0. && b <= bound);
        }
    }
}

#[test]
fn restricted_projective_maps_clip_outer_knots_before_inverse_mapping() {
    let source = quadratic();
    let part = source.try_trimmed(0.25..=0.75).unwrap();
    for factor in [0.25_f64, 4.] {
        let a = curve(
            &part
                .control_points()
                .iter()
                .map(|c| c.point().to_array())
                .collect::<Vec<_>>(),
            &[1., factor, factor * factor],
            2,
            part.knots(),
        );
        for offset in [0., 1e12] {
            let b = source.try_reparameterized(offset..=offset + 1.).unwrap();
            let interval = [offset + 0.25, offset + 0.75];
            assert_eq!(restricted(&a, &b, interval, 0.), Some(0.));
            assert_eq!(
                restricted(&a.reversed().unwrap(), &b, [interval[1], interval[0]], 0.),
                Some(0.)
            );
            let b = b
                .try_insert_knot(offset + 0.125, 1)
                .unwrap()
                .try_insert_knot(offset + 0.625, 1)
                .unwrap();
            assert!(restricted(&a, &b, interval, 1e-12).unwrap() <= 1e-12);
        }
    }
}

#[test]
fn restricted_endpoints_use_the_interior_side_of_full_order_knots() {
    let b = curve(
        &[[0.; 3], [1., 0., 0.], [10., 0., 0.], [11., 0., 0.]],
        &[1.; 4],
        1,
        &[0., 0., 0.5, 0.5, 1., 1.],
    );
    for (interval, ends) in [([0.25, 0.5], [0.5, 1.]), ([0.5, 0.75], [10., 10.5])] {
        let a = curve(&ends.map(|x| [x, 0., 0.]), &[1.; 2], 1, &[0., 0., 1., 1.]);
        for reverse in [false, true] {
            let mut interval = interval;
            let mut ends = ends;
            let a = if reverse {
                interval.reverse();
                ends.reverse();
                a.reversed().unwrap()
            } else {
                a.clone()
            };
            assert_eq!(restricted(&a, &b, interval, 0.), Some(0.));
            for end in [false, true] {
                let p = Point3::try_new(ends[usize::from(end)], 0., 0.).unwrap();
                assert_eq!(
                    restricted_endpoint_bound(p, &b, interval, end, 0., |_| Ok(())).unwrap(),
                    Some(0.)
                );
            }
        }
    }
    let a = curve(
        &[[0.5, 0., 0.], [10.5, 0., 0.]],
        &[1.; 2],
        1,
        &[0., 0., 1., 1.],
    );
    assert_eq!(restricted(&a, &b, [0.25, 0.75], 0.001), None);
}

#[test]
fn restricted_spans_match_independent_exact_basis_with_unclamped_and_repeated_knots() {
    for knots in [
        vec![-2., -1., 0., 0.25, 0.5, 1., 2., 3.],
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    ] {
        let count = knots.len() - 3;
        let b = curve(
            &(0..count)
                .map(|i| [i as Real, (i * i % 7) as Real, (i % 3) as Real])
                .collect::<Vec<_>>(),
            &(0..count).map(|i| (i + 1) as Real / 4.).collect::<Vec<_>>(),
            2,
            &knots,
        );
        for interval in [[0.1, 0.8], [0.8, 0.1]] {
            let spline = extract::Spline::restricted(&b, interval, &mut |_| Ok(()))
                .unwrap()
                .unwrap();
            for span in 2..count {
                let left = std::cmp::max(spline.knots[span].clone(), rational(0.));
                let right = std::cmp::min(spline.knots[span + 1].clone(), rational(1.));
                if left >= right {
                    continue;
                }
                let net = spline
                    .extract(span, &left, &right, &mut |_| Ok(()))
                    .unwrap();
                for i in 0..=8 {
                    let s = rational(i as Real / 8.);
                    let u = rational(1.) - &s;
                    let t = &left + &s * (&right - &left);
                    let original =
                        rational(interval[0]) + t * (rational(interval[1]) - rational(interval[0]));
                    let expected = exact_value(&b, &original);
                    let basis = [&u * &u, rational(2.) * &u * &s, &s * &s];
                    let h: H = std::array::from_fn(|axis| {
                        net.iter()
                            .zip(&basis)
                            .map(|(h, basis)| &h[axis] * basis)
                            .sum()
                    });
                    for axis in 0..3 {
                        assert_eq!(&h[axis] / &h[3], expected[axis]);
                    }
                }
            }
            for end in [false, true] {
                let (net, _) = spline.end_span(end, &mut |_| Ok(())).unwrap();
                let h = &net[if end { net.len() - 1 } else { 0 }];
                let expected = exact_value(&b, &rational(interval[usize::from(end)]));
                for axis in 0..3 {
                    assert_eq!(&h[axis] / &h[3], expected[axis]);
                }
            }
        }
    }
}

#[test]
fn restrictions_reject_invalid_intervals_and_charge_without_mutation() {
    let b = quadratic();
    let a = b.try_trimmed(0.25..=0.75).unwrap();
    let before = [a.clone(), b.clone()];
    for interval in [
        [-0.1, 0.5],
        [0.5, 1.1],
        [0.5, 0.5],
        [Real::NAN, 0.5],
        [0., Real::INFINITY],
    ] {
        assert_eq!(restricted(&a, &b, interval, 1.), None);
        assert_eq!(
            restricted_endpoint_bound(
                Point3::try_new(0., 0., 0.).unwrap(),
                &b,
                interval,
                false,
                1.,
                |_| Ok(())
            )
            .unwrap(),
            None
        );
    }
    assert!(
        restricted_curve_bound(&a, &b, [0.25, 0.75], 1., true, |_| Err(invalid(
            "test budget"
        )))
        .is_err()
    );
    assert!(
        restricted_endpoint_bound(
            a.control_points()[0].point(),
            &b,
            [0.25, 0.75],
            false,
            1.,
            |_| Err(invalid("test budget"))
        )
        .is_err()
    );
    assert_eq!([a, b], before);
}
