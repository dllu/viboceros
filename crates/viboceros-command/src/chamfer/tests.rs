use super::*;
use viboceros_geometry::{CircularArc3, CurveSegment3, LineSegment, NurbsCurve};

#[test]
fn chamfer_options_and_undo() {
    let registry = CommandRegistry::with_builtins();
    for (command, count, retained) in [
        ("Chamfer 0.5 1", 3, 2),
        ("Chamfer Distances=0.5,1", 3, 2),
        ("Chamfer 0 0 Join=Yes", 1, 1),
        ("Chamfer 0.5 1 Join=No", 3, 2),
        ("Chamfer 0.5 1 Trim=No", 3, 2),
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
        registry.execute(&mut document, command).unwrap();
        assert_eq!(document.objects().count(), count, "{command}");
        assert_eq!(
            originals
                .iter()
                .filter(|id| document.object(**id).is_some())
                .count(),
            retained
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn chamfers_meeting_arc_and_line_without_rationalizing_the_arc() {
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
    let line = LineSegment::try_new(p(0., 1.), p(-2., 1.), document.tolerance()).unwrap();
    let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
    let second = document.add_geometry(Geometry::Line(line)).unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Chamfer 0.3 0.5 Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined chamfer");
    };
    assert!(matches!(
        joined.segments(),
        [
            CurveSegment3::Arc(_),
            CurveSegment3::Line(_),
            CurveSegment3::Line(_)
        ]
    ));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn extends_nonmeeting_arc_before_chamfer_and_undo() {
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
    let line = LineSegment::try_new(p(-1., 2.), p(-1., 3.), document.tolerance()).unwrap();
    let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
    let second = document.add_geometry(Geometry::Line(line)).unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Chamfer 0.2 0.3 ExtendArcsBy=Arc Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined chamfer")
    };
    let [
        CurveSegment3::Arc(retained),
        CurveSegment3::Line(_),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("arc extension, bevel, line")
    };
    assert!(
        (retained.length().unwrap() - (std::f64::consts::PI - 2. * 0.1_f64.asin())).abs() < 1e-9
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

    document.clear_selection();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Chamfer 1.2 0.3 ExtendArcsBy=Line Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined tangent-extension chamfer")
    };
    let [
        CurveSegment3::Arc(retained),
        CurveSegment3::Line(_),
        CurveSegment3::Line(_),
    ] = joined.segments()
    else {
        panic!("original arc, bevel, retained line")
    };
    assert!((retained.end().unwrap().distance_to(p(-1., 1.)).unwrap() - 1.2).abs() < 1e-9);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn smooth_option_extends_nonmeeting_nurbs_before_chamfer() {
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
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "Chamfer 0.2 0.3 ExtendOtherCurvesBy=Smooth Join=Yes",
        )
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined smooth NURBS chamfer")
    };
    assert!(matches!(
        joined.segments(),
        [
            CurveSegment3::NurbsCurve(_),
            CurveSegment3::Line(_),
            CurveSegment3::Line(_)
        ]
    ));
}
