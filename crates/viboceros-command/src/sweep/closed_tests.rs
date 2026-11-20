use super::*;

fn radial(profile: (Real, Real)) -> NurbsCurve {
    let (quarter, height) = profile;
    let (sin, cos) = (quarter * std::f64::consts::FRAC_PI_2).sin_cos();
    let point = |r, z| Point3::try_new(r * cos, r * sin, z).unwrap();
    if height == 0. {
        NurbsCurve::try_new(1, vec![point(1., 0.), point(3., 0.)], vec![0., 0., 2., 2.]).unwrap()
    } else {
        NurbsCurve::try_new(
            2,
            vec![point(1., 0.), point(2., height), point(3., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
    }
}

fn fixture(profiles: &[(Real, Real)], reverse: bool) -> (Document, ObjectId) {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Circle 0,0,0 1").unwrap();
    let object = doc.objects().next().unwrap();
    let id = object.id();
    if reverse {
        let reversed = object
            .geometry_snapshot()
            .nurbs_curve_representation()
            .unwrap()
            .unwrap()
            .reversed()
            .unwrap();
        doc.replace_object_geometries([(id, Geometry::NurbsCurve(reversed))])
            .unwrap();
    }
    let ids = profiles
        .iter()
        .map(|&profile| {
            doc.add_geometry(Geometry::NurbsCurve(radial(profile)))
                .unwrap()
        })
        .collect::<Vec<_>>();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}

fn sweep<'a>(doc: &'a mut Document, rail: ObjectId, options: &str) -> &'a Brep {
    CommandRegistry::with_builtins()
        .execute(doc, &format!("Sweep1 RailId={rail} {options}"))
        .unwrap();
    let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
        panic!("expected Sweep1 B-rep")
    };
    brep
}

