#![cfg(feature = "native")]
use viboceros_geometry::{AffineTransform3, NurbsCurve, Point3, Tolerance, Vector3};
use viboceros_smlib::gordon_network_surface;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn value(u: f64, v: f64) -> Point3 {
    p(
        4. * u,
        3. * v,
        2. * u * v + 4. * u * (1. - u) + 2. * v * (1. - v) + 8. * u * (1. - u) * v * (1. - v),
    )
}
fn curve(along_u: bool, t: f64) -> NurbsCurve {
    let f = |s| if along_u { value(s, t) } else { value(t, s) };
    let a = f(0.).to_array();
    let b = f(0.5).to_array();
    let c = f(1.).to_array();
    let mid = std::array::from_fn(|i| 2. * b[i] - (a[i] + c[i]) / 2.);
    NurbsCurve::try_new(
        2,
        vec![f(0.), Point3::try_from(mid).unwrap(), f(1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn network() -> Vec<NurbsCurve> {
    [true, false]
        .into_iter()
        .flat_map(|along| [0., 0.5, 1.].map(|t| curve(along, t)))
        .collect()
}
#[test]
fn native_network_surface_interpolates_complete_curves_and_the_biquadratic_interior() {
    let curves = network();
    let before = format!("{curves:?}");
    let s = gordon_network_surface(&curves, Tolerance::DEFAULT, Tolerance::DEFAULT).unwrap();
    for i in 0..=6 {
        for j in 0..=6 {
            let q = value(i as f64 / 6., j as f64 / 6.);
            let (u, v) = s.closest_parameters(q, Tolerance::DEFAULT).unwrap();
            assert!(
                s.evaluate(u, v).unwrap().distance_to(q).unwrap() < 1e-6,
                "{i} {j}"
            );
        }
    }
    assert_eq!(format!("{curves:?}"), before);
}
#[test]
fn native_network_surface_sorts_reversed_shuffled_independent_charts_at_a_remote_origin() {
    let transform = AffineTransform3::try_new(
        [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        Vector3::try_new(1e8, -2e8, 3e8).unwrap(),
    )
    .unwrap();
    let original = network();
    let curves = [4, 2, 3, 0, 5, 1]
        .into_iter()
        .enumerate()
        .map(|(i, j)| {
            let c = original[j]
                .transformed(transform)
                .unwrap()
                .try_reparameterized((-7. - i as f64)..=(13. + i as f64))
                .unwrap();
            if i % 2 == 0 { c.reversed().unwrap() } else { c }
        })
        .collect::<Vec<_>>();
    let s = gordon_network_surface(&curves, Tolerance::DEFAULT, Tolerance::DEFAULT).unwrap();
    let q = transform.transform_point(value(0.37, 0.62)).unwrap();
    let (u, v) = s.closest_parameters(q, Tolerance::DEFAULT).unwrap();
    assert!(s.evaluate(u, v).unwrap().distance_to(q).unwrap() < 1e-6);
}
#[test]
fn native_network_surface_rejects_non_network_duplicate_and_incomplete_constraints_without_mutation()
 {
    let curves = network();
    let original = format!("{curves:?}");
    for sources in [
        curves[..3].to_vec(),
        vec![curves[0].clone(); 4],
        curves[..5].to_vec(),
    ] {
        assert!(gordon_network_surface(&sources, Tolerance::DEFAULT, Tolerance::DEFAULT).is_err());
    }
    assert_eq!(format!("{curves:?}"), original);
}

#[test]
fn native_network_surface_preserves_rational_cylinder_inputs_with_complete_image_certificates() {
    use viboceros_geometry::WeightedPoint3;
    let mut curves = vec![];
    for z in [0., 1.5, 3.] {
        curves.push(
            NurbsCurve::try_new_rational(
                2,
                [
                    (p(2., 0., z), 1.),
                    (p(2., 2., z), 0.5_f64.sqrt()),
                    (p(0., 2., z), 1.),
                ]
                .into_iter()
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        );
    }
    for angle in [0., std::f64::consts::FRAC_PI_4, std::f64::consts::FRAC_PI_2] {
        curves.push(
            NurbsCurve::try_new(
                1,
                vec![
                    p(2. * angle.cos(), 2. * angle.sin(), 0.),
                    p(2. * angle.cos(), 2. * angle.sin(), 3.),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        );
    }
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let surface = gordon_network_surface(&curves, tolerance, tolerance).unwrap();
    assert!((surface.area(tolerance).unwrap() - 3. * std::f64::consts::PI).abs() < 1e-5);
}

#[test]
fn native_gordon_network_retains_kinks_for_a_separate_network_fitting_stage() {
    let mut curves = vec![];
    for y in [0., 1.5, 3.] {
        curves.push(
            NurbsCurve::try_new(
                1,
                vec![p(0., y, 0.), p(2., y, 1.), p(4., y, 0.)],
                vec![0., 0., 0.5, 1., 1.],
            )
            .unwrap(),
        );
    }
    for (x, z) in [(0., 0.), (2., 1.), (4., 0.)] {
        curves.push(
            NurbsCurve::try_new(1, vec![p(x, 0., z), p(x, 3., z)], vec![0., 0., 1., 1.]).unwrap(),
        );
    }
    let s = gordon_network_surface(&curves, Tolerance::DEFAULT, Tolerance::DEFAULT).unwrap();
    assert!(
        s.sampled_kink_parameters(Tolerance::DEFAULT.angular())
            .unwrap()
            .iter()
            .any(|p| !p.is_empty())
    );
    assert!((s.area(Tolerance::DEFAULT).unwrap() - 6. * 5_f64.sqrt()).abs() < 1e-7);
}

#[test]
fn constrained_arc_length_refits_keep_network_crossings_detectable() {
    use viboceros_geometry::{CurveCurveIntersectionEvent, CurveFitConstraint, CurveRef};
    let curves = network();
    let before = curves.clone();
    let fitted = curves
        .iter()
        .map(|source| {
            let pins = [0., 0.5, 1.].map(|t| CurveFitConstraint {
                source_parameter: t,
                point: source.evaluate(t).unwrap(),
            });
            viboceros_smlib::fit_cubic_curve_with_constraints(
                CurveRef::NurbsCurve(source),
                &pins,
                1e-7,
                0.,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .curve
        })
        .collect::<Vec<_>>();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for a in &fitted[..3] {
        for b in &fitted[3..] {
            let contacts = viboceros_smlib::curve_intersection_events(a, b, tolerance).unwrap();
            assert!(
                matches!(contacts.as_slice(), [CurveCurveIntersectionEvent::Point(_)]),
                "{contacts:?}"
            );
        }
    }
    let surface = gordon_network_surface(&fitted, tolerance, tolerance).unwrap();
    for u in [0., 0.5, 1.] {
        for v in [0., 0.5, 1.] {
            let q = value(u, v);
            let (s, t) = surface.closest_parameters(q, tolerance).unwrap();
            assert!(surface.evaluate(s, t).unwrap().distance_to(q).unwrap() < 1e-6);
        }
    }
    assert_eq!(curves, before);
}

#[test]
fn native_network_rejects_oversized_tensor_compatibility_before_construction() {
    // Straight curves with distinct, redundant interior knots keep the input
    // below 2,048 controls but force large common bases in both directions.
    let mut curves = vec![];
    for along_u in [true, false] {
        for i in 0..8 {
            let mut parameters = vec![0.];
            parameters.extend((1..99).map(|j| (j as f64 + 0.07 * i as f64) / 100.));
            parameters.push(1.);
            let points = parameters
                .iter()
                .map(|&t| {
                    let s = i as f64 / 7.;
                    if along_u {
                        p(4. * t, 3. * s, 0.)
                    } else {
                        p(4. * s, 3. * t, 0.)
                    }
                })
                .collect();
            let mut knots = vec![0.];
            knots.extend(parameters);
            knots.push(1.);
            curves.push(NurbsCurve::try_new(1, points, knots).unwrap());
        }
    }
    let before = curves.clone();
    let error =
        gordon_network_surface(&curves, Tolerance::DEFAULT, Tolerance::DEFAULT).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Network compatibility control-net resource limit"),
        "{error}"
    );
    assert_eq!(curves, before);
}

#[test]
fn native_network_aligns_asymmetric_families_without_rounding_away_the_endpoint() {
    let point = |u, v| p(4. * u, 3. * v, 0.25 * u * v);
    let line = |a, b| NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap();
    let curves = [0., 0.5, 1.]
        .map(|v| line(point(0., v), point(1., v)))
        .into_iter()
        .chain([0., 0.1, 0.2, 0.4, 0.7, 0.9, 1.].map(|u| line(point(u, 0.), point(u, 1.))))
        .collect::<Vec<_>>();
    let surface = gordon_network_surface(&curves, Tolerance::DEFAULT, Tolerance::DEFAULT).unwrap();
    for (u, v) in [(0., 0.), (1., 1.), (0.37, 0.62)] {
        assert!(
            surface
                .evaluate(u, v)
                .unwrap()
                .distance_to(point(u, v))
                .unwrap()
                < 1e-9
        );
    }
}

#[test]
fn common_station_curve_fits_preserve_network_contacts_and_complete_original_images() {
    use viboceros_geometry::{CurveFitConstraint, CurveFitStation, CurveRef};
    let original = network();
    let before = original.clone();
    let pins = |source: &NurbsCurve| {
        [0., 0.5, 1.].map(|t| CurveFitConstraint {
            source_parameter: t,
            point: source.evaluate(t).unwrap(),
        })
    };
    let arc = original
        .iter()
        .map(|source| {
            viboceros_smlib::fit_cubic_curve_with_constraints(
                CurveRef::NurbsCurve(source),
                &pins(source),
                1e-7,
                0.,
                Tolerance::DEFAULT,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mid = [&arc[..3], &arc[3..]].map(|family| {
        family
            .iter()
            .map(|fit| fit.constraint_parameters[1] / fit.curve.domain().end())
            .sum::<f64>()
            / family.len() as f64
    });
    let fitted = original
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let stations = pins(source)
                .into_iter()
                .zip([0., mid[i / 3], 1.])
                .map(|(pin, fit_parameter)| CurveFitStation {
                    source_parameter: pin.source_parameter,
                    fit_parameter,
                    point: pin.point,
                })
                .collect::<Vec<_>>();
            let fit = viboceros_smlib::fit_cubic_curve_at_stations(
                CurveRef::NurbsCurve(source),
                &stations,
                1e-7,
                0.,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert!(
                fit.curve
                    .knots()
                    .chunk_by(|a, b| a == b)
                    .filter(|g| g[0] > 0. && g[0] < 1.)
                    .all(|g| g.len() == 1)
            );
            for station in stations {
                assert!(
                    fit.curve
                        .evaluate(station.fit_parameter)
                        .unwrap()
                        .distance_to(station.point)
                        .unwrap()
                        < 1e-11
                );
            }
            fit.curve
        })
        .collect::<Vec<_>>();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let surface = gordon_network_surface(&fitted, tolerance, tolerance).unwrap();
    for (index, source) in original.iter().enumerate() {
        let fixed = if index < 3 {
            [0., mid[1], 1.][index]
        } else {
            [0., mid[0], 1.][index - 3]
        };
        let endpoints = if index < 3 {
            [[0., fixed], [1., fixed]]
        } else {
            [[fixed, 0.], [fixed, 1.]]
        };
        let endpoints = endpoints.map(|p| viboceros_geometry::Point2::try_new(p[0], p[1]).unwrap());
        let result =
            surface.try_pullback_curve_certified_with_bound(source, Some(endpoints), tolerance);
        let (uv, bound) = result.unwrap_or_else(|e| panic!("original curve {index}: {e}"));
        assert_eq!(uv.domain(), source.domain());
        assert!(bound <= tolerance.absolute());
    }
    for (i, u) in [0., mid[0], 1.].into_iter().enumerate() {
        for (j, v) in [0., mid[1], 1.].into_iter().enumerate() {
            assert!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(value(i as f64 / 2., j as f64 / 2.))
                    .unwrap()
                    < 1e-6
            );
        }
    }
    assert_eq!(original, before);
}
