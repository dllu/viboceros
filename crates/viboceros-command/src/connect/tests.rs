use super::*;
use viboceros_geometry::{CircularArc3, CurveSegment3, LineSegment, NurbsCurve};

#[test]
fn interactive_polyline_getters_can_connect_the_other_end() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Polyline -4,-2,0 -4,0,0 -1,0,0")
        .unwrap();
    registry
        .execute(&mut document, "Polyline 0,2,0 0,3,0 3,3,0")
        .unwrap();
    let before: Vec<_> = document.objects().cloned().collect();
    let a = Source::capture(
        &document,
        before[0].id(),
        Some(Point3::try_new(-4., -1.8, 0.).unwrap()),
        None,
    )
    .unwrap();
    let b = Source::capture(
        &document,
        before[1].id(),
        Some(Point3::try_new(0., 2.2, 0.).unwrap()),
        None,
    )
    .unwrap();
    let result = prepare(&document, [&a, &b], Options::default()).unwrap();
    for (geometry, expected) in result.geometries().iter().zip([
        [[-4., -2., 0.], [-4., 0., 0.], [0., 0., 0.]],
        [[0., 0., 0.], [0., 3., 0.], [3., 3., 0.]],
    ]) {
        let Geometry::Polyline(polyline) = geometry else {
            panic!("expected retained polyline")
        };
        assert_eq!(polyline.vertices().len(), expected.len());
        for (actual, expected) in polyline.vertices().iter().zip(expected) {
            assert!(
                actual
                    .distance_to(Point3::try_from(expected).unwrap())
                    .unwrap()
                    < 1e-9
            );
        }
    }
    let ids = result.commit(&mut document).unwrap();
    assert_eq!(ids, vec![a.id(), b.id()]);
    assert_eq!(document.selected_object_count(), 0);
    document.undo().unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn connects_extended_lines_with_or_without_join_and_undo() {
    let registry = CommandRegistry::with_builtins();
    for (command, expected_count) in [("Connect", 2), ("Connect Join=Yes", 1)] {
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,4,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, command).unwrap();
        assert_eq!(document.objects().count(), expected_count);
        if expected_count == 2 {
            assert!(
                document
                    .objects()
                    .all(|object| matches!(object.geometry(), Geometry::Line(_)))
            );
        } else {
            assert!(matches!(
                document.objects().next().unwrap().geometry(),
                Geometry::PolyCurve(_)
            ));
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn already_meeting_arc_and_line_keep_native_geometry() {
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
    registry.execute(&mut document, "Connect").unwrap();
    assert_eq!(document.objects().count(), 2);
    assert!(
        document
            .objects()
            .any(|object| matches!(object.geometry(), Geometry::Arc(_)))
    );
    assert!(
        document
            .objects()
            .any(|object| matches!(object.geometry(), Geometry::Line(_)))
    );
    assert!(document.object(first).is_none() && document.object(second).is_none());
}

#[test]
fn connects_nurbs_endpoint_with_line_or_joined_smooth_extension() {
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
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Connect Join=Yes ExtendOtherCurvesBy=Line")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined NURBS connection");
    };
    assert!(matches!(
        joined.segments(),
        [
            CurveSegment3::NurbsCurve(_),
            CurveSegment3::Line(_),
            CurveSegment3::Line(_)
        ]
    ));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry
        .execute(&mut document, "Connect ExtendOtherCurvesBy=Smooth Join=Yes")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined smooth connection");
    };
    assert_eq!(joined.segments().len(), 2);
}

#[test]
fn smooth_option_extends_nurbs_shape_and_preserves_undo() {
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
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Connect ExtendOtherCurvesBy=Smooth")
        .unwrap();
    let curves = document.objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    let meeting = curves[0]
        .geometry()
        .curve_ref()
        .unwrap()
        .end_point()
        .unwrap();
    assert!(meeting.distance_to(p(3., 2.25)).unwrap() < 1e-10);
    assert!(
        curves[1]
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap()
            .distance_to(meeting)
            .unwrap()
            < 1e-10
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn smooth_option_connects_two_curved_nurbs_ends() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let first = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 0.), p(2., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let second = NurbsCurve::try_new(
        2,
        vec![p(3., 3.), p(3., 4.), p(3.2, 5.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let first_id = document.add_geometry(Geometry::NurbsCurve(first)).unwrap();
    let second_id = document.add_geometry(Geometry::NurbsCurve(second)).unwrap();
    document
        .select_objects_direct([first_id, second_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Connect ExtendOtherCurvesBy=Smooth")
        .unwrap();
    let curves = document.objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    let meeting = p(3.02533554003123, 2.28816378244402);
    assert!(
        curves[0]
            .geometry()
            .curve_ref()
            .unwrap()
            .end_point()
            .unwrap()
            .distance_to(meeting)
            .unwrap()
            < 1e-10
    );
    assert!(
        curves[1]
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap()
            .distance_to(meeting)
            .unwrap()
            < 1e-10
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn extend_arcs_by_line_uses_the_endpoint_tangent() {
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
    registry
        .execute(&mut document, "Connect Join=Yes ExtendArcsBy=Line")
        .unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined arc connection")
    };
    assert!(matches!(
        joined.segments(),
        [
            CurveSegment3::Arc(_),
            CurveSegment3::Line(_),
            CurveSegment3::Line(_)
        ]
    ));
}

#[test]
fn default_arc_extension_connects_two_native_arcs() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let diagonal = 2.0_f64.sqrt() / 2.0;
    let first = CircularArc3::try_from_three_points(
        p(1., 0.),
        p(diagonal, diagonal),
        p(0., 1.),
        document.tolerance(),
    )
    .unwrap();
    let second = CircularArc3::try_from_three_points(
        p(-3., 0.),
        p(-2. - diagonal, -diagonal),
        p(-2., -1.),
        document.tolerance(),
    )
    .unwrap();
    let first_id = document.add_geometry(Geometry::Arc(first)).unwrap();
    let second_id = document.add_geometry(Geometry::Arc(second)).unwrap();
    document
        .select_objects_direct([first_id, second_id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Connect Join=Yes").unwrap();
    let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
        panic!("joined arc connection")
    };
    assert!(matches!(
        joined.segments(),
        [CurveSegment3::Arc(_), CurveSegment3::Arc(_)]
    ));
}

#[test]
fn default_arc_style_trims_arc_to_line() {
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
    let line = LineSegment::try_new(p(0.5, 2.), p(0.5, 3.), document.tolerance()).unwrap();
    let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
    let second = document.add_geometry(Geometry::Line(line)).unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Connect").unwrap();
    let trimmed = document
        .objects()
        .find_map(|object| match object.geometry() {
            Geometry::Arc(arc) => Some(*arc),
            _ => None,
        })
        .expect("native trimmed arc");
    assert!((trimmed.sweep_radians() - std::f64::consts::PI / 3.0).abs() < 1e-12);
}
