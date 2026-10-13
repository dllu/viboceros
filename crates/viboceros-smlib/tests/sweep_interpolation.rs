#![cfg(feature = "native")]
use viboceros_geometry::{
    Circle3, CurveRef, LineSegment, NurbsCurve, NurbsSurface, Point3, Sweep1, SweepBlend,
    SweepFrameStyle, SweepSection, Tolerance, UnitVector3, WeightedPoint3,
};
use viboceros_smlib::{fit_sweep_model_surface, sweep_model, sweep_surface};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn profile(z: f64) -> NurbsCurve {
    NurbsCurve::try_new(1, vec![p(0., 0., z), p(1., 0., z)], vec![0., 0., 1., 1.]).unwrap()
}
fn compare(a: &NurbsSurface, b: &NurbsSurface) {
    assert_eq!((a.degree_u(), a.degree_v()), (b.degree_u(), b.degree_v()));
    assert_eq!(a.knots_u(), b.knots_u());
    assert_eq!(a.knots_v(), b.knots_v());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!(
            (a.weight() - b.weight()).abs() < 1e-9 + 1e-12 * a.weight().abs().max(b.weight().abs())
        );
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-8);
    }
    for j in 0..=16 {
        for i in 0..=16 {
            let a = a
                .evaluate(
                    a.parameter_at_u(i as f64 / 16.).unwrap(),
                    a.parameter_at_v(j as f64 / 16.).unwrap(),
                )
                .unwrap();
            let b = b
                .evaluate(
                    b.parameter_at_u(i as f64 / 16.).unwrap(),
                    b.parameter_at_v(j as f64 / 16.).unwrap(),
                )
                .unwrap();
            assert!(a.distance_to(b).unwrap() < 1e-8);
        }
    }
}
#[test]
fn native_retained_and_refitted_sweeps_cover_high_degree_rails_and_interior_stations() {
    for degree in [1, 3, 5, 11] {
        let rail = NurbsCurve::try_new(
            degree,
            (0..=degree)
                .map(|i| p(0., 0., 5. * i as f64 / degree as f64))
                .collect(),
            [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
        )
        .unwrap();
        let sections = [
            SweepSection {
                parameter: 0.,
                curve: profile(0.),
            },
            SweepSection {
                parameter: 0.4,
                curve: profile(2.),
            },
            SweepSection {
                parameter: 1.,
                curve: profile(5.),
            },
        ];
        let before = (rail.clone(), sections.clone());
        for blend in [SweepBlend::Local, SweepBlend::Global] {
            let native = sweep_model(
                CurveRef::NurbsCurve(&rail),
                &sections,
                SweepFrameStyle::Freeform,
                blend,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let portable = Sweep1::try_new(
                CurveRef::NurbsCurve(&rail),
                &sections,
                SweepFrameStyle::Freeform,
                blend,
                Tolerance::DEFAULT,
            )
            .unwrap();
            compare(
                &sweep_surface(&native, false).unwrap(),
                &portable.to_rail_basis_surface().unwrap(),
            );
            compare(
                &sweep_surface(&native, true).unwrap(),
                &portable.to_surface().unwrap(),
            );
        }
        assert_eq!((rail, sections), before);
    }
}
#[test]
fn native_closed_rail_sections_and_continuous_model_fits_match_portable_geometry() {
    let circle = Circle3::try_new(
        p(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let section = SweepSection {
        parameter: *circle.domain().start(),
        curve: NurbsCurve::try_new(1, vec![p(3., 0., -1.), p(3., 0., 1.)], vec![0., 0., 1., 1.])
            .unwrap(),
    };
    let native = sweep_model(
        CurveRef::Circle(&circle),
        std::slice::from_ref(&section),
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let portable = Sweep1::try_new(
        CurveRef::Circle(&circle),
        &[section],
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for refit in [false, true] {
        let a = sweep_surface(&native, refit).unwrap();
        assert!(a.is_closed_u().unwrap());
        compare(
            &a,
            &if refit {
                portable.to_surface().unwrap()
            } else {
                portable.to_rail_basis_surface().unwrap()
            },
        );
    }
    compare(
        &fit_sweep_model_surface(&native).unwrap(),
        &portable.fit_model_surface().unwrap(),
    );
}
#[test]
fn native_homogeneous_section_solves_keep_common_weight_gauges_and_tiny_channels() {
    let rail = LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap();
    for gauge in [1e-300, 1., 1e300] {
        for tiny in [1e-18, 1e-180, 1e-280] {
            let sections = [0., 5.].map(|z| SweepSection {
                parameter: z,
                curve: NurbsCurve::try_new_rational(
                    1,
                    vec![
                        WeightedPoint3::try_new(p(0., 0., z), gauge).unwrap(),
                        WeightedPoint3::try_new(p(1., tiny, z), 2. * gauge).unwrap(),
                    ],
                    vec![0., 0., 1., 1.],
                )
                .unwrap(),
            });
            let native = sweep_model(
                CurveRef::Line(&rail),
                &sections,
                SweepFrameStyle::Freeform,
                SweepBlend::Local,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let portable = Sweep1::try_new(
                CurveRef::Line(&rail),
                &sections,
                SweepFrameStyle::Freeform,
                SweepBlend::Local,
                Tolerance::DEFAULT,
            )
            .unwrap();
            for refit in [false, true] {
                let a = sweep_surface(&native, refit).unwrap();
                let b = if refit {
                    portable.to_surface().unwrap()
                } else {
                    portable.to_rail_basis_surface().unwrap()
                };
                compare(&a, &b);
                for (a, b) in a.control_points().iter().zip(b.control_points()) {
                    assert!((a.point().y() / tiny - b.point().y() / tiny).abs() < 1e-10);
                }
            }
        }
    }
}
