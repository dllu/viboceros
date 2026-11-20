use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn sources(doc: &mut Document, profiles: &[(Real, Real)]) -> Vec<ObjectId> {
    let mut ids = Vec::new();
    for y in [0., 2.] {
        let line = LineSegment::try_new(p(0., y, 0.), p(8., y, 0.), doc.tolerance()).unwrap();
        ids.push(doc.add_geometry(Geometry::Line(line)).unwrap());
    }
    for &(x, height) in profiles {
        let arch = NurbsCurve::try_new(
            2,
            vec![p(x, 0., 0.), p(x, 1., height), p(x, 2., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        ids.push(doc.add_geometry(Geometry::NurbsCurve(arch)).unwrap());
    }
    ids
}

#[test]
fn sweep_two_rail_slash_matches_native_single_and_multi_profile_sweeps() {
    let registry = CommandRegistry::with_builtins();
    // Rhino 8.32 Add Slash, picked at (2,0,0) and (5,2,0).
    for (profiles, area, heights) in [
        (
            &[(0., 0.8)][..],
            18.759575455748848,
            [
                0.535351296530553,
                0.721110255092798,
                0.705682912474685,
                0.4758936389601242,
            ],
        ),
        (
            &[(0., 0.8), (8., 1.2)][..],
            20.048064321660178,
            [
                0.5793446895334075,
                0.8677618279668948,
                0.8761978455281562,
                0.6782097451334327,
            ],
        ),
    ] {
        let mut doc = Document::default();
        let ids = sources(&mut doc, profiles);
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, "Sweep2 RefitRails=No Slash=2,0,0/5,2,0")
            .unwrap();
        let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
            panic!("Expected a guided Sweep2 B-rep");
        };
        assert_eq!(brep.faces().len(), 1);
        let surface = brep.faces()[0].surface();
        assert_eq!(
            surface.knots_u(),
            &[0., 0., 0., 0., 1.75, 3.5, 5.25, 8., 8., 8., 8.]
        );
        for (t, height) in [2., 3.5, 4., 6.].into_iter().zip(heights) {
            let actual = surface.evaluate(t, 0.5).unwrap();
            assert!(
                actual.distance_to(p(t, 1., height)).unwrap() < 1e-8,
                "{actual:?}"
            );
        }
        for (v, expected) in [(0., p(2., 0., 0.)), (1., p(5., 2., 0.))] {
            assert!(
                surface
                    .evaluate(3.5, v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-9
            );
        }
        let actual = brep.area(doc.tolerance()).unwrap();
        assert!((actual - area).abs() < 1e-5, "{actual} != {area}");
        assert_eq!(doc.undo_label(), Some("Sweep2"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn sweep_two_rail_slash_validates_each_pair_before_a_document_edit() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let ids = sources(&mut doc, &[(0., 0.8), (4., 1.), (8., 1.2)]);
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for options in [
        "Slash=2,0,0",
        "Slash=2,0,0/5,0,0",
        "Slash=0,0,0/2,2,0",
        "Slash=2,0,0/5,2,0",
        "Slash=2,0,0/3,2,0 Slash=2,0,0/3,2,0",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("Sweep2 {options}"))
                .is_err(),
            "{options}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), None);
    }
    registry
        .execute(&mut doc, "Sweep2 Slash=2,0,0/3,2,0 Slash=5,0,0/6,2,0")
        .unwrap();
    assert_eq!(doc.objects().count(), before.len() + 1);
}
