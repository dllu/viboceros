use super::*;
use viboceros_geometry::{CircularArc3, CurveSegment3, LineSegment, NurbsCurve};

#[test]
fn joins_selected_lines_and_restores_sources_with_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0,0 4,0,0").unwrap();
    registry.execute(&mut document, "Line 4,0,0 4,4,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Fillet 0.5 Join=Yes")
        .unwrap();
    assert_eq!(document.objects().count(), 1);
    assert!(
        matches!(
            document.objects().next().unwrap().geometry(),
            Geometry::PolyCurve(_)
        ),
        "segments: {:?}",
        document.objects().next().unwrap().geometry()
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn join_and_trim_options_control_sources_and_arc() {
    let registry = CommandRegistry::with_builtins();
    for (command, expected_count, originals_retained, arc_count) in [
        ("Fillet 0.5 Join=No Trim=Yes", 3, true, 1),
        ("Fillet 0.5 Join=No Trim=No", 3, true, 1),
        ("Fillet 0.5 Join=Yes Trim=No", 3, true, 1),
        ("Fillet 0 Join=No Trim=Yes", 2, true, 0),
        ("Fillet 0 Join=Yes Trim=Yes", 1, false, 0),
    ] {
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 4,0,0").unwrap();
        registry.execute(&mut document, "Line 4,0,0 4,4,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let originals = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let before = document.objects().cloned().collect::<Vec<_>>();
        document.clear_history().unwrap();
        registry.execute(&mut document, command).unwrap();
        assert_eq!(document.objects().count(), expected_count, "{command}");
        assert_eq!(
            originals.iter().all(|id| document.object(*id).is_some()),
            originals_retained,
            "{command}"
        );
        assert_eq!(
            document
                .objects()
                .filter(|object| matches!(object.geometry(), Geometry::Arc(_)))
                .count(),
            arc_count,
            "{command}"
        );
        assert!(document.can_undo(), "{command}");
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn nonmeeting_arc_extends_to_fillet_and_undo_restores_sources() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let diagonal = 2.0_f64.sqrt() / 2.0;
    let arc = CircularArc3::try_from_three_points(
        p(1., 0.),
        p(diagonal, diagonal),
        p(0., 1.),
        document.tolerance(),
    )
    .unwrap();
    let line = LineSegment::try_new(p(-0.5, 2.), p(-0.5, 3.), document.tolerance()).unwrap();
    let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
    let second = document.add_geometry(Geometry::Line(line)).unwrap();
    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    assert_eq!(document.selected_objects().next().unwrap().id(), first);
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Fillet 0.2 ExtendArcsBy=Arc Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined fillet")
    };
    assert!(matches!(
        joined.segments(),
        [
            CurveSegment3::Arc(_),
            CurveSegment3::Arc(_),
            CurveSegment3::Line(_)
        ]
    ));
    assert!((joined.length(document.tolerance()).unwrap() - 4.026276894462145).abs() < 1e-7);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "Fillet 0.2 ExtendArcsBy=Line Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined tangent-extension fillet")
    };
    assert!(
        matches!(
            joined.segments(),
            [
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_),
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_)
            ]
        ),
        "segments: {:?}",
        joined.segments()
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "Fillet 0.7 ExtendArcsBy=Line Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined fillet after consuming tangent extension")
    };
    let [
        CurveSegment3::Arc(retained),
        CurveSegment3::Arc(_),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("original arc, fillet, retained line")
    };
    assert!(retained.length().unwrap() < std::f64::consts::FRAC_PI_2);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
}

#[test]
fn nurbs_extension_options_create_fillets() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let nurbs = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 0.), p(2., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let line = LineSegment::try_new(p(3., 3.), p(3., 4.), document.tolerance()).unwrap();
    let first = document.add_geometry(Geometry::NurbsCurve(nurbs)).unwrap();
    let second = document.add_geometry(Geometry::Line(line)).unwrap();
    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    assert_eq!(document.selected_objects().next().unwrap().id(), first);
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Fillet 0.2 Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined tangent-extension fillet")
    };
    assert!(
        matches!(
            joined.segments(),
            [
                CurveSegment3::NurbsCurve(_),
                CurveSegment3::Line(_),
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_)
            ]
        ),
        "segments: {:?}",
        joined.segments()
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "Fillet 0.2 ExtendOtherCurvesBy=Smooth Join=Yes",
        )
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined smooth-extension fillet")
    };
    assert!(
        matches!(
            joined.segments(),
            [
                CurveSegment3::NurbsCurve(_),
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_)
            ]
        ),
        "segments: {:?}",
        joined.segments()
    );
}
