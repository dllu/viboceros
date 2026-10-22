//! curves split command regressions.
use super::*;

#[test]
fn split_curve_preserves_attributes_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SplitCurves")
        .unwrap();
    registry
        .execute(
            &mut document,
            "ControlPointCurve 3 0,0,0 2,3,1 5,-2,-1 8,1,2",
        )
        .unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName SplitSource")
        .unwrap();
    registry
        .execute(&mut document, "Group SplitResults")
        .unwrap();
    let source = document.object(source_id).unwrap().clone();
    let curve = source
        .geometry()
        .nurbs_curve_representation()
        .unwrap()
        .unwrap();
    let parameter = curve.parameter_at(0.4).unwrap();
    let (expected_left, expected_right) = curve.try_split(parameter).unwrap();
    let check_piece = |actual: &Geometry, expected: &NurbsCurve| {
        let Geometry::NurbsCurve(actual) = actual else {
            panic!("split must preserve NURBS type")
        };
        assert_eq!(actual.degree(), expected.degree());
        assert_eq!(actual.knots(), expected.knots());
        assert_eq!(
            actual.control_points().len(),
            expected.control_points().len()
        );
        for (a, e) in actual
            .control_points()
            .iter()
            .zip(expected.control_points())
        {
            assert_eq!(a.weight(), e.weight());
            assert!(a.point().distance_to(e.point()).unwrap() < 1e-12);
        }
        for i in 0..=64 {
            let t = actual.parameter_at(i as Real / 64.).unwrap();
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    };

    assert_eq!(
        registry
            .execute(&mut document, &format!("Split Parameter={parameter}"))
            .unwrap(),
        "Split the selected curve at 1 location(s) into 2 exact curve piece(s)"
    );
    assert_eq!(document.objects().count(), 2);
    let left = document.object(source_id).unwrap();
    check_piece(left.geometry(), &expected_left);
    assert_eq!(left.attributes(), source.attributes());
    let right = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap();
    let right_id = right.id();
    check_piece(right.geometry(), &expected_right);
    assert_eq!(right.attributes(), source.attributes());
    assert!(document.is_selected(source_id));
    assert!(document.is_selected(right_id));
    assert_eq!(
        document
            .group_by_name("SplitResults")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source_id, right_id])
    );
    assert_eq!(document.undo_label(), Some("Split"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.object(source_id).unwrap(), &source);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().count(), 2);
}

#[test]
fn split_curve_accepts_a_model_point_and_preserves_analytic_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 5").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "Split 0,5,0 -5,0,0")
        .unwrap();
    assert_eq!(document.objects().count(), 2);
    for object in document.objects() {
        assert!(matches!(
            object.geometry(),
            Geometry::Arc(_) | Geometry::PolyCurve(_)
        ));
        let curve = object.geometry().curve_ref().unwrap();
        let start = curve.evaluate(*curve.domain().start()).unwrap();
        let end = curve.evaluate(*curve.domain().end()).unwrap();
        let top = Point3::try_new(0.0, 5.0, 0.0).unwrap();
        let left = Point3::try_new(-5.0, 0.0, 0.0).unwrap();
        assert!(
            (start.is_near(top, document.tolerance()) && end.is_near(left, document.tolerance()))
                || (start.is_near(left, document.tolerance())
                    && end.is_near(top, document.tolerance()))
        );
    }

    let mut single = Document::default();
    registry.execute(&mut single, "Circle 0,0 5").unwrap();
    let id = single.objects().next().unwrap().id();
    single.select_object(id, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry.execute(&mut single, "Split Parameter=1").unwrap(),
        "Split the selected curve at 1 location(s) into 1 exact curve piece(s)"
    );
    assert_eq!(single.objects().count(), 1);
    assert!(single.object(id).is_some());
    let Geometry::PolyCurve(curve) = single.object(id).unwrap().geometry() else {
        panic!("a singly split closed curve must retain both seam pieces")
    };
    assert!(curve.is_closed().unwrap());
    assert_eq!(curve.segments().len(), 2);
    assert_eq!(*curve.domain().start(), 1.0);
}

#[test]
fn split_curve_accepts_multiple_unsorted_parameters() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split Parameter=8,2,5")
            .unwrap(),
        "Split the selected curve at 3 location(s) into 4 exact curve piece(s)"
    );
    let domains = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            _ => panic!("Split must retain native line pieces"),
        })
        .collect::<Vec<_>>();
    assert_eq!(domains, vec![0.0..=2.0, 2.0..=5.0, 5.0..=8.0, 8.0..=10.0]);
    assert_eq!(document.selected_object_count(), 4);
}

