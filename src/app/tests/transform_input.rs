use super::*;

#[test]
fn coordinate_commands_still_bypass_interactive_mode() {
    let mut app = test_app();
    app.command_input = "Point 7,8,9".to_owned();
    app.run_command();
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Point(point) if *point == Point3::try_new(7.0, 8.0, 9.0).unwrap()
    ));

    app.command_input = "Curve 0,0 1,1".to_owned();
    app.run_command();
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().nth(1).unwrap().geometry(),
        Geometry::NurbsCurve(curve) if curve.degree() == 1
    ));
}

#[test]
fn interactive_move_and_copy_use_the_selected_objects() {
    let mut app = test_app();
    let original = app
        .document
        .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
        .unwrap();
    app.document
        .select_object(original, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("Move"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Move {
            start: Some(point(0.0, 0.0, 0.0))
        })
    );
    app.accept_drafting_point(point(3.0, -1.0, 0.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.undo_label(), Some("Move"));
    assert!(matches!(
        app.document.object(original).unwrap().geometry(),
        Geometry::Point(position) if *position == point(4.0, 1.0, 0.0)
    ));

    assert!(app.try_start_interactive_command("Copy"));
    app.accept_drafting_point(point(4.0, 1.0, 0.0));
    app.accept_drafting_point(point(6.0, 4.0, 0.0));
    assert_eq!(app.document.undo_label(), Some("Copy"));
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![original]
    );
    let copy = app
        .document
        .objects()
        .find(|object| object.id() != original)
        .unwrap()
        .id();
    assert!(!app.document.is_selected(copy));
    assert_ne!(copy, original);
    assert!(matches!(
        app.document.object(copy).unwrap().geometry(),
        Geometry::Point(position) if *position == point(6.0, 4.0, 0.0)
    ));
    assert!(app.active_command.is_some());
    app.cancel_interactive_command(false);
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        Vec::<ObjectId>::new()
    );
    app.document.redo().unwrap();
    assert!(app.document.object(copy).is_some());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        Vec::<ObjectId>::new()
    );
}

#[test]
fn interactive_linear_array_uses_a_count_and_two_reference_points() {
    let mut app = test_app();
    let original = app
        .document
        .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
        .unwrap();
    app.document
        .select_object(original, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ArrayLinear 3"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ArrayLinear {
            item_count: 3,
            start: Some(point(0.0, 0.0, 0.0)),
        })
    );
    app.accept_drafting_point(point(2.0, -1.0, 0.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.undo_label(), Some("ArrayLinear"));
    assert!(app.document.is_selected(original));
    let mut locations = app
        .document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => point.to_array(),
            _ => panic!("expected arrayed points"),
        })
        .collect::<Vec<_>>();
    locations.sort_by(|left, right| left.partial_cmp(right).unwrap());
    assert_eq!(
        locations,
        vec![[1.0, 2.0, 0.0], [3.0, 1.0, 0.0], [5.0, 0.0, 0.0]]
    );

    assert!(!app.try_start_interactive_command("ArrayLinear"));
    assert!(!app.try_start_interactive_command("ArrayLinear 1"));
}

#[test]
fn interactive_rectangular_array_uses_counts_options_and_two_corners() {
    let mut app = test_app();
    let original = app
        .document
        .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
        .unwrap();
    app.document
        .select_object(original, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("Array 2 2 2 Mode=UnitCell ZDistance=4"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Array {
            counts: [2, 2, 2],
            fill: false,
            z_distance: 4.0,
            start: None,
        })
    );
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Array {
            counts: [2, 2, 2],
            fill: false,
            z_distance: 4.0,
            start: Some(point(0.0, 0.0, 0.0)),
        })
    );
    app.accept_drafting_point(point(2.0, -1.0, 0.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.undo_label(), Some("Array"));
    assert!(app.document.is_selected(original));
    let mut locations = app
        .document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => point.to_array(),
            _ => panic!("expected rectangular-array points"),
        })
        .collect::<Vec<_>>();
    locations.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let mut expected = Vec::new();
    for z in [0.0, 4.0] {
        for y in [0.0, -1.0] {
            for x in [0.0, 2.0] {
                expected.push([1.0 + x, 2.0 + y, z]);
            }
        }
    }
    expected.sort_by(|left, right| left.partial_cmp(right).unwrap());
    assert_eq!(locations, expected);

    assert!(!app.try_start_interactive_command("Array"));
    assert!(!app.try_start_interactive_command("Array 0 2"));
    assert!(!app.try_start_interactive_command("Array 2 2 2"));
    assert!(!app.try_start_interactive_command("Array 2 2 Mode=Maybe"));
    assert!(!app.try_start_interactive_command("Array 3 2 2 2 -1 4"));
}

