use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn line(a: Point3, b: Point3) -> crate::LineSegment {
    crate::LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap()
}

#[test]
fn multi_section_rail_refit_interpolates_local_blends_in_its_cubic_basis() {
    let rail = line(p(0., 0., 0.), p(0., 0., 5.));
    let sections = [(0., 2.), (2., 1.), (5., 3.)].map(|(t, width)| SweepSection {
        parameter: t,
        curve: line(p(0., 0., t), p(width, 0., t)).to_nurbs().unwrap(),
    });
    let sweep = Sweep1::try_new(
        CurveRef::Line(&rail),
        &sections,
        Default::default(),
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let refitted = sweep.to_surface().unwrap();
    let implicit = sweep.to_rail_basis_surface().unwrap();
    assert_eq!(refitted, implicit);
    assert_eq!(
        refitted.knots_u(),
        &[0., 0., 0., 0., 1., 2., 3., 5., 5., 5., 5.]
    );
    for t in [0., 1. / 3., 1., 2., 10. / 3., 13. / 3., 5.] {
        let (f, a, b) = if t <= 2. {
            (t / 2., 2., 1.)
        } else {
            ((t - 2.) / 3., 1., 3.)
        };
        let width = a + (b - a) * f * f * (3. - 2. * f);
        for v in [0., 0.17, 0.51, 1.] {
            assert!(
                refitted
                    .evaluate(t, v)
                    .unwrap()
                    .distance_to(p(width * v, 0., t))
                    .unwrap()
                    < 1e-12
            );
        }
    }
    // The fixed basis interpolant is not the continuous piecewise smoothstep.
    let model = sweep.sections_at(&[2.5]).unwrap();
    assert!(
        refitted
            .evaluate(2.5, 1.)
            .unwrap()
            .distance_to(model[0].evaluate(1.).unwrap())
            .unwrap()
            > 1e-4
    );
}

#[test]
fn neighboring_profile_stations_keep_distinct_symmetric_knot_neighborhoods() {
    let rail = line(p(0., 0., 0.), p(0., 0., 5.));
    let sections = [(0., 2.), (1., 1.), (2., 3.), (5., 1.5)].map(|(t, width)| SweepSection {
        parameter: t,
        curve: line(p(0., 0., t), p(width, 0., t)).to_nurbs().unwrap(),
    });
    let sweep = Sweep1::try_new(
        CurveRef::Line(&rail),
        &sections,
        Default::default(),
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = sweep.to_surface().unwrap();
    assert_eq!(
        surface.knots_u(),
        &[0., 0., 0., 0., 0.5, 1., 1.5, 1.75, 2., 2.25, 5., 5., 5., 5.]
    );
    for section in sections {
        for v in [0., 0.37, 1.] {
            let expected = section
                .curve
                .evaluate(section.curve.parameter_at(v).unwrap())
                .unwrap();
            assert!(
                surface
                    .evaluate(section.parameter, v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
        }
    }
}

#[test]
fn shifted_parameter_roundoff_cannot_silently_discard_an_interior_profile() {
    let start = 1e12;
    let rail = line(p(0., 0., 0.), p(0., 0., 5.))
        .to_nurbs()
        .unwrap()
        .try_reparameterized(start..=start + 5.)
        .unwrap();
    let sections =
        [(start, 2.), (start + 0.005, 1.), (start + 5., 3.)].map(|(parameter, width)| {
            let origin = rail.evaluate(parameter).unwrap();
            SweepSection {
                parameter,
                curve: line(origin, p(width, 0., origin.z())).to_nurbs().unwrap(),
            }
        });
    let sweep = Sweep1::try_new(
        CurveRef::NurbsCurve(&rail),
        &sections,
        Default::default(),
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap();
    // A near-Greville classification must never erase a supplied constraint.
    assert!(matches!(
        sweep.to_rail_basis_surface(),
        Err(GeometryError::InvalidSweep {
            context: "section interpolation lost a supplied profile"
        })
    ));
}

#[test]
fn rational_sweep_boundary_search_attains_the_half_space_distance_bound() {
    let rail = line(p(0., 0., 0.), p(0., 0., 5.));
    let sections = [(0., [1., 0.5, 2.]), (5., [2., 1., 1.])].map(|(t, weights)| SweepSection {
        parameter: t,
        curve: NurbsCurve::try_new_rational(
            2,
            [p(0., 0., t), p(1., 0.5, t), p(2., 0., t)]
                .into_iter()
                .zip(weights)
                .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    });
    let sweep = Sweep1::try_new(
        CurveRef::Line(&rail),
        &sections,
        Default::default(),
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = sweep.to_surface().unwrap();
    // Positive weights and nonnegative control Y bound the entire surface by
    // Y >= 0. A point directly across that plane is a certified minimum.
    assert!(
        surface
            .control_points()
            .iter()
            .all(|c| c.point().y() >= 0. && c.weight() > 0.)
    );
    for (x, z) in [(0., 2.5462962962962963), (2., 2.4537037037037033)] {
        let query = p(x, -0.2, z);
        let (u, v) = surface
            .closest_parameters(query, Tolerance::DEFAULT)
            .unwrap();
        let closest = surface.evaluate(u, v).unwrap();
        assert!(closest.distance_to(p(x, 0., z)).unwrap() < 1e-12);
        assert!((closest.distance_to(query).unwrap() - 0.2).abs() < 1e-14);
    }
}

#[test]
fn straight_sweeps_interpolate_local_and_global_section_blending() {
    let rail = line(p(0., 0., 0.), p(0., 0., 5.));
    let sections = [
        SweepSection {
            parameter: 0.,
            curve: line(p(0., 0., 0.), p(2., 0., 0.)).to_nurbs().unwrap(),
        },
        SweepSection {
            parameter: 5.,
            curve: line(p(0., 0., 5.), p(1., 0., 5.)).to_nurbs().unwrap(),
        },
    ];
    for blend in [SweepBlend::Local, SweepBlend::Global] {
        let sweep = Sweep1::try_new(
            CurveRef::Line(&rail),
            &sections,
            Default::default(),
            blend,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for surface in [
            sweep.to_surface().unwrap(),
            sweep.fit_model_surface().unwrap(),
        ] {
            for i in 0..=64 {
                let f = i as Real / 64.;
                let g = if blend == SweepBlend::Local {
                    f * f * (3. - 2. * f)
                } else {
                    f
                };
                for j in 0..=8 {
                    let v = j as Real / 8.;
                    assert!(
                        surface
                            .evaluate(f * 5., v)
                            .unwrap()
                            .distance_to(p((2. - g) * v, 0., f * 5.))
                            .unwrap()
                            < 1e-12
                    );
                }
            }
        }
    }
}

#[test]
fn curved_sweep_fits_analytic_radial_sections() {
    let circle = crate::Circle3::try_new(
        p(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let arc =
        crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
    let start = arc.start().unwrap();
    let outward = p(start.x() * 4. / 3., start.y() * 4. / 3., 0.);
    let sweep = Sweep1::try_new(
        CurveRef::Arc(&arc),
        &[SweepSection {
            parameter: 0.,
            curve: line(start, outward).to_nurbs().unwrap(),
        }],
        Default::default(),
        Default::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for surface in [
        sweep.to_surface().unwrap(),
        sweep.fit_model_surface().unwrap(),
    ] {
        for i in 0..=97 {
            let t = *arc.domain().end() * i as Real / 97.;
            let rail_point = arc.evaluate(t).unwrap();
            for j in 0..=11 {
                let ratio = 1. + j as Real / 33.;
                let expected = p(rail_point.x() * ratio, rail_point.y() * ratio, 0.);
                assert!(
                    surface
                        .evaluate(t, j as Real / 11.)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-9
                );
            }
        }
    }
}

#[test]
fn continuous_model_two_section_arc_blending_matches_an_independent_analytic_surface() {
    let circle = crate::Circle3::try_new(
        p(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let arc =
        crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
    let start = arc.start().unwrap();
    let end = arc.end().unwrap();
    let sections = [
        SweepSection {
            parameter: 0.,
            curve: line(start, p(start.x() * 4. / 3., start.y() * 4. / 3., 0.))
                .to_nurbs()
                .unwrap(),
        },
        SweepSection {
            parameter: *arc.domain().end(),
            curve: line(end, p(end.x() * 5. / 3., end.y() * 5. / 3., 0.))
                .to_nurbs()
                .unwrap(),
        },
    ];
    for blend in [SweepBlend::Local, SweepBlend::Global] {
        let sweep = Sweep1::try_new(
            CurveRef::Arc(&arc),
            &sections,
            Default::default(),
            blend,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = sweep.fit_model_surface().unwrap();
        for i in 0..=97 {
            let f = i as Real / 97.;
            let t = *arc.domain().end() * f;
            let width = 1.
                + if blend == SweepBlend::Local {
                    f * f * (3. - 2. * f)
                } else {
                    f
                };
            let rail_point = arc.evaluate(t).unwrap();
            for j in 0..=11 {
                let v = j as Real / 11.;
                let ratio = 1. + v * width / 3.;
                let expected = p(rail_point.x() * ratio, rail_point.y() * ratio, 0.);
                assert!(
                    surface
                        .evaluate(t, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-9
                );
            }
        }
    }
}

#[test]
fn prefix_arc_length_queries_preserve_forward_and_inverse_correspondence() {
    let rail = NurbsCurve::try_new(
        3,
        vec![p(0., 0., 0.), p(2., 0., 4.), p(3., 4., -2.), p(5., 2., 3.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let reference =
        crate::curve::ArcLengthSampler::try_new(CurveRef::NurbsCurve(&rail), Tolerance::DEFAULT)
            .unwrap();
    let mut cached =
        crate::curve::ArcLengthSampler::try_new(CurveRef::NurbsCurve(&rail), Tolerance::DEFAULT)
            .unwrap();
    cached.prepare_repeated_sampling(16).unwrap();
    for i in 0..=97 {
        let t = i as Real / 97.;
        let distance = cached.distance_at_parameter(t).unwrap();
        assert!((distance - reference.distance_at_parameter(t).unwrap()).abs() < 1e-11);
        assert!((cached.parameter_at_distance(distance).unwrap() - t).abs() < 1e-10);
    }
}

#[test]
fn mixed_section_bases_and_weights_retain_every_input_section() {
    let rail = line(p(0., 0., 0.), p(0., 0., 5.));
    let sections = [
        SweepSection {
            parameter: 0.,
            curve: line(p(0., 0., 0.), p(2., 0., 0.)).to_nurbs().unwrap(),
        },
        SweepSection {
            parameter: 2.,
            curve: NurbsCurve::try_new_rational(
                2,
                [
                    (p(0., 0., 2.), 1.),
                    (p(1., 1., 2.), 0.5),
                    (p(2., 0., 2.), 2.),
                ]
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .to_vec(),
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        },
        SweepSection {
            parameter: 5.,
            curve: line(p(0., 0., 5.), p(3., 0., 5.)).to_nurbs().unwrap(),
        },
    ];
    let sweep = Sweep1::try_new(
        CurveRef::Line(&rail),
        &sections,
        Default::default(),
        Default::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let output = sweep.sections_at(&[0., 2., 5.]).unwrap();
    let surfaces = [
        sweep.to_surface().unwrap(),
        sweep.fit_model_surface().unwrap(),
    ];
    for (source, target) in sections.iter().zip(output) {
        for i in 0..=32 {
            let f = i as Real / 32.;
            let expected = source
                .curve
                .evaluate(source.curve.parameter_at(f).unwrap())
                .unwrap();
            assert!(target.evaluate(f).unwrap().distance_to(expected).unwrap() < 1e-12);
            for surface in &surfaces {
                assert!(
                    surface
                        .evaluate(source.parameter, f)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-9
                );
            }
        }
    }
}

#[test]
fn continuous_model_spatial_roadlike_sections_keep_their_horizontal_witness() {
    let rail = NurbsCurve::try_new(
        3,
        [p(0., 0., 0.), p(2., 0., 4.), p(3., 4., -2.), p(5., 2., 3.)].to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let sweep = Sweep1::try_new(
        CurveRef::NurbsCurve(&rail),
        &[SweepSection {
            parameter: 0.,
            curve: line(p(0., 0., 0.), p(0., 1., 0.)).to_nurbs().unwrap(),
        }],
        SweepFrameStyle::Roadlike(UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap()),
        Default::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = sweep.fit_model_surface().unwrap();
    for i in 0..=47 {
        let t = i as Real / 47.;
        let point = surface.evaluate(t, 1.).unwrap();
        let origin = rail.evaluate(t).unwrap();
        assert!((point.z() - origin.z()).abs() < 1e-9);
        assert!((point.distance_to(origin).unwrap() - 1.).abs() < 1e-9);
    }
    let expected = p(
        5. + std::f64::consts::FRAC_1_SQRT_2,
        2. + std::f64::consts::FRAC_1_SQRT_2,
        3.,
    );
    assert!(
        surface
            .evaluate(1., 1.)
            .unwrap()
            .distance_to(expected)
            .unwrap()
            < 1e-12
    );
}

#[test]
fn sweep_rejects_invalid_stations_unimplemented_miters_and_axis_degeneracy() {
    let rail = line(p(0., 0., 0.), p(0., 0., 5.));
    let mut section = SweepSection {
        parameter: 0.,
        curve: line(p(0., 0., 0.), p(2., 0., 0.)).to_nurbs().unwrap(),
    };
    for parameter in [Real::NAN, -1., 5., 6.] {
        section.parameter = parameter;
        assert!(
            Sweep1::try_new(
                CurveRef::Line(&rail),
                &[section.clone()],
                Default::default(),
                Default::default(),
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
    section.parameter = 0.;
    assert!(
        Sweep1::try_new(
            CurveRef::Line(&rail),
            &[section.clone()],
            SweepFrameStyle::Roadlike(
                UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap()
            ),
            Default::default(),
            Tolerance::DEFAULT
        )
        .is_err()
    );
    let kink = crate::Polyline3::try_new(
        vec![p(0., 0., 0.), p(0., 0., 2.), p(0., 2., 2.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(
        Sweep1::try_new(
            CurveRef::Polyline(&kink),
            &[section],
            Default::default(),
            Default::default(),
            Tolerance::DEFAULT
        )
        .is_err()
    );
}

#[test]
fn unrefitted_arc_with_a_middle_section_preserves_the_rational_rail() {
    let circle = crate::Circle3::try_new(
        p(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let arc =
        crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
    let sections = [0., 0.5, 1.].map(|f| {
        let t = *arc.domain().end() * f;
        let a = arc.evaluate(t).unwrap();
        SweepSection {
            parameter: t,
            curve: line(a, p(a.x() * 4. / 3., a.y() * 4. / 3., 0.))
                .to_nurbs()
                .unwrap(),
        }
    });
    let sweep = Sweep1::try_new(
        CurveRef::Arc(&arc),
        &sections,
        Default::default(),
        Default::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = sweep.to_rail_basis_surface().unwrap();
    for i in 0..=97 {
        let t = *arc.domain().end() * i as Real / 97.;
        let u = CurveRef::Arc(&arc).nurbs_parameter(t).unwrap();
        assert!(
            surface
                .evaluate(u, 0.)
                .unwrap()
                .distance_to(arc.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn rational_rail_basis_sweep_is_invariant_to_common_weight_scale() {
    let mut reference = None::<NurbsSurface>;
    for scale in [1., 1e-280, 1e280] {
        let rail = NurbsCurve::try_new_rational(
            3,
            [
                (p(0., 0., 0.), 1.),
                (p(2., 0., 4.), 0.5),
                (p(3., 4., -2.), 2.),
                (p(5., 2., 3.), 1.),
            ]
            .into_iter()
            .map(|(point, weight)| WeightedPoint3::try_new(point, weight * scale).unwrap())
            .collect(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let sweep = Sweep1::try_new(
            CurveRef::NurbsCurve(&rail),
            &[SweepSection {
                parameter: 0.,
                curve: line(p(0., 0., 0.), p(0., 1., 0.)).to_nurbs().unwrap(),
            }],
            Default::default(),
            Default::default(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = sweep.to_rail_basis_surface().unwrap();
        for i in 0..=64 {
            let u = i as Real / 64.;
            assert!(
                surface
                    .evaluate(u, 0.)
                    .unwrap()
                    .distance_to(rail.evaluate(u).unwrap())
                    .unwrap()
                    < 1e-12
            );
            if let Some(reference) = &reference {
                for j in 0..=8 {
                    let v = j as Real / 8.;
                    assert!(
                        surface
                            .evaluate(u, v)
                            .unwrap()
                            .distance_to(reference.evaluate(u, v).unwrap())
                            .unwrap()
                            < 1e-11
                    );
                }
            }
        }
        reference.get_or_insert(surface);
    }
}

#[test]
fn rail_basis_sweep_keeps_the_native_rail_and_interpolates_transport_stations() {
    let rail = NurbsCurve::try_new(
        3,
        vec![p(0., 0., 0.), p(2., 0., 4.), p(3., 4., -2.), p(5., 2., 3.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let sweep = Sweep1::try_new(
        CurveRef::NurbsCurve(&rail),
        &[SweepSection {
            parameter: 0.,
            curve: line(p(0., 0., 0.), p(0., 1., 0.)).to_nurbs().unwrap(),
        }],
        Default::default(),
        Default::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = sweep.to_rail_basis_surface().unwrap();
    assert_eq!(surface.knots_u(), rail.knots());
    for i in 0..=64 {
        let t = i as Real / 64.;
        assert!(
            surface
                .evaluate(t, 0.)
                .unwrap()
                .distance_to(rail.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
    let parameters = [0., 1. / 3., 2. / 3., 1.];
    for (&t, section) in parameters
        .iter()
        .zip(sweep.sections_at(&parameters).unwrap())
    {
        assert!(
            surface
                .evaluate(t, 1.)
                .unwrap()
                .distance_to(section.evaluate(1.).unwrap())
                .unwrap()
                < 1e-12
        );
    }
    // Rail-basis preservation and rigid transport are different surface models.
    let rigid = sweep.sections_at(&[0., 0.5]).unwrap();
    assert!(
        surface
            .evaluate(0.5, 1.)
            .unwrap()
            .distance_to(rigid[1].evaluate(1.).unwrap())
            .unwrap()
            > 0.01
    );
}
