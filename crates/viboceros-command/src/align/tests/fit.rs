use super::*;

fn nonplanar() -> Document {
    let mut doc = Document::default();
    for a in [
        [4., 0., 0.],
        [-4., 0., 0.],
        [0., 4., 0.],
        [0., -4., 0.],
        [0., 0., 1.],
        [0., 0., 1.],
    ] {
        doc.add_geometry(cloud(a, [a[0] + 1., a[1] + 1., a[2] + 2.]))
            .unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.add_group(Some("all".into()), ids).unwrap();
    doc.select_all();
    doc
}

#[test]
fn fit_uses_one_bottom_anchor_per_object_preserving_shape_identity_and_undo() {
    let mut doc = nonplanar();
    let registry = CommandRegistry::with_builtins();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "Align ToFitPlane").unwrap();
    let after = doc.objects().cloned().collect::<Vec<_>>();
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.id(), b.id());
        assert_eq!(a.attributes(), b.attributes());
        assert_eq!(a.group_ids(), b.group_ids());
        let (Geometry::PointCloud(a), Geometry::PointCloud(b)) = (a.geometry(), b.geometry())
        else {
            panic!("cloud")
        };
        assert!((b.points()[0].z() - 1. / 3.).abs() < 1e-14);
        assert!((b.points()[1].z() - 7. / 3.).abs() < 1e-14);
        assert_eq!(a.points()[0].x(), b.points()[0].x());
        assert_eq!(a.points()[0].y(), b.points()[0].y());
    }
    assert_eq!(doc.undo_label(), Some("Align"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn insufficient_objects_release_only_postselection_without_consuming_redo() {
    let mut doc = nonplanar();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let ids = doc.objects().map(|o| o.id()).take(2).collect::<Vec<_>>();
    for post in [false, true] {
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let redo = doc.redo_label().map(str::to_owned);
        let input = "Align ToFitPlane AlignTo=World";
        let result = if post {
            registry.execute_postselected(&mut doc, input, CommandContext::default())
        } else {
            registry.execute(&mut doc, input)
        };
        assert!(matches!(
            result,
            Err(CommandError::InsufficientPlaneAlignmentObjects { actual: 2 })
        ));
        assert_eq!(doc.selected_object_ids().count(), if post { 0 } else { 2 });
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.redo_label(), redo.as_deref());
        assert!(
            registry
                .object_selection_prompt("Align")
                .unwrap()
                .unwrap()
                .command_line()
                .contains("World")
        );
    }
}

#[test]
fn exactly_planar_fit_is_a_no_op_and_invalid_options_do_not_clear_selection() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    for a in [[0., 0., 0.], [1., 2., 3.], [2., -1., 4.]] {
        doc.add_geometry(Geometry::Point(p(a))).unwrap();
    }
    doc.select_all();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    registry.execute(&mut doc, "Align ToFitPlane").unwrap();
    assert_eq!(format!("{doc:?}"), before);
    for input in [
        "Align ToFitPlane Auto",
        "Align ToFitPlane 1,2,3",
        "Align ToFitPlane 3Point",
    ] {
        assert!(
            registry
                .execute_postselected(&mut doc, input, CommandContext::default())
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn partial_group_selection_and_late_bounds_failure_are_atomic() {
    let mut doc = nonplanar();
    let registry = CommandRegistry::with_builtins();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    let untouched = doc.object(ids[5]).unwrap().clone();
    doc.select_objects_direct(ids[..5].iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry
        .execute_postselected(&mut doc, "Align ToFitPlane", CommandContext::default())
        .unwrap();
    assert_eq!(doc.object(ids[5]).unwrap(), &untouched);
    assert_eq!(doc.selected_object_ids().count(), 0);
    let pole = NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 4., 0.], [2., 0., 0.]]
            .into_iter()
            .zip([1., -1., 1.])
            .map(|(a, w)| WeightedPoint3::try_new(p(a), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    doc.add_geometry(Geometry::NurbsCurve(pole)).unwrap();
    doc.select_all();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute_postselected(&mut doc, "Align ToFitPlane", CommandContext::default())
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}
