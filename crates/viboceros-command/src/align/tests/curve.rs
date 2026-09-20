use super::*;
use viboceros_geometry::{LineSegment, Tolerance};

fn line(doc: &mut Document) -> viboceros_document::ObjectId {
    doc.add_geometry(Geometry::Line(
        LineSegment::try_new(p([0., 0., 1.]), p([10., 0., 1.]), Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap()
}

#[test]
fn curve_state_roundtrips_and_switching_modes_discards_the_target() {
    let mut doc = Document::default();
    let id = line(&mut doc);
    assert_eq!(
        id.to_string()
            .parse::<viboceros_document::ObjectId>()
            .unwrap(),
        id
    );
    let args = format!("ToCurve CurveId={id}");
    let options = AlignmentOptions::default()
        .parse(&args.split_whitespace().collect::<Vec<_>>())
        .unwrap();
    assert!(options.ready());
    assert_eq!(options.curve, Some(id));
    assert_eq!(
        AlignmentOptions::default()
            .parse(
                &options
                    .command_line()
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
        options
    );
    for input in ["Auto", "0,0,0", "3Point", "CurveId=bad"] {
        assert!(options.parse(&[input]).is_err(), "{input}");
    }
    assert!(options.parse(&["Left"]).unwrap().curve.is_none());
    assert!(options.parse(&["ToLine"]).unwrap().curve.is_none());
    let duplicate = format!("ToCurve CurveId={id} CurveId={id}");
    assert!(
        AlignmentOptions::default()
            .parse(&duplicate.split_whitespace().collect::<Vec<_>>())
            .is_err()
    );
    let invalid = format!("Left CurveId={id}");
    assert!(
        AlignmentOptions::default()
            .parse(&invalid.split_whitespace().collect::<Vec<_>>())
            .is_err()
    );
    assert!(
        !AlignmentOptions::default()
            .parse(&["ToCurve"])
            .unwrap()
            .ready()
    );
}

#[test]
fn curve_alignment_clamps_ends_preserves_target_and_is_one_reversible_edit() {
    for postselected in [false, true] {
        let mut doc = Document::default();
        let ids = [[-10., 5., 3.], [5., 5., 3.], [20., 5., 3.]]
            .map(|q| doc.add_geometry(Geometry::Point(p(q))).unwrap());
        let target = line(&mut doc);
        doc.add_group(Some("sources and target".into()), [ids[0], ids[1], target])
            .unwrap();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        let cmd = format!("Align ToCurve CurveId={target} AlignTo=World");
        if postselected {
            registry
                .execute_postselected(&mut doc, &cmd, CommandContext::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, &cmd).unwrap();
        }
        for (id, expected) in ids
            .into_iter()
            .zip([[0., 0., 1.], [5., 0., 1.], [10., 0., 1.]])
        {
            assert_eq!(
                doc.object(id).unwrap().geometry(),
                &Geometry::Point(p(expected))
            );
            assert_eq!(doc.is_selected(id), !postselected);
        }
        assert_eq!(
            doc.object(target).unwrap(),
            before.iter().find(|o| o.id() == target).unwrap()
        );
        let after = doc.objects().cloned().collect::<Vec<_>>();
        for (a, b) in before.iter().zip(&after) {
            assert_eq!(a.id(), b.id());
            assert_eq!(a.attributes(), b.attributes());
            assert_eq!(a.group_ids(), b.group_ids());
        }
        assert_eq!(doc.undo_label(), Some("Align"));
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        doc.redo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn failed_curve_targets_leave_geometry_selection_and_history_unchanged() {
    let mut doc = setup();
    let point = doc.add_geometry(Geometry::Point(p([3., 4., 5.]))).unwrap();
    let missing = line(&mut Document::default());
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selection = doc.selected_object_ids().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    for id in [point, missing] {
        assert!(
            CommandRegistry::with_builtins()
                .execute_postselected(
                    &mut doc,
                    &format!("Align ToCurve CurveId={id}"),
                    CommandContext::default()
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selection);
        assert_eq!(doc.undo_label(), history.as_deref());
    }
}

#[test]
fn selected_target_and_late_translation_failure_are_atomic() {
    let mut doc = Document::default();
    let a = doc.add_geometry(Geometry::Point(p([0., 0., 0.]))).unwrap();
    let b = doc
        .add_geometry(Geometry::Point(p([-f64::MAX, 0., 0.])))
        .unwrap();
    let target = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                p([f64::MAX, 0., 0.]),
                p([f64::MAX, 1., 0.]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    let cmd = format!("Align ToCurve CurveId={target}");
    doc.select_objects_direct([target], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut doc, &cmd),
        Err(CommandError::AlignmentTargetSelected)
    ));
    doc.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let label = doc.undo_label().map(str::to_owned);
    assert!(registry.execute(&mut doc, &cmd).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), label.as_deref());
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [a, b]);
}

#[test]
fn on_target_points_are_no_ops_and_world_vs_cplane_uses_bottom_center() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p([3., 0., 1.]))).unwrap();
    let target = line(&mut doc);
    doc.select_objects_direct([point], SelectionMode::Replace)
        .unwrap();
    let label = doc.undo_label().map(str::to_owned);
    registry
        .execute(&mut doc, &format!("Align ToCurve CurveId={target}"))
        .unwrap();
    assert_eq!(doc.undo_label(), label.as_deref());
    let frame = Frame3::try_from_directions(
        p([10., -20., 30.]),
        Vector3::try_from([0., 1., 0.]).unwrap(),
        Vector3::try_from([0., 0., 1.]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (system, expected) in [("World", [0., -0.5, 1.]), ("CPlane", [0., -0.5, -0.5])] {
        let mut doc = Document::default();
        let id = doc.add_geometry(cloud([0., 0., 0.], [2., 1., 3.])).unwrap();
        let target = line(&mut doc);
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute_in_context(
                &mut doc,
                &format!("Align ToCurve CurveId={target} AlignTo={system}"),
                CommandContext {
                    construction_plane: frame,
                },
            )
            .unwrap();
        let Geometry::PointCloud(c) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(c.points()[0], p(expected));
    }
}