#[test]
fn sweep_one_closed_rail_wraps_profiles_and_matches_native_retained_faces() {
    // Rhino 8.32's retained rail basis, including the wrapped last-to-first span.
    let cases = [
        (
            &[(0., 0.5), (1., 0.8)][..],
            false,
            &[
                [0.5, 0.65, 0.8],
                [0.8, 0.8013480038173294, 0.7222222222222223],
                [0.7222222222222223, 0.65, 0.5777777777777778],
                [0.5777777777777778, 0.49865199618267042, 0.5],
            ][..],
            26.840998622947964,
        ),
        (
            &[(2., 0.5), (3., 0.8), (0., 0.6)][..],
            true,
            &[
                [0.5, 0.65, 0.8],
                [0.8, 0.7, 0.6],
                [0.6, 0.59763325214724783, 0.55],
                [0.55, 0.50236674785275204, 0.5],
            ][..],
            26.655658096240359,
        ),
    ];
    for (profiles, reverse, heights, expected_area) in cases {
        let (mut doc, rail) = fixture(profiles, reverse);
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let tolerance = doc.tolerance();
        let brep = sweep(&mut doc, rail, "Closed=_Yes");
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (4, 12, 8)
        );
        assert_eq!(
            brep.edge_use_counts().iter().filter(|&&n| n == 2).count(),
            4
        );
        for (face, expected) in brep.faces().iter().zip(heights) {
            let surface = face.surface();
            assert_eq!((surface.degree_u(), surface.degree_v()), (2, 2));
            for (row, height) in expected.iter().enumerate() {
                let actual = surface.control_point(row, 1).unwrap().point().z();
                assert!((actual - height).abs() < 1e-8, "{actual} != {height}");
            }
        }
        let area = brep.area(tolerance).unwrap();
        assert!(
            (area - expected_area).abs() < 1e-5,
            "{area} != {expected_area}"
        );
        assert_eq!(doc.undo_label(), Some("Sweep1"));
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn sweep_one_closed_rail_refits_a_periodic_surface_and_pins_profiles() {
    for (profiles, options, sample) in [
        (
            [(0., 0.5), (1., 0.8)],
            "Closed=Yes RefitRail=Yes",
            [1.4142154540178604, 1.4142116707258003, 0.325],
        ),
        (
            [(0.3, 0.5), (2.6, 0.8)],
            "Closed=Yes ShapeBlending=Global",
            [0.6179968737701072, 1.9020973759726456, 0.26818306879770337],
        ),
    ] {
        let (mut doc, rail) = fixture(&profiles, false);
        let brep = sweep(&mut doc, rail, options);
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (1, 3, 2)
        );
        let surface = brep.faces()[0].surface();
        assert!(surface.is_periodic_u());
        let u = surface.domain_u();
        let v = surface.domain_v();
        let v_mid = v.start().midpoint(*v.end());
        let actual = surface
            .evaluate((u.end() - u.start()) / 8. + u.start(), v_mid)
            .unwrap();
        assert!(
            actual
                .distance_to(Point3::try_from(sample).unwrap())
                .unwrap()
                < 8e-5,
            "{actual:?}"
        );
        let first = radial(profiles[0]);
        let along = first.domain();
        let expected = first
            .evaluate(along.start().midpoint(*along.end()))
            .unwrap();
        assert!(
            surface
                .evaluate(*u.start(), v_mid)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-8
        );
        let second = radial(profiles[1]);
        let along = second.domain();
        let expected = second
            .evaluate(along.start().midpoint(*along.end()))
            .unwrap();
        let station = (profiles[1].0 - profiles[0].0) * std::f64::consts::FRAC_PI_2;
        assert!(
            surface
                .evaluate(station, v_mid)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn sweep_one_closed_rail_keeps_authored_direction_for_two_picks() {
    for (profiles, reverse, faces) in [
        ([(0., 0.), (3., 0.)], false, 3),
        ([(0., 0.), (3., 0.)], true, 1),
        ([(3., 0.), (0., 0.)], false, 1),
        ([(2., 0.), (1., 0.)], false, 3),
    ] {
        let (mut doc, rail) = fixture(&profiles, reverse);
        let tolerance = doc.tolerance();
        let brep = sweep(&mut doc, rail, "Closed=No");
        assert_eq!(brep.faces().len(), faces);
        let area = brep.area(tolerance).unwrap();
        assert!((area - faces as Real * std::f64::consts::TAU).abs() < 1e-5);
    }
}

#[test]
fn sweep_one_closed_rail_rejects_point_ends_and_open_or_incomplete_rails() {
    let (mut doc, rail) = fixture(&[(0., 0.)], false);
    let commands = CommandRegistry::with_builtins();
    assert!(
        commands
            .execute(&mut doc, &format!("Sweep1 RailId={rail} Closed=Yes"))
            .is_err()
    );
    let second = doc
        .add_geometry(Geometry::NurbsCurve(radial((1., 0.))))
        .unwrap();
    doc.select_objects_direct([second], SelectionMode::Add)
        .unwrap();
    commands.execute(&mut doc, "Line 1,0,0 0,1,0").unwrap();
    let line = doc.objects().last().unwrap().id();
    doc.clear_history().unwrap();
    let originals = doc.objects().cloned().collect::<Vec<_>>();
    for input in [
        format!("Sweep1 RailId={rail} Closed=Yes EndPoint=1,0,0"),
        format!("Sweep1 RailId={line} Closed=Yes"),
        format!("Sweep1 RailId={rail} Closed=Yes Closed=No"),
        format!("Sweep1 RailId={rail} Closed=Yes Parameters=0,500"),
    ] {
        assert!(commands.execute(&mut doc, &input).is_err(), "{input}");
        assert_eq!(doc.undo_label(), None);
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), originals);
    }
}

#[test]
fn sweep_one_closed_rail_with_one_profile_uses_the_full_circuit() {
    for (quarter, faces) in [(1., 4), (0.3, 5)] {
        let profile = (quarter, 0.5);
        let (mut doc, rail) = fixture(&[profile], false);
        let brep = sweep(&mut doc, rail, "Closed=No");
        assert_eq!((brep.faces().len(), brep.edges().len()), (faces, 3 * faces));
        let surface = brep.faces()[0].surface();
        let v = surface.domain_v();
        let actual = surface
            .evaluate(*surface.domain_u().start(), v.start().midpoint(*v.end()))
            .unwrap();
        let expected = radial(profile).evaluate(0.5).unwrap();
        assert!(actual.distance_to(expected).unwrap() < 1e-8);
    }
}

#[test]
fn sweep_one_closed_rail_with_a_closed_profile_creates_a_cubic_torus() {
    let (mut doc, rail) = fixture(&[], false);
    let id = add_closed_profile(&mut doc, 0.3, 0.5);
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let brep = sweep(&mut doc, rail, "Closed=No");
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (1, 2, 1)
    );
    assert!(brep.is_solid());
    let surface = brep.faces()[0].surface();
    assert_eq!((surface.degree_u(), surface.degree_v()), (3, 2));
    assert!(!surface.is_periodic_u());
}

#[test]
fn sweep_one_closed_rail_refits_closed_profiles_at_knots() {
    let (mut doc, rail) = fixture(&[], false);
    let ids = [(0., 0.5), (1., 0.65)]
        .map(|(quarter, radius)| add_closed_profile(&mut doc, quarter, radius));
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    for (option, closed, expected_edges) in [("Closed=No", false, 3), ("Closed=Yes", true, 2)] {
        let brep = sweep(&mut doc, rail, option);
        assert_eq!(
            (brep.faces().len(), brep.edges().len()),
            (1, expected_edges)
        );
        let surface = brep.faces()[0].surface();
        assert_eq!((surface.degree_u(), surface.degree_v()), (3, 2));
        assert_eq!(surface.is_periodic_u(), closed);
        assert_eq!(brep.is_solid(), closed);
        doc.undo().unwrap();
    }
}

fn add_closed_profile(doc: &mut Document, quarter: Real, radius: Real) -> ObjectId {
    let (sin, cos) = (quarter * std::f64::consts::FRAC_PI_2).sin_cos();
    let tolerance = doc.tolerance();
    let profile = viboceros_geometry::Circle3::try_new(
        Point3::try_new((1. + radius) * cos, (1. + radius) * sin, 0.).unwrap(),
        radius,
        UnitVector3::try_new(-sin, cos, 0., tolerance).unwrap(),
        tolerance,
    )
    .unwrap();
    doc.add_geometry(Geometry::Circle(profile)).unwrap()
}
