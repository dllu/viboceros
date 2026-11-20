use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn add_curve(doc: &mut Document, points: Vec<Point3>) -> ObjectId {
    let count = points.len();
    let knots = std::iter::repeat_n(0., count)
        .chain(std::iter::repeat_n(1., count))
        .collect();
    let curve = NurbsCurve::try_new(count - 1, points, knots).unwrap();
    doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap()
}

#[test]
fn sweep_two_rail_point_ends_match_rhino_singular_arch_fixtures() {
    for (point, z, argument, crossing) in [
        (p(0., 0., 0.), 5., "StartPoint=0,0,0", false),
        (p(0., 0., 0.), 5., "StartPoint=0,0,0", true),
        (p(0., 0., 5.), 0., "EndPoint=0,0,5", false),
    ] {
        let mut doc = Document::default();
        let left = p(-2., 0., z);
        let right = p(2., 0., z);
        let ridge = p(0., 0.3, z + 0.7);
        let other = if crossing {
            [p(2., 0., -5.), p(-2., 0., -5.)]
        } else {
            [point; 2]
        };
        let ids = [
            add_curve(&mut doc, vec![other[0], left]),
            add_curve(&mut doc, vec![right, other[1]]),
            add_curve(&mut doc, vec![left, ridge, right]),
        ];
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, &format!("Sweep2 {argument}"))
            .unwrap();
        let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
            panic!("Expected a sweep with a pole");
        };
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (1, 3, 3)
        );
        assert!(brep.vertices().iter().any(|vertex| vertex.point() == point));
        let s = brep.faces()[0].surface();
        assert_eq!([s.degree_u(), s.degree_v()], [3, 2]);
        assert_eq!(
            [s.control_point_count_u(), s.control_point_count_v()],
            [4, 3]
        );
        // Rhino 8.32 returned a single clamped cubic from the arch to the
        // point. Matching controls up to U reversal bounds the whole image.
        let reverse = argument.starts_with("StartPoint");
        for u in 0..4 {
            let blend = if reverse { u } else { 3 - u } as Real / 3.;
            for (v, profile) in [left, ridge, right].into_iter().enumerate() {
                let expected = point
                    .translated(point.vector_to(profile).unwrap().scaled(blend).unwrap())
                    .unwrap();
                let control = s.control_point(u, v).unwrap();
                assert_eq!(control.weight(), 1.);
                assert!(expected.distance_to(control.point()).unwrap() < 1e-9);
            }
        }
        assert_eq!(doc.undo_label(), Some("Sweep2"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn sweep_two_rail_two_point_ends_certify_original_rails_and_reject_off_rail_points() {
    let mut doc = Document::default();
    let start = p(0.1, 0., 0.);
    let end = p(0.2, 0., 5.);
    let ids = [
        add_curve(&mut doc, vec![start, p(-2., 0., 2.5), end]),
        add_curve(&mut doc, vec![end, p(2., 0., 2.5), start]),
        add_curve(
            &mut doc,
            vec![p(-0.925, 0., 2.5), p(0.075, 0.3, 3.2), p(1.075, 0., 2.5)],
        ),
    ];
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    for input in [
        "Sweep2 StartPoint=0,0,0 EndPoint=0.2,0,5",
        "Sweep2 StartPoint=0.1,0,0 EndPoint=0.1,0,0",
        "Sweep2 StartPoint=0.1,0,0 StartPoint=0.1,0,0",
        "Sweep2 StartPoint=NaN,0,0 EndPoint=0.2,0,5",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), None);
    }
    registry
        .execute(&mut doc, "Sweep2 StartPoint=0.1,0,0 EndPoint=0.2,0,5")
        .unwrap();
    let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
        panic!("Expected a sweep with two poles");
    };
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (1, 2, 2)
    );
    for point in [start, end] {
        assert!(brep.vertices().iter().any(|vertex| vertex.point() == point));
    }
}

#[test]
fn sweep_two_rail_point_and_interior_profile_trim_both_boundaries() {
    let pole = p(0., 0., 0.);
    let profile = [p(-1.25, 0., 5.), p(0., 0.5, 5.5), p(1.25, 0., 5.)];
    let registry = CommandRegistry::with_builtins();
    for crossing in [false, true] {
        let mut doc = Document::default();
        let [a, b] = if crossing {
            [p(2., 0., -8.), p(-2., 0., -8.)]
        } else {
            [pole; 2]
        };
        let ids = [
            add_curve(&mut doc, vec![a, p(-2., 0., 8.)]),
            add_curve(&mut doc, vec![p(2., 0., 8.), b]),
            add_curve(&mut doc, profile.to_vec()),
        ];
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, "Sweep2 StartPoint=0,0,0")
            .unwrap();
        let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
            panic!("Expected a Sweep2 B-rep");
        };
        assert_eq!(brep.faces().len(), 1);
        assert_eq!(brep.edges().len(), 3);
        // Rhino 8.32 produced the same cubic for a terminal pole and a pole
        // at an interior crossing. Test its complete control net.
        let surface = brep.faces()[0].surface();
        assert_eq!(surface.control_point_count_u(), 4);
        for u in 0..4 {
            for (v, end) in profile.into_iter().enumerate() {
                let expected = pole
                    .translated(pole.vector_to(end).unwrap().scaled(u as f64 / 3.).unwrap())
                    .unwrap();
                let actual = surface.control_point(u, v).unwrap().point();
                assert!(actual.distance_to(expected).unwrap() < 1e-9);
            }
        }
        let area = brep.area(doc.tolerance()).unwrap();
        assert!((area - 6.831194353237493).abs() < 1e-5);
        assert_eq!(doc.undo_label(), Some("Sweep2"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}
