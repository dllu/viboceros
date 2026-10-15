use super::*;
use viboceros_geometry::{ControlPointCurveClosure, Vector3, WeightedPoint3};

fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn curve() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [p(0., 0.), p(1., 3.), p(3., -2.), p(4., 0.)]
            .into_iter()
            .zip([1., 0.5, 2., 1.])
            .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
            .collect(),
        vec![2., 2., 2., 3., 7., 7., 7.],
    )
    .unwrap()
}
fn translation() -> AffineTransform3 {
    AffineTransform3::from_translation(Vector3::try_new(0., 2., 0.).unwrap())
}
fn check_edit(source: &NurbsCurve, output: &NurbsCurve, index: usize) {
    assert_eq!(source.degree(), output.degree());
    assert_eq!(source.knots(), output.knots());
    assert_eq!(source.control_points().len(), output.control_points().len());
    for (a, b) in source.control_points().iter().zip(output.control_points()) {
        assert_eq!(a.weight(), b.weight());
    }
    for (i, edit) in source.edit_points().unwrap().iter().enumerate() {
        let target = if i == index {
            translation().transform_point(edit.point).unwrap()
        } else {
            edit.point
        };
        assert!(
            output
                .evaluate_on_side(edit.parameter, edit.side)
                .unwrap()
                .distance_to(target)
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn edit_display_switches_modes_without_geometry_or_history_changes_and_stages_failures() {
    let mut doc = Document::default();
    let source = curve();
    let id = doc
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    // Unsupported edit-point layout must not switch an earlier valid source.
    let bad = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new_rational(
                1,
                vec![
                    WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                    WeightedPoint3::try_new(p(1., 1.), -1.).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    doc.add_geometry(Geometry::Point(p(8., 0.))).unwrap();
    doc.undo().unwrap();
    let snapshot = doc.object(id).unwrap().geometry_snapshot().clone();
    let label = doc.undo_label().map(str::to_owned);
    let version = doc.history.version;
    doc.enable_control_points([id]).unwrap();
    let pick = ControlPointId {
        object: id,
        index: 1,
    };
    doc.select_control_points([pick], SelectionMode::Add)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(doc.enable_edit_points([id, bad]).is_err());
    assert_eq!(format!("{doc:?}"), before);
    doc.enable_edit_points([id]).unwrap();
    assert_eq!(doc.grip_mode(id), Some(GripMode::Edit));
    assert_eq!(doc.selected_control_points().count(), 0);
    let points = source
        .edit_points()
        .unwrap()
        .iter()
        .map(|e| e.point)
        .collect::<Vec<_>>();
    assert_eq!(doc.control_point_locations(id).unwrap(), points);
    assert_ne!(points[1], source.control_points()[1].point());
    doc.select_control_points([pick], SelectionMode::Add)
        .unwrap();
    doc.enable_edit_points([id]).unwrap();
    assert_eq!(doc.selected_control_points().count(), 1);
    doc.enable_control_points([id]).unwrap();
    assert_eq!(doc.grip_mode(id), Some(GripMode::Control));
    assert_eq!(doc.selected_control_points().count(), 0);
    assert!(snapshot.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    assert_eq!(doc.undo_label(), label.as_deref());
    assert_eq!(doc.history.version, version);
    assert!(doc.can_redo());
}

#[test]
fn rational_and_periodic_edit_grips_reconstruct_partial_edits_and_replay_atomically() {
    let periodic = NurbsCurve::try_control_point_curve_with_closure(
        3,
        vec![
            p(2., 0.),
            p(1., 2.),
            p(-1., 2.),
            p(-2., 0.),
            p(-1., -2.),
            p(1., -2.),
        ],
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    for source in [curve(), periodic] {
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::NurbsCurve(source.clone()))
            .unwrap();
        doc.enable_edit_points([id]).unwrap();
        let pick = ControlPointId {
            object: id,
            index: 1,
        };
        doc.select_control_points([pick], SelectionMode::Add)
            .unwrap();
        let before = format!("{doc:?}");
        doc.begin_transaction("Cancelled edit points").unwrap();
        doc.transform_objects_and_grips(
            [],
            [pick],
            translation(),
            false,
            CopyGroupPolicy::Preserve,
        )
        .unwrap();
        doc.rollback_transaction().unwrap();
        assert_eq!(format!("{doc:?}"), before);
        doc.transform_objects_and_grips(
            [],
            [pick],
            translation(),
            false,
            CopyGroupPolicy::Preserve,
        )
        .unwrap();
        let Geometry::NurbsCurve(output) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        check_edit(&source, output, 1);
        assert_eq!(output.is_periodic(), source.is_periodic());
        let changed = output.clone();
        assert_eq!(doc.grip_mode(id), Some(GripMode::Edit));
        assert_eq!(
            doc.selected_control_points()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            [pick]
        );
        doc.undo().unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::NurbsCurve(source)
        );
        assert_eq!(doc.grip_mode(id), Some(GripMode::Edit));
        doc.redo().unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::NurbsCurve(changed)
        );
        assert_eq!(doc.grip_mode(id), Some(GripMode::Edit));
    }
}

#[test]
fn copying_edit_grips_preserves_mode_and_source_and_undo_redo_restores_the_copy() {
    let mut doc = Document::default();
    let source = curve();
    let id = doc
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    doc.enable_edit_points([id]).unwrap();
    let pick = ControlPointId {
        object: id,
        index: 1,
    };
    doc.select_control_points([pick], SelectionMode::Add)
        .unwrap();
    let (_, copies) = doc
        .transform_objects_and_grips([], [pick], translation(), true, CopyGroupPolicy::Preserve)
        .unwrap();
    let copy = copies[0];
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::NurbsCurve(source.clone())
    );
    let Geometry::NurbsCurve(output) = doc.object(copy).unwrap().geometry() else {
        panic!()
    };
    check_edit(&source, output, 1);
    assert_eq!(doc.grip_mode(copy), Some(GripMode::Edit));
    assert!(
        doc.control_points()
            .any(|(p, _, selected)| p.object == copy && p.index == 1 && selected)
    );
    doc.undo().unwrap();
    assert!(doc.object(copy).is_none());
    doc.redo().unwrap();
    assert_eq!(doc.grip_mode(copy), Some(GripMode::Edit));
}

#[test]
fn invalid_edit_grip_transform_preserves_whole_object_peers_and_redo() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve())).unwrap();
    let point = doc.add_geometry(Geometry::Point(p(0., 9.))).unwrap();
    doc.add_geometry(Geometry::Point(p(10., 0.))).unwrap();
    doc.undo().unwrap();
    doc.enable_edit_points([id]).unwrap();
    let before = format!("{doc:?}");
    assert!(
        doc.transform_objects_and_grips(
            [point],
            [ControlPointId {
                object: id,
                index: 4
            }],
            translation(),
            false,
            CopyGroupPolicy::Preserve
        )
        .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    // The ordinary peer stages a finite changed point before the later curve
    // endpoint overflows. Neither replacements nor copies may leak through.
    let overflow = AffineTransform3::try_nonuniform_scale(p(0., 0.), [f64::MAX, 2., 1.]).unwrap();
    for copy in [false, true] {
        assert!(
            doc.transform_objects_and_grips(
                [point],
                [ControlPointId {
                    object: id,
                    index: 3
                }],
                overflow,
                copy,
                CopyGroupPolicy::Preserve,
            )
            .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    assert!(doc.can_redo());
}