#[test]
fn interactive_polar_array_uses_a_count_angle_options_and_center() {
    let mut app = test_app();
    let original = app
        .document
        .add_geometry(Geometry::Point(point(2.0, 0.0, 0.0)))
        .unwrap();
    app.document
        .select_object(original, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ArrayPolar 4 180 Rotate=No ZOffset=2"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ArrayPolar {
            item_count: 4,
            fill_angle_degrees: 180.0,
            rotate: false,
            z_offset: 2.0,
        })
    );
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.undo_label(), Some("ArrayPolar"));
    assert!(app.document.is_selected(original));
    let mut locations = app
        .document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected arrayed points"),
        })
        .collect::<Vec<_>>();
    locations.sort_by(|left, right| left.z().partial_cmp(&right.z()).unwrap());
    let root_three = 3.0_f64.sqrt();
    let expected = [
        point(2.0, 0.0, 0.0),
        point(1.0, root_three, 2.0),
        point(-1.0, root_three, 4.0),
        point(-2.0, 0.0, 6.0),
    ];
    for (actual, expected) in locations.into_iter().zip(expected) {
        assert!(actual.is_near(expected, app.document.tolerance()));
    }

    assert!(!app.try_start_interactive_command("ArrayPolar"));
    assert!(!app.try_start_interactive_command("ArrayPolar 1"));
    assert!(!app.try_start_interactive_command("ArrayPolar 4 0"));
    assert!(!app.try_start_interactive_command("ArrayPolar 4 Rotate=Maybe"));
}

#[test]
fn interactive_transforms_require_a_selection() {
    let mut app = test_app();
    for command in [
        "M",
        "Copy",
        "Array 3 2",
        "ArrayLinear 3",
        "ArrayPolar 4",
        "Scale",
        "Scale1D",
        "Scale2D",
        "Rotate",
        "Rotate3D",
        "Mirror",
        "Shear",
        "ExtrudeCrv",
        "ExtrudeCrvToPoint",
        "DupEdge",
        "DupFaceBorder",
        "ExtractMeshFaces",
        "DeleteFaces",
        "SwapMeshEdge",
        "CollapseMeshEdge",
        "SplitMeshEdge",
        "FillMeshHole",
        "DupMeshEdge",
        "DupMeshHoleBoundary",
        "WeldEdge",
        "WeldVertices",
        "UnweldEdge",
        "UnweldVertex",
        "ExtractIsocurve",
        "Revolve",
    ] {
        assert!(app.try_start_interactive_command(command));
        assert_eq!(app.active_command, None);
        if transform_prompt::supports_name(&command.to_ascii_lowercase())
            || translation_prompt::supports_name(&command.to_ascii_lowercase())
        {
            assert!(app.object_prompt.is_some());
            assert!(app.transform_session.is_none());
            app.cancel_interactive_command(false);
        } else {
            assert!(app.command_log.back().unwrap().contains("no objects"));
        }
    }
    // ExtractSrf supports command-first component selection.
    assert!(app.try_start_interactive_command("ExtractSrf"));
    assert!(app.picking_extract_faces());
    assert!(!app.document.can_undo());
    app.cancel_interactive_command(false);
}
