use super::*;

#[test]
fn explicit_weight_edits_validate_every_pick_and_value_before_touching_history() {
    let mut doc = Document::default();
    let p = |x| Point3::try_new(x, 0., 0.).unwrap();
    let id = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, vec![p(0.), p(1.), p(2.)]).unwrap(),
        ))
        .unwrap();
    let marker = doc.add_geometry(Geometry::Point(p(8.))).unwrap();
    doc.undo().unwrap();
    doc.enable_control_points([id]).unwrap();
    let first = ControlPointId {
        object: id,
        index: 0,
    };
    let hidden = ControlPointId {
        object: id,
        index: 1,
    };
    doc.select_control_points([first], SelectionMode::Add)
        .unwrap();
    doc.set_control_points_hidden([hidden], true).unwrap();
    let original = format!("{doc:?}");
    for weight in [0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(doc.set_control_point_weights([first], weight).is_err());
        assert_eq!(format!("{doc:?}"), original);
    }
    for invalid in [
        hidden,
        ControlPointId {
            object: id,
            index: 3,
        },
        ControlPointId {
            object: marker,
            index: 0,
        },
    ] {
        assert!(doc.set_control_point_weights([first, invalid], 2.).is_err());
        assert_eq!(format!("{doc:?}"), original);
    }
    assert_eq!(doc.set_control_point_weights([first], 1.).unwrap(), 0);
    assert_eq!(format!("{doc:?}"), original);
    assert!(doc.can_redo());
    assert_eq!(
        doc.set_control_point_weights([first, first], 2.).unwrap(),
        1
    );
    assert_eq!(doc.undo_label(), Some("Weight"));
    assert!(!doc.can_redo());
    assert_eq!(doc.control_point_weight(first), Some(2.));
    doc.undo().unwrap();
    assert_eq!(doc.control_point_weight(first), Some(1.));
    assert_eq!(doc.is_control_point_hidden(hidden), Some(true));
    doc.redo().unwrap();
    assert_eq!(doc.control_point_weight(first), Some(2.));
    assert_eq!(doc.is_control_point_hidden(hidden), Some(false));
}
