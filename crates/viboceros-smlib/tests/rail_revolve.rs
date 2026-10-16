#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, UnitVector3, WeightedPoint3};
use viboceros_smlib::rail_revolved_surface;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn profile(y: f64) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            (p(2., 0., 2.), 1.),
            (p(3., y, 4.), 0.8),
            (p(1., 0., 5.), 1.),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![-7., -7., -7., 13., 13., 13.],
    )
    .unwrap()
}
fn rail(varying: bool) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            (p(2., 0., 2.), 1.),
            (p(2., 2., if varying { 3. } else { 2. }), 0.5_f64.sqrt()),
            (p(0., 2., if varying { 4. } else { 2. }), 1.),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![42., 42., 42., 45., 45., 45.],
    )
    .unwrap()
}
#[test]
fn native_rail_revolution_retains_rational_basis_and_follows_height_without_mutating_sources() {
    for y in [0., 0.5] {
        for varying in [false, true] {
            for scale in [false, true] {
                let profile = profile(y);
                let rail = rail(varying);
                let before = format!("{profile:?}{rail:?}");
                let s = rail_revolved_surface(
                    &profile,
                    &rail,
                    p(0., 0., 0.),
                    UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
                    scale,
                    Tolerance::DEFAULT,
                )
                .unwrap();
                assert_eq!(s.domain_u(), profile.domain());
                assert_eq!(s.domain_v(), rail.domain());
                for i in 0..=12 {
                    for j in 0..=12 {
                        let u = profile.parameter_at(i as f64 / 12.).unwrap();
                        let v = rail.parameter_at(j as f64 / 12.).unwrap();
                        let a = profile.evaluate(u).unwrap();
                        let b = rail.evaluate(v).unwrap();
                        let expected = p(
                            (a.x() * b.x() - a.y() * b.y()) / 2.,
                            (a.x() * b.y() + a.y() * b.x()) / 2.,
                            if scale {
                                a.z() * b.z() / 2.
                            } else {
                                a.z() + b.z() - 2.
                            },
                        );
                        let actual = s.evaluate(u, v).unwrap();
                        assert!(
                            actual.distance_to(expected).unwrap() < 1e-10,
                            "{varying} {scale} {u} {v}: {actual:?} {expected:?}"
                        );
                    }
                }
                assert_eq!(format!("{profile:?}{rail:?}"), before);
            }
        }
    }
}
#[test]
fn native_rail_revolution_rejects_axis_starts_and_translates_at_zero_reference_height() {
    let axis = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
    assert!(
        rail_revolved_surface(
            &profile(0.),
            &rail(false),
            p(2., 0., 0.),
            axis,
            false,
            Tolerance::DEFAULT
        )
        .is_err()
    );
    let a = rail_revolved_surface(
        &profile(0.),
        &rail(true),
        p(0., 0., 2.),
        axis,
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = rail_revolved_surface(
        &profile(0.),
        &rail(true),
        p(0., 0., 2.),
        axis,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn native_rail_revolution_height_fallback_uses_the_observed_numeric_boundary() {
    let axis = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
    let boundary = 2_f64.powi(-26);
    for h in [
        0.,
        boundary * (1. - 2_f64.powi(-26)),
        boundary,
        -boundary,
        boundary * (1. + 2_f64.powi(-26)),
    ] {
        let origin = p(0., 0., 2. - h);
        let surface = rail_revolved_surface(
            &profile(0.),
            &rail(true),
            origin,
            axis,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let actual = surface.evaluate(13., 45.).unwrap();
        let h = 2. - origin.z();
        let expected = if h.abs() <= boundary {
            7.
        } else {
            origin.z() + (5. - origin.z()) * (4. - origin.z()) / h
        };
        assert!(
            (actual.z() - expected).abs() <= expected.abs() * 1e-15,
            "h={h}: {actual:?}, z={expected}"
        );
    }
}

#[test]
fn native_rail_revolution_is_invariant_under_common_signed_weight_gauges() {
    fn gauge(c: &NurbsCurve, g: f64) -> NurbsCurve {
        NurbsCurve::try_new_rational(
            c.degree(),
            c.control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * g).unwrap())
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap()
    }
    let a = profile(0.5);
    let b = rail(true);
    let axis = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
    let reference =
        rail_revolved_surface(&a, &b, p(0., 0., 0.), axis, true, Tolerance::DEFAULT).unwrap();
    let actual = rail_revolved_surface(
        &gauge(&a, -1e100),
        &gauge(&b, -1e-100),
        p(0., 0., 0.),
        axis,
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for i in 0..=6 {
        for j in 0..=6 {
            let u = a.parameter_at(i as f64 / 6.).unwrap();
            let v = b.parameter_at(j as f64 / 6.).unwrap();
            assert!(
                actual
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(reference.evaluate(u, v).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
}

#[test]
fn native_rail_revolution_splits_profile_kinks_with_shared_brep_topology() {
    let a = NurbsCurve::try_new(
        1,
        vec![p(2., 0., 2.), p(3., 0., 3.), p(1., 0., 5.)],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let b = viboceros_smlib::rail_revolved_brep(
        &a,
        &rail(false),
        p(0., 0., 0.),
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(b.faces().len(), 2);
    assert!((0..b.edges().len()).any(|index| {
        b.faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .filter(|t| t.edge() == Some(index))
            .count()
            > 1
    }));
}

#[test]
fn native_rail_revolution_retains_collapsed_profile_ends_as_singular_trims() {
    let a = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(2., 0., 2.), p(0., 0., 4.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let b = viboceros_smlib::rail_revolved_brep(
        &a,
        &rail(false),
        p(0., 0., 0.),
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(b.faces().len(), 1);
    assert_eq!(
        b.faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .filter(|t| t.edge().is_none())
            .count(),
        2
    );
}

#[test]
fn native_rail_revolution_completes_the_native_clamped_controls_for_unclamped_rational_inputs() {
    let a = NurbsCurve::try_new_rational(
        2,
        profile(0.5).control_points().to_vec(),
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let b = NurbsCurve::try_new_rational(
        2,
        rail(true).control_points().to_vec(),
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let start = b.evaluate(0.).unwrap();
    let radius2 = start.x().powi(2) + start.y().powi(2);
    assert!(
        rail_revolved_surface(
            &a,
            &b,
            p(0., 0., 0.),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            true,
            Tolerance::DEFAULT
        )
        .is_err()
    );
    {
        let scale = false;
        let s = rail_revolved_surface(
            &a,
            &b,
            p(0., 0., 0.),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            scale,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = i as f64 / 8.;
                let v = j as f64 / 8.;
                let q = a.evaluate(u).unwrap();
                let r = b.evaluate(v).unwrap();
                let x = q.x() * r.x() - q.y() * r.y();
                let y = q.x() * r.y() + q.y() * r.x();
                let expected = p(
                    (x * start.x() + y * start.y()) / radius2,
                    (y * start.x() - x * start.y()) / radius2,
                    if scale {
                        q.z() * r.z() / start.z()
                    } else {
                        q.z() + r.z() - start.z()
                    },
                );
                assert!(
                    s.evaluate(u, v).unwrap().distance_to(expected).unwrap() < 1e-10,
                    "{scale} {u} {v}"
                );
            }
        }
    }
}
