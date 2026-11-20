use viboceros_geometry::{
    AffineTransform3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::{
    TwoRailSweepOptions, two_rail_sweep_surface, two_rail_sweep_surface_with_options,
};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn curve(points: Vec<Point3>) -> NurbsCurve {
    let degree = points.len() - 1;
    NurbsCurve::try_new(
        degree,
        points,
        std::iter::repeat_n(0., degree + 1)
            .chain(std::iter::repeat_n(1., degree + 1))
            .collect(),
    )
    .unwrap()
}
fn inputs(bend: f64) -> ([NurbsCurve; 2], Vec<NurbsCurve>) {
    let rails = [0., 2.].map(|x| {
        if bend == 0. {
            curve(vec![p(x, 0., 0.), p(x, 0., 5.)])
        } else {
            curve(vec![p(x, 0., 0.), p(x, bend, 2.5), p(x, 0., 5.)])
        }
    });
    let mut sections = vec![curve(vec![p(0., 0., 0.), p(1., 1., 0.), p(2., 0., 0.)])];
    if bend != 0. {
        sections.push(curve(vec![
            p(0., bend / 2., 2.5),
            p(1., bend / 2. + 1., 2.5),
            p(2., bend / 2., 2.5),
        ]));
    }
    sections.push(curve(vec![p(0., 0., 5.), p(1., 1., 5.), p(2., 0., 5.)]));
    (rails, sections)
}
fn check(surface: &NurbsSurface, bend: f64, shift: Vector3) {
    for i in 0..=12 {
        for j in 0..=12 {
            let (u, v) = (i as f64 / 12., j as f64 / 12.);
            let expected = p(2. * v, 2. * bend * u * (1. - u) + 2. * v * (1. - v), 5. * u)
                .translated(shift)
                .unwrap();
            let actual = surface.evaluate(u, v).unwrap();
            assert!(
                actual.distance_to(expected).unwrap() < 2e-6,
                "{u} {v}: {actual:?} != {expected:?}"
            );
        }
    }
}
#[test]
fn native_two_rail_sweep_interpolates_rails_sections_and_preserves_inputs() {
    for bend in [0., 1.] {
        let (rails, sections) = inputs(bend);
        let before = format!("{rails:?}{sections:?}");
        let s =
            two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
        check(&s, bend, Vector3::try_new(0., 0., 0.).unwrap());
        assert_eq!(format!("{rails:?}{sections:?}"), before);
    }
}
#[test]
fn native_two_rail_sweep_orients_and_orders_independent_curve_domains() {
    let (mut rails, mut sections) = inputs(1.);
    rails[0] = rails[0].try_reparameterized(-7. ..=13.).unwrap();
    rails[1] = rails[1]
        .reversed()
        .unwrap()
        .try_reparameterized(42. ..=45.)
        .unwrap();
    sections.reverse();
    sections[0] = sections[0]
        .reversed()
        .unwrap()
        .try_reparameterized(10. ..=12.)
        .unwrap();
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    check(&s, 1., Vector3::try_new(0., 0., 0.).unwrap());
}

#[test]
fn native_two_rail_sweep_transports_one_arch_along_curved_and_skew_rails() {
    let profile = curve(vec![p(0., 0., 0.), p(1., 0.8, 0.), p(2., 0., 0.)]);
    let curved = [
        curve(vec![p(0., 0., 0.), p(1., 0., 3.), p(0., 0., 6.)]),
        curve(vec![p(2., 0., 0.), p(3., 0., 3.), p(6., 0., 6.)]),
    ];
    for maintain in [false, true] {
        let surface = two_rail_sweep_surface_with_options(
            &curved[0],
            &curved[1],
            std::slice::from_ref(&profile),
            TwoRailSweepOptions {
                maintain_height: maintain,
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        for i in 0..=8 {
            let u = f64::from(i) / 8.;
            // Rhino 8.32 kept constant height with MaintainHeight enabled
            // and scaled by the quadratic rail width otherwise.
            let height = if maintain { 0.4 } else { 0.4 + 0.8 * u * u };
            let actual = surface.evaluate(u, 0.5).unwrap();
            assert!(actual.distance_to(p(1. + 2. * u, height, 6. * u)).unwrap() < 1e-8);
        }
    }
    let skew = [
        curve(vec![p(0., 0., 0.), p(0., 0., 6.)]),
        curve(vec![p(2., 0., 0.), p(4., 2., 6.)]),
    ];
    let surface =
        two_rail_sweep_surface(&skew[0], &skew[1], &[profile], Tolerance::DEFAULT).unwrap();
    assert_eq!([surface.degree_u(), surface.degree_v()], [1, 2]);
    let middle_control = surface.control_point(1, 1).unwrap().point();
    let expected = p(1.2032920118407415, 2.5934159763185169, 6.1621169578302464);
    assert!(middle_control.distance_to(expected).unwrap() < 1e-8);
}

#[test]
fn native_two_rail_sweep_refit_maps_independent_profile_stations() {
    let rails = [
        curve(vec![p(0., 0., 0.), p(0., 0., 8.)]),
        curve(vec![p(2., 0., 8.), p(2., 0., 0.)])
            .try_reparameterized(-2. ..=7.)
            .unwrap(),
    ];
    let profiles = [
        curve(vec![p(0., 0., 0.), p(1., 0.8, 0.), p(2., 0., 0.)]),
        curve(vec![p(0., 0., 3.2), p(1., 1.5, 4.24), p(2., 0., 5.28)]),
        curve(vec![p(0., 0., 8.), p(1., 1.2, 8.), p(2., 0., 8.)]),
    ];
    let tolerance = Tolerance::DEFAULT;
    let automatic = two_rail_sweep_surface(&rails[0], &rails[1], &profiles, tolerance).unwrap();
    let surface = two_rail_sweep_surface_with_options(
        &rails[0],
        &rails[1],
        &profiles,
        TwoRailSweepOptions {
            refit_rails: true,
            ..Default::default()
        },
        tolerance,
    )
    .unwrap();
    assert_eq!(automatic, surface);
    assert_eq!(surface.domain_u(), 0. ..=8.);
    for (u, profile) in [0., 4.24, 8.].into_iter().zip(&profiles) {
        for v in [0., 0.25, 0.5, 0.75, 1.] {
            let reference = profile.evaluate(v).unwrap();
            assert!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(reference)
                    .unwrap()
                    < 1e-9
            );
        }
    }
    // Rhino 8.32, document absolute tolerance 0.001.
    for (u, y) in [
        (1., 0.42815116020261246),
        (4., 0.7408997519081474),
        (7., 0.6044386433718247),
    ] {
        let actual = surface.evaluate(u, 0.5).unwrap();
        assert!(
            actual.distance_to(p(1., y, u)).unwrap() < 1e-8,
            "{actual:?}"
        );
    }
}

#[test]
fn native_two_rail_sweep_refit_matches_curved_rhino_fixture_within_model_tolerance() {
    let rails = [
        curve(vec![p(0., 0., 0.), p(0., 3., 2.), p(1., 0., 8.)]),
        curve(vec![p(2., 0., 0.), p(6., -1., 5.), p(4., 0., 8.)]),
    ];
    let profiles = [
        curve(vec![p(0., 0., 0.), p(1., 0.8, 0.), p(2., 0., 0.)]),
        curve(vec![p(1., 0., 8.), p(2.5, 1.2, 8.), p(4., 0., 8.)]),
    ];
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1_f64.to_radians()).unwrap();
    let surface = two_rail_sweep_surface_with_options(
        &rails[0],
        &rails[1],
        &profiles,
        TwoRailSweepOptions {
            refit_rails: true,
            ..Default::default()
        },
        tolerance,
    )
    .unwrap();
    assert_eq!(surface.degree_u(), 3);
    let dz = *surface.domain_u().end();
    assert!((dz - 8.871950897010286).abs() < 2e-5);
    for (t, point) in [
        (
            0.125,
            [1.5406735845574164, 0.8474975176649883, 0.7782109536474813],
        ),
        (
            0.375,
            [2.398524350492659, 1.2425154634872606, 2.775444742670522],
        ),
        (
            0.625,
            [2.879416524857919, 1.1461089387741645, 4.90287651044745],
        ),
        (
            0.875,
            [2.7999163695429656, 0.8260814488784234, 7.0206808510504795],
        ),
    ] {
        let actual = surface.evaluate(dz * t, 0.5).unwrap();
        let distance = actual
            .distance_to(Point3::try_from(point).unwrap())
            .unwrap();
        assert!(distance < tolerance.absolute(), "{t}: {distance}");
    }
}

#[test]
fn native_two_rail_sweep_refit_certifies_rational_rail_images() {
    let rails = [0., 2.].map(|x| {
        NurbsCurve::try_new_rational(
            2,
            [
                (p(x, 1., 0.), 1.),
                (p(x, 1., 1.), std::f64::consts::FRAC_1_SQRT_2),
                (p(x, 0., 1.), 1.),
            ]
            .into_iter()
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
    });
    let profile = curve(vec![p(0., 1., 0.), p(1., 1.2, 0.), p(2., 1., 0.)]);
    let surface = two_rail_sweep_surface_with_options(
        &rails[0],
        &rails[1],
        std::slice::from_ref(&profile),
        TwoRailSweepOptions {
            refit_rails: true,
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(surface.degree_u(), 3);
    let end = *surface.domain_u().end();
    assert!((end - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
    let v = 0.5;
    assert!(
        surface
            .evaluate(0., v)
            .unwrap()
            .distance_to(profile.evaluate(v).unwrap())
            .unwrap()
            < 1e-9
    );
    for u in [0.2, 0.5, 0.8] {
        for (i, v) in [0., 1.].into_iter().enumerate() {
            let point = surface.evaluate(u * end, v).unwrap();
            let expected = p(2. * i as f64, (u * end).cos(), (u * end).sin());
            assert!(point.distance_to(expected).unwrap() < 1e-9);
        }
    }
}

#[test]
fn native_two_rail_sweep_keeps_the_profile_bracket_on_curved_rails() {
    let rails = [
        curve(vec![p(0., 0., 0.), p(0., 3., 2.), p(1., 0., 8.)]),
        curve(vec![p(2., 0., 0.), p(6., -1., 5.), p(4., 0., 8.)]),
    ];
    let profiles = [(0.2, 0.25, 0.8), (0.8, 0.85, 1.2)].map(|(u, v, h)| {
        let a = rails[0].evaluate(u).unwrap();
        let b = rails[1].evaluate(v).unwrap();
        curve(vec![
            a,
            p(
                a.x().midpoint(b.x()),
                a.y().midpoint(b.y()) + h,
                a.z().midpoint(b.z()),
            ),
            b,
        ])
    });
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1_f64.to_radians()).unwrap();
    let surface = two_rail_sweep_surface(&rails[0], &rails[1], &profiles, tolerance).unwrap();
    let explicit = two_rail_sweep_surface_with_options(
        &rails[0],
        &rails[1],
        &profiles,
        TwoRailSweepOptions {
            refit_rails: true,
            ..Default::default()
        },
        tolerance,
    )
    .unwrap();
    assert_eq!(surface, explicit);
    let end = *surface.domain_u().end();
    assert!((end - 4.9589270852607434).abs() < 2e-5);
    for (u, profile) in [0., end].into_iter().zip(&profiles) {
        for v in [0., 0.25, 0.5, 0.75, 1.] {
            let reference = profile.evaluate(v).unwrap();
            assert!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(reference)
                    .unwrap()
                    < 1e-7
            );
        }
    }
    // Native Rhino command, default refit off. Both rails were shortened.
    for (u, xyz) in [
        (
            0.25,
            [2.1868743397099615, 0.9490370333951599, 2.8222574614962213],
        ),
        (
            0.5,
            [2.4728862414563366, 1.0590472153505524, 4.004652298436081],
        ),
        (
            0.75,
            [2.618389694517425, 1.0645071721040424, 5.2050962399910485],
        ),
    ] {
        let actual = surface.evaluate(u * end, 0.5).unwrap();
        let delta = actual.distance_to(Point3::try_from(xyz).unwrap()).unwrap();
        assert!(delta < tolerance.absolute(), "{u}: {delta}");
    }
}

#[test]
fn native_two_rail_sweep_shortens_straight_rails_in_the_picked_direction() {
    for (offset, reverse_rail, reverse_profiles) in [
        (0., false, false),
        (1., false, false),
        (0., true, false),
        (0., false, true),
    ] {
        let rails = [
            curve(vec![p(0., 0., 0.), p(0., 0., 8.)]),
            if reverse_rail {
                curve(vec![p(2., 0., 8.), p(2., 0., 0.)])
            } else {
                curve(vec![p(2., 0., 0.), p(2., 0., 8.)])
            },
        ];
        let mut profiles = [(2., 0.8), (6., 1.2)].map(|(z, height)| {
            curve(vec![
                p(0., 0., z),
                p(1., height, z + offset / 2.),
                p(2., 0., z + offset),
            ])
        });
        if reverse_profiles {
            profiles.reverse();
        }
        let surface =
            two_rail_sweep_surface(&rails[0], &rails[1], &profiles, Tolerance::DEFAULT).unwrap();
        assert_eq!([surface.degree_u(), surface.degree_v()], [3, 2]);
        assert!((*surface.domain_u().end() - 4.).abs() < 1e-9);
        for (u, profile) in [0., 4.].into_iter().zip(&profiles) {
            for v in [0., 0.25, 0.5, 0.75, 1.] {
                assert!(
                    surface
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(profile.evaluate(v).unwrap())
                        .unwrap()
                        < 1e-9
                );
            }
        }
        let actual = surface.evaluate(1., 0.5).unwrap();
        let reference = if reverse_profiles {
            p(1., 0.56875, 5.)
        } else {
            p(1., 0.43125, 3. + offset / 2.)
        };
        assert!(actual.distance_to(reference).unwrap() < 1e-9);
    }
}
#[test]
fn native_two_rail_sweep_rejects_missing_duplicate_off_rail_and_crossed_sections() {
    let (rails, sections) = inputs(0.);
    for invalid in [
        vec![],
        vec![sections[0].clone(), sections[0].clone()],
        vec![
            sections[0].clone(),
            curve(vec![p(0., 0., 3.), p(3., 0., 4.)]),
        ],
        vec![
            sections[0].clone(),
            curve(vec![p(0., 0., 2.5), p(2., 0., 4.)]),
            curve(vec![p(0., 0., 3.5), p(2., 0., 3.)]),
            sections[1].clone(),
        ],
    ] {
        assert!(
            two_rail_sweep_surface(&rails[0], &rails[1], &invalid, Tolerance::DEFAULT).is_err()
        );
    }
}
#[test]
fn native_two_rail_sweep_certifies_remote_geometry() {
    let (rails, sections) = inputs(1.);
    let shift = Vector3::try_new(1e9, -2e9, 3e9).unwrap();
    let tr = AffineTransform3::from_translation(shift);
    let rails = rails.map(|r| r.transformed(tr).unwrap());
    let sections = sections
        .iter()
        .map(|r| r.transformed(tr).unwrap())
        .collect::<Vec<_>>();
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    check(&s, 1., shift);
}

#[test]
fn native_two_rail_sweep_weight_gauges_and_resource_failures_are_explicit() {
    let (mut rails, mut sections) = inputs(0.);
    for (i, c) in rails.iter_mut().chain(&mut sections).enumerate() {
        *c = NurbsCurve::try_new_rational(
            c.degree(),
            c.control_points()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(p.point(), if i % 2 == 0 { -3. } else { 7. }).unwrap()
                })
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap();
    }
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    check(&s, 0., Vector3::try_new(0., 0., 0.).unwrap());
    let mut controls = sections[0].control_points().to_vec();
    controls[1] = WeightedPoint3::try_new(controls[1].point(), 2.).unwrap();
    sections[0] = NurbsCurve::try_new_rational(2, controls, sections[0].knots().to_vec()).unwrap();
    let before = format!("{rails:?}{sections:?}");
    assert!(
        two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT)
            .unwrap_err()
            .to_string()
            .contains("polynomial")
    );
    assert!(
        two_rail_sweep_surface(
            &rails[0],
            &rails[1],
            &vec![sections[1].clone(); 33],
            Tolerance::DEFAULT
        )
        .is_err()
    );
    assert_eq!(format!("{rails:?}{sections:?}"), before);
    let (rails, mut sections) = inputs(1.);
    sections.remove(1);
    assert!(
        two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT)
            .unwrap_err()
            .to_string()
            .contains("rail degree plus one")
    );
}

#[test]
fn native_two_rail_sweep_blends_different_cross_section_shapes() {
    let (rails, mut sections) = inputs(0.);
    sections[1] = curve(vec![p(0., 0., 5.), p(1., 2., 5.), p(2., 0., 5.)]);
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    for i in 0..=16 {
        for j in 0..=12 {
            let (u, v) = (i as f64 / 16., j as f64 / 12.);
            let expected = p(2. * v, 2. * (1. + u) * v * (1. - v), 5. * u);
            assert!(s.evaluate(u, v).unwrap().distance_to(expected).unwrap() < 1e-9);
        }
    }
}
