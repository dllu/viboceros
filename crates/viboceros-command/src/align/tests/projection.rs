use super::*;

#[test]
fn bottom_center_projection_translates_objects_individually_despite_groups() {
    for (command, expected) in [
        (
            "ToLine 0,0,0 0,0,1",
            [[-1., -0.5, 0.], [-2., -1., 0.], [-0.5, -1., 5.]],
        ),
        (
            "ToPlane 0,0,0 1,0,0",
            [[0., -0.5, 0.], [5., -1., 0.], [16., -1., 5.]],
        ),
        (
            "ToPlane 3Point 0,0,0 1,0,0 0,1,0",
            [[0., 0., 0.], [5., 2., 0.], [16., 8., 0.]],
        ),
    ] {
        let mut doc = setup();
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        doc.add_group(Some("pair".into()), [ids[0], ids[1]])
            .unwrap();
        doc.add_group(Some("overlap".into()), [ids[1], ids[2]])
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, &format!("Align {command}"))
            .unwrap();
        assert_eq!(locations(&doc), expected, "{command}");
        let after = doc.objects().cloned().collect::<Vec<_>>();
        for (a, b) in before.iter().zip(&after) {
            assert_eq!(a.id(), b.id());
            assert_eq!(a.attributes(), b.attributes());
            assert_eq!(a.group_ids(), b.group_ids());
            let (Geometry::PointCloud(a), Geometry::PointCloud(b)) = (a.geometry(), b.geometry())
            else {
                panic!("cloud")
            };
            assert_eq!(
                a.points()[0].vector_to(a.points()[1]).unwrap(),
                b.points()[0].vector_to(b.points()[1]).unwrap()
            );
        }
        assert_eq!(doc.undo_label(), Some("Align"));
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        doc.redo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(doc.selected_object_ids().count(), 3);
    }
}

#[test]
fn cplane_chooses_bottom_center_and_two_point_plane_axis_not_display_origin() {
    let registry = CommandRegistry::with_builtins();
    for origin in [[10., -20., 30.], [1e100, -1e100, 1e100]] {
        let plane = Frame3::try_from_directions(
            p(origin),
            Vector3::try_from([0., 1., 0.]).unwrap(),
            Vector3::try_from([0., 0., 1.]).unwrap(),
            viboceros_geometry::Tolerance::DEFAULT,
        )
        .unwrap();
        for (command, expected) in [
            (
                "ToLine 0,0,0 0,0,1",
                [[0., -0.5, 0.], [0., -1., 0.], [0., -1., 5.]],
            ),
            (
                "ToPlane 0,0,0 0,1,0",
                [[0., 0., -1.5], [5., 2., -1.], [16., 8., -1.5]],
            ),
        ] {
            let mut doc = setup();
            registry
                .execute_in_context(
                    &mut doc,
                    &format!("Align {command}"),
                    CommandContext {
                        construction_plane: plane,
                    },
                )
                .unwrap();
            assert_eq!(locations(&doc), expected, "{command}");
        }
    }
}

#[test]
fn invalid_projection_inputs_are_atomic_and_partial_selection_never_moves_peers() {
    let mut doc = setup();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Align Left").unwrap();
    doc.undo().unwrap();
    for input in [
        "ToLine",
        "ToLine 1,2,3",
        "ToLine Auto",
        "ToLine 0,0,0 0,0,0",
        "ToLine 0,0,0 1,0,0 2,0,0",
        "ToLine 3Point 0,0,0 1,0,0",
        "Left 0,0,0 1,0,0",
        "ToPlane 0,0,0 0,0,1",
        "ToPlane 3Point 0,0,0 1,2,3 2,4,6",
        "ToPlane 3Point 0,0,0 1,0,0",
        "ToPlane Auto 0,0,0 1,0,0",
    ] {
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(&mut doc, &format!("Align {input}"))
                .is_err(),
            "{input}"
        );
        assert_eq!(format!("{doc:?}"), before, "{input}");
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.add_group(Some("group".into()), ids.iter().copied())
        .unwrap();
    doc.select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    registry
        .execute_postselected(
            &mut doc,
            "Align ToLine 0,0,0 0,0,1",
            CommandContext::default(),
        )
        .unwrap();
    assert_eq!(
        locations(&doc),
        [[0., 0., 0.], [5., 2., 0.], [-0.5, -1., 5.]]
    );
    assert_eq!(doc.selected_object_ids().count(), 0);
}

#[test]
fn staged_projection_state_round_trips_and_mode_changes_discard_old_references() {
    let state = AlignmentOptions::default()
        .parse(&["ToPlane", "3Point", "1,2,3", "4,5,6"])
        .unwrap();
    assert!(!state.ready());
    let ready = state.with_point(p([0., 0., 1.])).unwrap();
    assert!(ready.ready());
    assert_eq!(
        AlignmentOptions::default()
            .parse(
                &ready
                    .command_line()
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
        ready
    );
    let changed = state.parse(&["ToLine"]).unwrap();
    assert_eq!(changed.references, [None; 3]);
    assert!(!changed.three_point);
    assert!(ready.parse(&["3Point=No"]).is_err());
    assert!(state.parse(&["Auto"]).is_err());
}

#[test]
fn projection_no_op_preserves_redo_and_late_overflow_preserves_all_sources() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    doc.add_geometry(Geometry::Point(p([0., 0., 2.]))).unwrap();
    doc.select_all();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    registry
        .execute(&mut doc, "Align ToLine 0,0,0 0,0,1")
        .unwrap();
    assert_eq!(format!("{doc:?}"), before);
    doc.add_geometry(cloud([f64::MAX / 2., 0., 0.], [f64::MAX, 0., 0.]))
        .unwrap();
    doc.select_all();
    let before = format!("{doc:?}");
    let result = registry.execute(
        &mut doc,
        &format!("Align ToPlane {},0,0 {},1,0", f64::MAX, f64::MAX),
    );
    assert!(result.is_err());
    assert_eq!(format!("{doc:?}"), before);
}