#[test]
fn split_curve_uses_selected_curve_cutters_and_replaces_source_with_grouped_pieces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New CuttingSplit")
        .unwrap();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName SplitSource")
        .unwrap();
    registry
        .execute(&mut document, "Group SplitPieces")
        .unwrap();
    let source = document.object(source_id).unwrap().clone();
    registry.execute(&mut document, "Line 3,-5 3,5").unwrap();
    registry.execute(&mut document, "Line 7,-5 7,5").unwrap();
    let cutter_ids = document
        .objects()
        .filter(|object| object.id() != source_id)
        .map(|object| object.id())
        .collect::<BTreeSet<_>>();
    let cutters = cutter_ids
        .iter()
        .map(|id| (*id, document.object(*id).unwrap().clone()))
        .collect::<Vec<_>>();
    document
        .select_objects_direct(
            std::iter::once(source_id).chain(cutter_ids.iter().copied()),
            SelectionMode::Replace,
        )
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=5,0,0")
            .unwrap(),
        "Split the selected curve at 2 location(s) into 3 exact curve piece(s)"
    );
    for (id, cutter) in cutters {
        assert_eq!(document.object(id).unwrap(), &cutter);
        assert!(!document.is_selected(id));
    }
    let outputs = document
        .objects()
        .filter(|object| !cutter_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 3);
    assert!(document.object(source_id).is_none());
    let domains = outputs
        .iter()
        .map(|object| {
            assert_eq!(object.attributes(), source.attributes());
            assert!(document.is_selected(object.id()));
            let Geometry::Line(curve) = object.geometry() else {
                panic!("cutting-object Split must retain native lines")
            };
            curve.domain()
        })
        .collect::<Vec<_>>();
    assert_eq!(domains, vec![0.0..=3.0, 3.0..=7.0, 7.0..=10.0]);
    assert_eq!(
        document
            .group_by_name("SplitPieces")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        outputs.iter().map(|object| object.id()).collect()
    );
}

#[test]
fn split_curve_uses_exact_surface_and_trimmed_brep_cutters() {
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
    let cutter_id = surface_document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    surface_document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut surface_document, "Split CuttingObjects 1,0,0")
        .unwrap();
    let surface_domains = surface_document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("surface Split selected unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(surface_domains, vec![0.0..=3.0, 3.0..=7.0, 7.0..=10.0]);
    assert!(surface_document.object(cutter_id).is_some());

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
    let cutter = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
    let mut brep_document = Document::default();
    registry
        .execute(&mut brep_document, "Line 0,0,0 10,0,0")
        .unwrap();
    let source_id = brep_document.objects().next().unwrap().id();
    let cutter_id = brep_document.add_geometry(Geometry::Brep(cutter)).unwrap();
    brep_document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut brep_document, "Split CuttingObjects=1,0,0")
            .unwrap(),
        "Split the selected curve at 4 location(s) into 5 exact curve piece(s)"
    );
    let brep_domains = brep_document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("trimmed B-rep Split selected unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    for (actual, expected) in
        brep_domains
            .iter()
            .zip([[0.0, 2.0], [2.0, 4.0], [4.0, 6.0], [6.0, 8.0], [8.0, 10.0]])
    {
        assert!((*actual.start() - expected[0]).abs() < 1.0e-12);
        assert!((*actual.end() - expected[1]).abs() < 1.0e-12);
    }
    assert!(brep_document.object(cutter_id).is_some());
}

#[test]
fn split_curve_rejects_invalid_or_ambiguous_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Split",
        "Split Parameter",
        "Split Parameter=0.2,,0.8",
        "Split Parameter=0.5 extra",
        "Split 1,2,3 extra",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(SPLIT_CURVE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "Split Parameter=0.5"),
        Err(CommandError::SplitRequiresOneCurve { actual: 0 })
    ));

    registry.execute(&mut document, "Line 0,0 2,0").unwrap();
    registry.execute(&mut document, "Line 0,1 2,1").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Split Parameter=0.5"),
        Err(CommandError::SplitRequiresOneCurve { actual: 2 })
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let first = before[0].id();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Split Parameter=0"),
        Err(CommandError::Geometry(
            GeometryError::InvalidCurveSplitParameter
        ))
    ));
    assert_eq!(document.object(first).unwrap(), &before[0]);
    assert_eq!(document.undo_label(), history.as_deref());
}
