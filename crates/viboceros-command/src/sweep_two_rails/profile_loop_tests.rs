use super::*;
use viboceros_geometry::{Circle3, UnitVector3};

fn p(xyz: [f64; 3]) -> Point3 {
    Point3::try_from(xyz).unwrap()
}

fn circle(center: [f64; 3], radius: f64, x: [f64; 3], normal: [f64; 3]) -> Geometry {
    let tolerance = Tolerance::DEFAULT;
    let axis = |[x, y, z]: [f64; 3]| UnitVector3::try_new(x, y, z, tolerance).unwrap();
    Geometry::Circle(
        Circle3::try_from_frame(p(center), radius, axis(x), axis(normal), tolerance).unwrap(),
    )
}

fn circle_at(y: f64, radius: f64) -> Geometry {
    circle([0., y, 0.], radius, [1., 0., 0.], [0., -1., 0.])
}

fn line(a: [f64; 3], b: [f64; 3]) -> Geometry {
    Geometry::Line(LineSegment::try_new(p(a), p(b), Tolerance::DEFAULT).unwrap())
}

fn pick(document: &mut Document, curves: impl IntoIterator<Item = Geometry>) {
    let ids = curves
        .into_iter()
        .map(|curve| document.add_geometry(curve).unwrap())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
}

fn last_brep(doc: &Document) -> &Brep {
    let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    brep
}

