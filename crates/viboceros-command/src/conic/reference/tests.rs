use super::*;
use viboceros_geometry::LineSegment;
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn guide() -> (Document, ReferenceInput) {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0.), p(10., 0.), doc.tolerance()).unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    let mut r = ReferenceInput::capture(&doc, id, None, true).unwrap();
    r.trim_point(p(6., 0.), doc.tolerance()).unwrap();
    (doc, r)
}
#[test]
fn locked_subcurve_point_direction_and_opposite_side_return_without_source_edits() {
    for (hover, end, expected) in [
        (8., 8., Some([6., 8.])),
        (2., 2., Some([2., 6.])),
        (8., 2., None),
        (8., 6., None),
    ] {
        let (doc, mut r) = guide();
        let before = format!("{doc:?}");
        assert!(r.direction(true).is_err());
        r.hover_point(p(hover, 0.)).unwrap();
        r.direction(true).unwrap();
        r.trim_point(p(end, 0.), doc.tolerance()).unwrap();
        if let Some([a, b]) = expected {
            assert_eq!(r.stage(), ReferenceStage::Endpoint);
            assert_eq!(r.curve().start_point().unwrap(), p(a, 0.));
            assert_eq!(r.curve().end_point().unwrap(), p(b, 0.));
        } else {
            assert_eq!(r.stage(), ReferenceStage::SelectCurve);
        }
        assert_eq!(format!("{doc:?}"), before);
        assert_eq!(doc.undo_label(), None);
    }
}
#[test]
fn locked_subcurve_numeric_finishes_immediately_clamps_and_rejects_invalid_length_atomically() {
    for (hover, ends) in [(8., [6., 9.]), (2., [3., 6.])] {
        let (doc, mut r) = guide();
        r.hover_point(p(hover, 0.)).unwrap();
        r.direction(true).unwrap();
        for length in [Real::NAN, Real::INFINITY, 11.] {
            let before = format!("{r:?}");
            assert!(r.length(length, doc.tolerance()).is_err());
            assert_eq!(format!("{r:?}"), before);
        }
        r.length(3., doc.tolerance()).unwrap();
        assert_eq!(r.stage(), ReferenceStage::Endpoint);
        assert_eq!(r.curve().start_point().unwrap(), p(ends[0], 0.));
        assert_eq!(r.curve().end_point().unwrap(), p(ends[1], 0.));
        assert_eq!(r.sample(p(100., 10.)).unwrap().0, p(ends[1], 0.));
        assert_eq!(doc.objects().count(), 1);
        assert_eq!(doc.undo_label(), None);
    }
}
#[test]
fn unlocking_releases_the_captured_side_and_full_curve_resets_temporary_input() {
    let (doc, mut r) = guide();
    r.hover_point(p(8., 0.)).unwrap();
    r.direction(true).unwrap();
    r.direction(false).unwrap();
    r.trim_point(p(2., 0.), doc.tolerance()).unwrap();
    assert_eq!(r.curve().start_point().unwrap(), p(2., 0.));
    assert_eq!(r.curve().end_point().unwrap(), p(6., 0.));
    r.full_curve();
    assert_eq!(r.curve().start_point().unwrap(), p(0., 0.));
    assert_eq!(r.curve().end_point().unwrap(), p(10., 0.));
    assert_eq!(r.hover_parameter(), None);
    assert_eq!(r.locked_forward(), None);
    assert_eq!(r.trim_start(), None);
}
#[test]
fn shared_direction_option_has_the_same_aliases_for_nested_and_standalone_getters() {
    use crate::subcurve_input::direction_option;
    for word in ["D", "_d", "Direction=Locked", "_Direction=_LOCKED"] {
        assert_eq!(direction_option(word), Some(Ok(true)));
    }
    assert_eq!(direction_option("Direction=Free"), Some(Ok(false)));
    assert_eq!(
        direction_option("Direction=Yes"),
        Some(Err("Direction expects Free or Locked"))
    );
    assert_eq!(direction_option("Curve=1"), None);
}
#[test]
fn zero_length_returns_to_the_parent_picker_and_does_not_replace_the_source() {
    let (doc, mut r) = guide();
    let before = format!("{doc:?}");
    r.length(0., doc.tolerance()).unwrap();
    assert_eq!(r.stage(), ReferenceStage::SelectCurve);
    assert_eq!(format!("{doc:?}"), before);
    assert_eq!(doc.undo_label(), None);
}
#[test]
fn closed_conic_numeric_portions_always_wait_for_confirmation_including_short_arcs() {
    use viboceros_geometry::Circle3;
    for length in [2., 8.] {
        let mut doc = Document::default();
        let plane = CommandContext::default().construction_plane;
        let id = doc
            .add_geometry(Geometry::Circle(
                Circle3::try_from_frame(
                    p(0., 0.),
                    4.,
                    plane.x_axis(),
                    plane.z_axis(),
                    doc.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        let mut r = ReferenceInput::capture(&doc, id, None, true).unwrap();
        r.trim_point(p(0., 4.), doc.tolerance()).unwrap();
        r.hover_point(p(-4., 0.)).unwrap();
        r.direction(true).unwrap();
        r.length(length, doc.tolerance()).unwrap();
        assert_eq!(r.stage(), ReferenceStage::TrimEnd);
        assert_eq!(r.locked_forward(), None);
        r.trim_point(p(-4., 0.), doc.tolerance()).unwrap();
        assert_eq!(r.stage(), ReferenceStage::Endpoint);
        assert!((r.curve().length(doc.tolerance()).unwrap() - length).abs() < 1e-8);
        let end = p(-4. * (length / 4.).sin(), 4. * (length / 4.).cos());
        assert!(r.curve().end_point().unwrap().distance_to(end).unwrap() < 1e-8);
        assert_eq!(doc.undo_label(), None);
    }
}
#[test]
fn surface_edge_numeric_length_preserves_the_edges_native_parameter_chart() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                p(0., 0.),
                p(10., 0.),
                p(10., 10.),
                p(0., 10.),
            ])
            .unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    let mut r = ReferenceInput::capture(&doc, id, Some(0), true).unwrap();
    r.trim_point(p(6., 0.), doc.tolerance()).expect("anchor");
    r.hover_point(p(8., 0.)).expect("hover");
    r.direction(true).expect("direction");
    r.length(3., doc.tolerance()).expect("numeric length");
    assert_eq!(r.stage(), ReferenceStage::Endpoint);
    assert!(
        r.sample(p(100., 0.))
            .unwrap()
            .0
            .distance_to(p(9., 0.))
            .unwrap()
            < 1e-10
    );
    assert_eq!(doc.undo_label(), None);
}
#[test]
fn closed_locked_backward_point_chooses_the_other_arc_and_preview_uses_the_same_stations() {
    use viboceros_geometry::Circle3;
    let mut doc = Document::default();
    let plane = CommandContext::default().construction_plane;
    let id = doc
        .add_geometry(Geometry::Circle(
            Circle3::try_from_frame(
                p(0., 0.),
                4.,
                plane.x_axis(),
                plane.z_axis(),
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let mut r = ReferenceInput::capture(&doc, id, None, true).unwrap();
    r.trim_point(p(0., -4.), doc.tolerance()).unwrap();
    r.hover_point(p(4., 0.)).unwrap();
    r.direction(true).unwrap();
    assert_eq!(r.locked_forward(), Some(false));
    r.hover_point(p(0., 4.)).unwrap();
    let [a, b] = r.preview_range().unwrap().unwrap();
    assert!(a < b);
    r.trim_point(p(0., 4.), doc.tolerance()).unwrap();
    let q = r
        .curve()
        .evaluate(
            r.curve()
                .domain()
                .start()
                .midpoint(*r.curve().domain().end()),
        )
        .unwrap();
    assert!(q.x() < -3.9);
    assert!(
        r.sample(p(6., 0.))
            .unwrap()
            .0
            .distance_to(p(0., 4.))
            .unwrap()
            < 1e-8
    );
}
