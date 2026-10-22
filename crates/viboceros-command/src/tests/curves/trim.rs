//! curves trim command regressions.
use super::*;

#[test]
fn trim_removes_a_middle_interval_and_replaces_the_source_with_grouped_pieces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName TrimSource")
        .unwrap();
    registry.execute(&mut document, "Group Trimmed").unwrap();
    let source = document.object(source_id).unwrap().clone();
    registry.execute(&mut document, "Line 3,-5 3,5").unwrap();
    registry.execute(&mut document, "Line 7,-5 7,5").unwrap();
    let cutter_ids = document
        .objects()
        .filter(|object| object.id() != source_id)
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    document
        .select_objects_direct(
            std::iter::once(source_id).chain(cutter_ids.iter().copied()),
            SelectionMode::Replace,
        )
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Trim 5,0").unwrap(),
        "Trimmed one curve interval and retained 2 exact curve piece(s)"
    );
    assert!(document.object(source_id).is_none());
    let outputs = document
        .objects()
        .filter(|object| !cutter_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut domains = outputs
        .iter()
        .map(|object| {
            assert_eq!(object.attributes(), source.attributes());
            assert!(document.is_selected(object.id()));
            let Geometry::Line(curve) = object.geometry() else {
                panic!("Trim must create native line pieces")
            };
            curve.domain()
        })
        .collect::<Vec<_>>();
    domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    assert_eq!(domains, vec![0.0..=3.0, 7.0..=10.0]);
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(
        document
            .group_by_name("Trimmed")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        outputs.iter().map(|object| object.id()).collect()
    );
    assert_eq!(document.undo_label(), Some("Trim"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 3);
    assert_eq!(document.object(source_id).unwrap(), &source);
    assert_eq!(
        document
            .group_by_name("Trimmed")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source_id])
    );
}

#[test]
fn trim_of_an_end_retains_identity_and_ignores_other_cuts_in_the_kept_interval() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    registry.execute(&mut document, "Line 3,-5 3,5").unwrap();
    registry.execute(&mut document, "Line 7,-5 7,5").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Trim 1,0").unwrap(),
        "Trimmed one curve interval and retained 1 exact curve piece(s)"
    );
    let Geometry::Line(curve) = document.object(source_id).unwrap().geometry() else {
        panic!("Trim must convert its result to native line geometry")
    };
    assert_eq!(curve.domain(), 3.0..=10.0);
    assert_eq!(document.objects().count(), 3);
    assert_eq!(document.selected_object_count(), 1);
    assert!(document.is_selected(source_id));
}

