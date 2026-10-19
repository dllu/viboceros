use super::*;

#[test]
fn nonsquare_surface_rows_use_parameter_directions_and_preserve_redo_and_peers() {
    let mut doc = Document::default();
    let surface = NurbsSurface::try_new(
        2,
        2,
        3,
        5,
        (0..5)
            .flat_map(|v| (0..3).map(move |u| Point3::try_new(u as f64, v as f64, 0.).unwrap()))
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 2., 3., 3., 3.],
    )
    .unwrap();
    let owner = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    let peer = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.enable_control_points([owner]).unwrap();
    doc.select_objects_direct([peer], SelectionMode::Add)
        .unwrap();
    doc.add_geometry(Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    let original = doc.object(owner).unwrap().clone();
    let labels = (
        doc.undo_label().map(str::to_owned),
        doc.redo_label().map(str::to_owned),
    );
    let registry = CommandRegistry::with_builtins();
    for (command, expected) in [
        ("SelU", vec![1, 6, 11]),
        ("SelV", vec![5, 6, 7, 8, 9]),
        ("NextU", vec![11]),
        ("NextV", vec![7]),
        ("AddPrevV", vec![5, 6]),
    ] {
        doc.clear_control_point_selection();
        doc.select_control_points(
            [ControlPointId {
                object: owner,
                index: 6,
            }],
            SelectionMode::Add,
        )
        .unwrap();
        registry.execute(&mut doc, command).unwrap();
        assert_eq!(
            doc.selected_control_points()
                .map(|(g, _)| g.index)
                .collect::<Vec<_>>(),
            expected,
            "{command}"
        );
        assert!(doc.is_selected(peer));
        assert_eq!(doc.object(owner), Some(&original));
        assert_eq!(
            (
                doc.undo_label().map(str::to_owned),
                doc.redo_label().map(str::to_owned)
            ),
            labels
        );
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 3);
}

#[test]
fn navigation_parsing_and_visibility_filters_leave_invalid_batches_atomic() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, [control, edit, _]) = super::super::tests::setup();
    doc.select_control_points(
        [
            ControlPointId {
                object: control,
                index: 0,
            },
            ControlPointId {
                object: edit,
                index: 1,
            },
        ],
        SelectionMode::Add,
    )
    .unwrap();
    doc.set_objects_visibility([control], false).unwrap();
    doc.set_objects_locked([edit], true).unwrap();
    doc.add_geometry(Geometry::Point(Point3::try_new(20., 30., 40.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    let original = format!("{doc:?}");
    for action in PointSelectionAction::ALL {
        let PointSelectionAction::Navigate(_) = action else {
            continue;
        };
        let input = format!("_-{}", action.name().to_ascii_lowercase());
        assert_eq!(
            PointSelectionAction::from_input(&input).unwrap(),
            Some(action)
        );
        registry.execute(&mut doc, &input).unwrap();
        assert_eq!(format!("{doc:?}"), original);
        assert!(
            registry
                .execute(&mut doc, &format!("{input} extra"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), original);
    }
}
