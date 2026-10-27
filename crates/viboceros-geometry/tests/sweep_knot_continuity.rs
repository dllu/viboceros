use viboceros_geometry::{
    CurveRef, GeometryError, LineSegment, NurbsCurve, Point3, Sweep1, SweepBlend, SweepFrameStyle,
    SweepSection, Tolerance, WeightedPoint3,
};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}

fn section(rail: &NurbsCurve, parameter: f64, axis: usize) -> SweepSection {
    let start = rail.evaluate(parameter).unwrap();
    let mut end = start.to_array();
    end[axis] += 0.5;
    SweepSection {
        parameter,
        curve: LineSegment::try_new(start, point(end), Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap(),
    }
}

fn model(rail: &NurbsCurve, sections: &[SweepSection]) -> Result<Sweep1, GeometryError> {
    Sweep1::try_new(
        CurveRef::NurbsCurve(rail),
        sections,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap(),
    )
}

#[test]
fn smooth_polynomial_join_remains_sweepable_with_a_few_ulps_left_before_it() {
    // Two exact quadratic spans of (7+t, -3+t², 11), with a C1 join at t=1.
    // Trimming just before that join can round the tiny span's controls equal.
    let source = NurbsCurve::try_new(
        2,
        [
            [7., -3., 11.],
            [7.5, -3., 11.],
            [8., -2., 11.],
            [8.5, -1., 11.],
            [9., 1., 11.],
        ]
        .map(point)
        .to_vec(),
        vec![0., 0., 0., 1., 1., 2., 2., 2.],
    )
    .unwrap();
    for reversed in [false, true] {
        let rail = if reversed {
            source.reversed().unwrap()
        } else {
            source.clone()
        };
        let knot: f64 = if reversed { -1. } else { 1. };
        for ulps in [1, 2, 4, 8] {
            let start = (0..ulps).fold(knot, |t, _| t.next_down());
            let profile = section(&rail, start, 2);
            let sweep = model(&rail, std::slice::from_ref(&profile)).unwrap();
            assert_eq!(sweep.domain(), start..=*rail.domain().end());
            let repeated = sweep
                .sections_at(&[start, knot, knot, *rail.domain().end()])
                .unwrap();
            assert_eq!(repeated[1], repeated[2]);
            assert!(sweep.sections_at(&[knot, start]).is_err());
            let stations = [start, knot, *rail.domain().end()];
            for (t, curve) in stations
                .into_iter()
                .zip(sweep.sections_at(&stations).unwrap())
            {
                let origin = rail.evaluate(t).unwrap();
                for (v, height) in [(0., 0.), (1., 0.5)] {
                    let p = curve.evaluate(curve.parameter_at(v).unwrap()).unwrap();
                    assert!(
                        p.distance_to(point([origin.x(), origin.y(), 11. + height]))
                            .unwrap()
                            < 1e-10
                    );
                }
            }
            // A normal profile is a translation of this planar rail. Verify
            // the fitted surface against its independent analytic equation.
            let surface = sweep.to_surface().unwrap();
            for i in 0..=16 {
                for v in [0., 0.5, 1.] {
                    let p = surface
                        .evaluate(
                            surface.parameter_at_u(i as f64 / 16.).unwrap(),
                            surface.parameter_at_v(v).unwrap(),
                        )
                        .unwrap();
                    assert!((p.y() + 3. - (p.x() - 7.).powi(2)).abs() < 1e-6);
                    assert!((p.z() - 11. - 0.5 * v).abs() < 1e-10);
                }
            }
            assert_eq!(profile.curve, section(&rail, start, 2).curve);
        }
    }
}

#[test]
fn translated_rational_arc_keeps_its_source_continuity_next_to_the_join() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/sweep1_refit_intervals.json"
    ))
    .unwrap();
    let op = request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["id"] == "greville-nurbs-0.5-refit")
        .unwrap();
    let r = &op["rail"];
    let source = NurbsCurve::try_new_rational(
        2,
        r["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    point(serde_json::from_value(c["point"].clone()).unwrap()),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(r["knots"].clone()).unwrap(),
    )
    .unwrap();
    for reversed in [false, true] {
        let rail = if reversed {
            source.reversed().unwrap()
        } else {
            source.clone()
        };
        let knot = rail.knots()[3];
        for start in [knot.next_down(), knot] {
            let profile = section(&rail, start, 0);
            let sweep = model(&rail, &[profile]).unwrap();
            let surface = sweep.to_surface().unwrap();
            for i in 0..=16 {
                for v in [0., 0.5, 1.] {
                    let p = surface
                        .evaluate(
                            surface.parameter_at_u(i as f64 / 16.).unwrap(),
                            surface.parameter_at_v(v).unwrap(),
                        )
                        .unwrap();
                    assert!((p.x() - 7. - 0.5 * v).abs() < 1e-10);
                    assert!(((p.y() + 3.).hypot(p.z() - 11.) - 0.75).abs() < 1e-7);
                }
            }
        }
    }
}

#[test]
fn a_real_corner_is_rejected_even_when_only_one_ulp_of_its_incoming_span_remains() {
    let source = NurbsCurve::try_new(
        1,
        [[7., -3., 11.], [8., -2., 11.], [9., -2., 11.]]
            .map(point)
            .to_vec(),
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    for reversed in [false, true] {
        let rail = if reversed {
            source.reversed().unwrap()
        } else {
            source.clone()
        };
        let knot: f64 = if reversed { -1. } else { 1. };
        assert!(matches!(
            model(&rail, &[section(&rail, knot.next_down(), 2)]),
            Err(GeometryError::InvalidSweep {
                context: "rail corners require sweep miter construction"
            })
        ));
        for start in [knot, knot.next_up()] {
            let sweep = model(&rail, &[section(&rail, start, 2)]).unwrap();
            assert_eq!(sweep.domain(), start..=*rail.domain().end());
            assert!(sweep.try_on_full_rail().is_err());
        }
        // A corner exactly at the far interval boundary also lies outside
        // the open interval whose tangent continuity is required.
        model(
            &rail,
            &[
                section(&rail, *rail.domain().start(), 2),
                section(&rail, knot, 2),
            ],
        )
        .unwrap();
    }
}