#[test]
fn trim_removes_the_picked_arc_of_a_closed_curve() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 5").unwrap();
    let source_id = document.objects().next().unwrap().id();
    registry.execute(&mut document, "Line 0,-8 0,8").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();

    registry.execute(&mut document, "Trim 5,0").unwrap();
    let Geometry::Arc(curve) = document.object(source_id).unwrap().geometry() else {
        panic!("closed-curve Trim must create native arc geometry")
    };
    assert!(curve.evaluate(*curve.domain().start()).unwrap().is_near(
        Point3::try_new(0.0, 5.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert!(curve.evaluate(*curve.domain().end()).unwrap().is_near(
        Point3::try_new(0.0, -5.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert!(!curve.is_closed());
    assert_eq!(document.selected_object_count(), 1);
}

#[test]
fn trim_without_a_bounded_interval_is_a_non_destructive_success() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    registry.execute(&mut document, "Line 20,-5 20,5").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let source_id = ids[0];
    let before = document.object(source_id).unwrap().clone();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Trim 5,0").unwrap(),
        "No bounded curve interval was available to trim"
    );
    assert_eq!(document.object(source_id).unwrap(), &before);
    assert_eq!(document.selected_object_count(), 1);
    assert!(document.is_selected(source_id));
}

#[test]
fn trim_uses_view_projected_intersections_by_default_and_can_disable_them() {
    let registry = CommandRegistry::with_builtins();
    let trim_document = || {
        let mut document = Document::default();
        registry
            .execute(&mut document, "Line 0,0,0 10,0,0")
            .unwrap();
        registry
            .execute(&mut document, "Line 5,-5,1 5,5,1")
            .unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        document
    };

    let mut apparent = trim_document();
    let source_id = apparent.objects().next().unwrap().id();
    registry.execute(&mut apparent, "Trim 2,0,0").unwrap();
    let Geometry::Line(curve) = apparent.object(source_id).unwrap().geometry() else {
        panic!("apparent Trim must create native line geometry")
    };
    assert_eq!(curve.domain(), 5.0..=10.0);

    let mut actual = trim_document();
    let source_id = actual.objects().next().unwrap().id();
    let before = actual.object(source_id).unwrap().clone();
    assert_eq!(
        registry
            .execute(&mut actual, "Trim 2,0,0 ApparentIntersections=No")
            .unwrap(),
        "No bounded curve interval was available to trim"
    );
    assert_eq!(actual.object(source_id).unwrap(), &before);
}

#[test]
fn trim_uses_exact_surface_and_brep_cutters() {
    let registry = CommandRegistry::with_builtins();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();

    let mut surface_document = Document::default();
    registry
        .execute(&mut surface_document, "Line 0,0,0 10,0,0")
        .unwrap();
    let source_id = surface_document.objects().next().unwrap().id();
    let surface = NurbsSurface::try_bilinear([
        point(3.0, -1.0, 0.0),
        point(7.0, -1.0, 0.0),
        point(7.0, 1.0, 0.0),
        point(3.0, 1.0, 0.0),
    ])
    .unwrap();
    let surface_id = surface_document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    surface_document
        .select_objects_direct([source_id, surface_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut surface_document, "Trim 5,0,0 ApparentIntersections=No",)
            .unwrap(),
        "Trimmed one curve interval and retained 2 exact curve piece(s)"
    );
    let mut surface_domains = surface_document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("surface Trim selected unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    surface_domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    assert_eq!(surface_domains, vec![0.0..=3.0, 7.0..=10.0]);
    assert!(surface_document.object(surface_id).is_some());

    let mut brep_document = Document::default();
    registry
        .execute(&mut brep_document, "Line 0,0,0 10,0,0")
        .unwrap();
    let source_id = brep_document.objects().next().unwrap().id();
    let cutter_id = brep_document
        .add_geometry(Geometry::Brep(intersection_box_with_intervals([
            [2.0, 8.0],
            [-1.0, 1.0],
            [-1.0, 1.0],
        ])))
        .unwrap();
    brep_document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut brep_document, "Trim 5,0,0").unwrap(),
        "Trimmed one curve interval and retained 2 exact curve piece(s)"
    );
    let mut brep_domains = brep_document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("B-rep Trim selected unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    brep_domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    for (actual, expected) in brep_domains.iter().zip([[0.0, 2.0], [8.0, 10.0]]) {
        assert!((*actual.start() - expected[0]).abs() < 1.0e-12);
        assert!((*actual.end() - expected[1]).abs() < 1.0e-12);
    }
    assert!(brep_document.object(cutter_id).is_some());

    let closed_polyline = |points| {
        Polyline3::try_new(points, Tolerance::DEFAULT)
            .and_then(|polyline| polyline.to_nurbs())
            .unwrap()
    };
    let outer = closed_polyline(vec![
        point(2.0, -2.0, 0.0),
        point(8.0, -2.0, 0.0),
        point(8.0, 2.0, 0.0),
        point(2.0, 2.0, 0.0),
        point(2.0, -2.0, 0.0),
    ]);
    let hole = closed_polyline(vec![
        point(4.0, -1.0, 0.0),
        point(6.0, -1.0, 0.0),
        point(6.0, 1.0, 0.0),
        point(4.0, 1.0, 0.0),
        point(4.0, -1.0, 0.0),
    ]);
    let trimmed_face =
        Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
    let mut trimmed_document = Document::default();
    registry
        .execute(&mut trimmed_document, "Line 0,0,0 10,0,0")
        .unwrap();
    let source_id = trimmed_document.objects().next().unwrap().id();
    let cutter_id = trimmed_document
        .add_geometry(Geometry::Brep(trimmed_face))
        .unwrap();
    trimmed_document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut trimmed_document, "Trim 5,0,0")
        .unwrap();
    let mut trimmed_domains = trimmed_document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("trimmed-face Trim selected unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    trimmed_domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    for (actual, expected) in trimmed_domains.iter().zip([[0.0, 4.0], [6.0, 10.0]]) {
        assert!((*actual.start() - expected[0]).abs() < 1.0e-12);
        assert!((*actual.end() - expected[1]).abs() < 1.0e-12);
    }
    assert!(trimmed_document.object(cutter_id).is_some());
}

#[test]
fn trim_applies_apparent_intersections_to_surface_and_brep_cutters() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Line 0,0,1 10,0,1")
        .unwrap();
    let source_id = document.objects().next().unwrap().id();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let surface = NurbsSurface::try_bilinear([
        point(3.0, -1.0, 0.0),
        point(7.0, -1.0, 0.0),
        point(7.0, 1.0, 0.0),
        point(3.0, 1.0, 0.0),
    ])
    .unwrap();
    let cutter_id = document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Trim 5,0,1").unwrap(),
        "Trimmed one curve interval and retained 2 exact curve piece(s)"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let mut domains = document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("projected surface Trim selected unexpected {geometry:?}"),
        })
        .collect::<Vec<_>>();
    domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    for (actual, expected) in domains.iter().zip([[0.0, 3.0], [7.0, 10.0]]) {
        assert!((*actual.start() - expected[0]).abs() < 1.0e-12);
        assert!((*actual.end() - expected[1]).abs() < 1.0e-12);
    }

    let mut brep_document = Document::default();
    registry
        .execute(&mut brep_document, "Line 0,0,2 10,0,2")
        .unwrap();
    let source_id = brep_document.objects().next().unwrap().id();
    let cutter_id = brep_document
        .add_geometry(Geometry::Brep(intersection_box_with_intervals([
            [2.0, 8.0],
            [-1.0, 1.0],
            [0.0, 1.0],
        ])))
        .unwrap();
    brep_document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    registry.execute(&mut brep_document, "Trim 5,0,2").unwrap();
    let mut domains = brep_document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("projected B-rep Trim selected unexpected {geometry:?}"),
        })
        .collect::<Vec<_>>();
    domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    for (actual, expected) in domains.iter().zip([[0.0, 2.0], [8.0, 10.0]]) {
        assert!((*actual.start() - expected[0]).abs() < 1.0e-12);
        assert!((*actual.end() - expected[1]).abs() < 1.0e-12);
    }
}

#[test]
fn trim_rejects_invalid_or_underspecified_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Trim",
        "Trim 1,2 extra",
        "Trim 1,2 ApparentIntersections=Maybe",
        "Trim 1,2 ViewNormal=0,0,1 ViewNormal=0,1,0",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(TRIM_CURVE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "Trim 0,0"),
        Err(CommandError::TrimRequiresSourceAndCuttingObjects { actual: 0 })
    ));
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    let before = document.object(source_id).unwrap().clone();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Trim 5,0"),
        Err(CommandError::TrimRequiresSourceAndCuttingObjects { actual: 1 })
    ));
    assert_eq!(document.object(source_id).unwrap(), &before);
    assert_eq!(document.undo_label(), history.as_deref());
}
