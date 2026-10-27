use viboceros_geometry::{
    Circle3, CircularArc3, CurveRef, LineSegment, NurbsCurve, Point3, Polyline3, Sweep1,
    SweepBlend, SweepFrameStyle, SweepSection, Tolerance, Vector3,
};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn line(a: [f64; 3], b: [f64; 3]) -> NurbsCurve {
    LineSegment::try_new(point(a), point(b), Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap()
}
fn model(rail: CurveRef<'_>, sections: &[SweepSection]) -> Sweep1 {
    Sweep1::try_new(
        rail,
        sections,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn full_rail_anchor_retains_interior_profile_and_backwards_frame_transport() {
    let arc = CircularArc3::try_from_three_points(
        point([3., 0., 0.]),
        point([3. / 2_f64.sqrt(), 3. / 2_f64.sqrt(), 0.]),
        point([0., 3., 0.]),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let t = CurveRef::Arc(&arc).parameter_at(0.5).unwrap();
    let profile = line(
        [3. / 2_f64.sqrt(), 3. / 2_f64.sqrt(), 0.],
        [4. / 2_f64.sqrt(), 4. / 2_f64.sqrt(), 0.],
    );
    let before = profile.clone();
    let sweep = model(
        CurveRef::Arc(&arc),
        &[SweepSection {
            parameter: t,
            curve: profile.clone(),
        }],
    )
    .try_on_full_rail()
    .unwrap();
    assert_eq!(sweep.domain(), CurveRef::Arc(&arc).domain());
    let sections = sweep.sections_at(&[0., t, *sweep.domain().end()]).unwrap();
    assert!(
        sections[1]
            .parameter_curve_deviation_bound(&profile, 1e-12)
            .unwrap()
            .is_some()
    );
    for (s, ends) in [
        (&sections[0], [[3., 0., 0.], [4., 0., 0.]]),
        (&sections[2], [[0., 3., 0.], [0., 4., 0.]]),
    ] {
        for (p, q) in s.control_points().iter().zip(ends) {
            assert!(p.point().distance_to(point(q)).unwrap() < 1e-12);
        }
    }
    assert_eq!(profile, before);
}

#[test]
fn expanding_the_rail_rechecks_corners_outside_the_original_sweep_interval() {
    let rail = Polyline3::try_new(
        vec![
            point([0., 0., 0.]),
            point([0., 0., 1.]),
            point([1., 0., 1.]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sweep = model(
        CurveRef::Polyline(&rail),
        &[SweepSection {
            parameter: 1.5,
            curve: line([0.5, 0., 1.], [0.5, 1., 1.]),
        }],
    );
    assert!(sweep.try_on_full_rail().is_err());
}

#[test]
fn closed_rail_anchoring_checks_and_retains_the_surface_seam() {
    let circle = Circle3::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        3.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let t = CurveRef::Circle(&circle).parameter_at(0.25).unwrap();
    let profile = line([0., 3., -1.], [0., 3., 1.]);
    let sweep = model(
        CurveRef::Circle(&circle),
        &[SweepSection {
            parameter: t,
            curve: profile,
        }],
    )
    .try_on_full_rail()
    .unwrap();
    let surface = sweep.to_rail_basis_surface().unwrap();
    let ends = [
        surface.isocurve_v(*surface.domain_u().start()).unwrap(),
        surface.isocurve_v(*surface.domain_u().end()).unwrap(),
    ];
    assert!(
        ends[0]
            .parameter_curve_deviation_bound(&ends[1], 1e-9)
            .unwrap()
            .is_some()
    );
}
