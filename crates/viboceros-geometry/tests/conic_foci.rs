use viboceros_geometry::{ConicFoci, NurbsCurve, Point3, Tolerance, WeightedPoint3};
fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn conic(points: [[f64; 3]; 3], weights: [f64; 3]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        points
            .into_iter()
            .zip(weights)
            .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn expected(curve: &NurbsCurve, parabola: bool, tilted: bool) {
    let map = |p: [f64; 3]| {
        if tilted {
            [
                3. + 0.6 * p[0] - 0.48 * p[1],
                -4. + 0.8 * p[0] + 0.36 * p[1],
                5. + 0.8 * p[1],
            ]
        } else {
            p
        }
    };
    let before = curve.clone();
    let f = curve
        .conic_foci(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap())
        .unwrap()
        .unwrap_or_else(|| panic!("unrecognized conic {curve:?}"));
    if parabola {
        let ConicFoci::Parabola { vertex, focus } = f else {
            panic!("{f:?}")
        };
        assert!(vertex.distance_to(point(map([0., 0., 0.]))).unwrap() < 1e-8);
        assert!(focus.distance_to(point(map([0., 1., 0.]))).unwrap() < 1e-8);
    } else {
        let ConicFoci::Hyperbola { foci } = f else {
            panic!("{f:?}")
        };
        for wanted in [[5., 0., 0.], [-5., 0., 0.]] {
            assert!(
                foci.iter()
                    .any(|p| p.distance_to(point(map(wanted))).unwrap() < 1e-8),
                "{f:?}"
            );
        }
    }
    assert_eq!(curve, &before);
}
#[test]
fn open_conics_preserve_foci_under_elevation_refinement_weights_domains_and_rigid_placement() {
    for parabola in [false, true] {
        for tilted in [false, true] {
            let mut points = if parabola {
                [[-2., 1., 0.], [0., -1., 0.], [2., 1., 0.]]
            } else {
                [[3.75, -3., 0.], [2.4, 0., 0.], [3.75, 3., 0.]]
            };
            if tilted {
                for p in &mut points {
                    *p = [
                        3. + 0.6 * p[0] - 0.48 * p[1],
                        -4. + 0.8 * p[0] + 0.36 * p[1],
                        5. + 0.8 * p[1],
                    ];
                }
            }
            let base = conic(points, if parabola { [1.; 3] } else { [1., 1.25, 1.] });
            for degree in [2, 5, 12] {
                let elevated = base.try_change_degree(degree, false).unwrap();
                for refined in [false, true] {
                    let source = if refined {
                        elevated.try_insert_knot(0.3, 1).unwrap()
                    } else {
                        elevated.clone()
                    };
                    for gauge in [1., -8., 1e-200, 1e200] {
                        let scaled = NurbsCurve::try_new_rational(
                            degree,
                            source
                                .control_points()
                                .iter()
                                .map(|p| {
                                    WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap()
                                })
                                .collect(),
                            source.knots().to_vec(),
                        )
                        .unwrap();
                        for domain in [0. ..=1., -3. ..=17., 0. ..=1e-170, 0. ..=1e170] {
                            let curve = scaled.try_reparameterized(domain).unwrap();
                            expected(&curve, parabola, tilted);
                            expected(&curve.reversed().unwrap(), parabola, tilted);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn nonconics_and_degenerate_planar_loci_do_not_produce_markers() {
    let line = conic([[0., 0., 0.], [1., 0., 0.], [3., 0., 0.]], [1.; 3]);
    assert!(line.conic_foci(Tolerance::DEFAULT).unwrap().is_none());
    let cubic = NurbsCurve::try_new_rational(
        3,
        [[0., 0., 0.], [1., 3., 0.], [3., -1., 0.], [5., 0., 0.]]
            .into_iter()
            .map(|p| WeightedPoint3::try_new(point(p), 1.).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(cubic.conic_foci(Tolerance::DEFAULT).unwrap().is_none());
    let nonplanar = NurbsCurve::try_new_rational(
        3,
        [[0., 0., 0.], [1., 3., 0.], [3., -1., 1.], [5., 0., 0.]]
            .into_iter()
            .map(|p| WeightedPoint3::try_new(point(p), 1.).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(nonplanar.conic_foci(Tolerance::DEFAULT).unwrap().is_none());
}