#[test]
fn sweep_two_rail_closed_profiles_match_native_straight_rail_fixtures() {
    let registry = CommandRegistry::with_builtins();
    // Rhino 8.32 command captures. Smooth closed profiles form one face with
    // a seam and two bare boundary loops, even with asymmetric guide contacts.
    for (name, rails, profiles, area) in [
        (
            "single",
            [[1., 1., 0.], [-1., -1., 0.]],
            &[(0., 1.)][..],
            31.415926923587868,
        ),
        (
            "interior",
            [[1., 1., 0.], [-1., -1., 0.]],
            &[(2.5, 1.)][..],
            15.707963461793939,
        ),
        (
            "two",
            [[1., 1., 0.], [-1., -1., 0.]],
            &[(0., 1.), (5., 1.)][..],
            31.415926923587868,
        ),
        (
            "taper",
            [[1., 2., 0.], [-1., -2., 0.]],
            &[(0., 1.), (5., 2.)][..],
            48.057127326293809,
        ),
        (
            "eccentric",
            [[1., 1., 0.], [0., 0., 1.]],
            &[(0., 1.)][..],
            31.415926923587861,
        ),
        (
            "disjoint guide",
            [[1., 1., 0.], [-1.3, -1.3, 0.]],
            &[(0., 1.)][..],
            31.415926923587886,
        ),
    ] {
        let mut doc = Document::default();
        pick(
            &mut doc,
            rails
                .map(|[a, b, z]| line([a, 0., z], [b, 5., z]))
                .into_iter()
                .chain(profiles.iter().map(|&(y, radius)| circle_at(y, radius))),
        );
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, "Sweep2")
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let result = last_brep(&doc);
        assert_eq!(
            (result.faces().len(), result.edges().len()),
            (1, 3),
            "{name}"
        );
        let surface = result.faces()[0].surface();
        assert_eq!([surface.degree_u(), surface.degree_v()], [3, 2]);
        assert!(surface.is_closed_v().unwrap());
        let actual = result.area(doc.tolerance()).unwrap();
        assert!((actual - area).abs() < 1e-5, "{name}: {actual} != {area}");
        assert_eq!(doc.undo_label(), Some("Sweep2"));
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn sweep_two_rail_closed_profile_kinks_split_faces_and_mixed_profiles_fail() {
    let mut doc = Document::default();
    let rail1 = line([1., 0., 1.], [1., 5., 1.]);
    let rail2 = line([-1., 0., -1.], [-1., 5., -1.]);
    let corners = [[1., 1.], [-1., 1.], [-1., -1.], [1., -1.], [1., 1.]];
    let profile = |y| {
        Geometry::NurbsCurve(
            NurbsCurve::try_new(
                1,
                corners.map(|[x, z]| p([x, y, z])).into(),
                vec![0., 0., 0.25, 0.5, 0.75, 1., 1.],
            )
            .unwrap(),
        )
    };
    pick(
        &mut doc,
        [rail1.clone(), rail2.clone(), profile(0.), profile(5.)],
    );
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Sweep2 Natural").unwrap();
    let result = last_brep(&doc);
    assert_eq!((result.faces().len(), result.edges().len()), (4, 12));
    assert!((result.area(doc.tolerance()).unwrap() - 40.).abs() < 1e-8);

    let mut mixed = Document::default();
    pick(
        &mut mixed,
        [
            rail1,
            rail2,
            profile(0.),
            line([1., 5., 1.], [-1., 5., -1.]),
        ],
    );
    assert!(registry.execute(&mut mixed, "Sweep2").is_err());
    assert_eq!(mixed.objects().count(), 4);
    assert_eq!(mixed.undo_label(), None);
}

#[test]
fn sweep_two_rail_closed_profiles_can_close_along_both_rails() {
    let mut doc = Document::default();
    pick(
        &mut doc,
        [
            circle([0.; 3], 2., [1., 0., 0.], [0., 0., 1.]),
            circle([0.; 3], 4., [1., 0., 0.], [0., 0., 1.]),
            circle([3., 0., 0.], 1., [1., 0., 0.], [0., -1., 0.]),
            circle([0., 3., 0.], 1., [0., 1., 0.], [1., 0., 0.]),
        ],
    );
    CommandRegistry::with_builtins()
        .execute(&mut doc, "Sweep2 Natural Closed=Yes")
        .unwrap();
    let brep = last_brep(&doc);
    assert_eq!((brep.faces().len(), brep.edges().len()), (1, 2));
    let surface = brep.faces()[0].surface();
    assert_eq!([surface.degree_u(), surface.degree_v()], [3, 2]);
    assert!(surface.is_closed_u().unwrap() && surface.is_closed_v().unwrap());
    let analytic_area = 12. * std::f64::consts::PI.powi(2);
    assert!((brep.area(doc.tolerance()).unwrap() - analytic_area).abs() < 1e-5);
}

#[test]
fn sweep_two_rail_closed_profiles_accept_a_point_at_either_end() {
    let registry = CommandRegistry::with_builtins();
    for (option, [start, end], profile_y, tip_y, area) in [
        ("StartPoint=0,0,0", [0., 1.], 5., 0., 16.019042442097962),
        ("EndPoint=0,5,0", [1., 0.], 0., 5., 16.019042442097955),
    ] {
        let mut doc = Document::default();
        pick(
            &mut doc,
            [
                line([start, 0., 0.], [end, 5., 0.]),
                line([-start, 0., 0.], [-end, 5., 0.]),
                circle_at(profile_y, 1.),
            ],
        );
        registry
            .execute(&mut doc, &format!("Sweep2 {option}"))
            .unwrap();
        let brep = last_brep(&doc);
        assert_eq!((brep.faces().len(), brep.edges().len()), (1, 2));
        let surface = brep.faces()[0].surface();
        assert!(surface.is_closed_v().unwrap());
        assert!(
            brep.vertices()
                .iter()
                .any(|v| v.point() == p([0., tip_y, 0.]))
        );
        let actual = brep.area(doc.tolerance()).unwrap();
        assert!((actual - area).abs() < 1e-5, "{actual} != {area}");
    }
}

#[test]
fn sweep_two_rail_closed_profiles_rebuild_and_refit_as_closed_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    pick(
        &mut doc,
        [
            line([1., 0., 0.], [1., 5., 0.]),
            line([-1., 0., 0.], [-1., 5., 0.]),
            circle_at(0., 1.),
            circle_at(5., 1.),
        ],
    );
    for (options, degree) in [("RefitRails=Yes", 2), ("Rebuild=8", 3), ("Refit=0.01", 3)] {
        registry
            .execute(&mut doc, &format!("Sweep2 {options}"))
            .unwrap_or_else(|error| panic!("{options}: {error}"));
        let brep = last_brep(&doc);
        assert_eq!(
            (brep.faces().len(), brep.edges().len()),
            (1, 3),
            "{options}"
        );
        assert_eq!(brep.faces()[0].surface().degree_v(), degree, "{options}");
        assert!(brep.faces()[0].surface().is_closed_v().unwrap());
        doc.undo().unwrap();
    }
}
