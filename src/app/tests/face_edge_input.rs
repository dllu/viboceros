use super::*;

#[test]
fn interactive_srfpt_collects_four_corners_and_uses_command_history() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("SurfaceFromCorners"));
    let corners = [
        point(0.0, 0.0, 0.0),
        point(4.0, 0.0, 0.0),
        point(4.0, 3.0, 0.0),
        point(0.0, 3.0, 0.0),
    ];
    for corner in corners[..3].iter().copied() {
        app.accept_drafting_point(corner);
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::SrfPt { .. })
        ));
    }
    app.accept_drafting_point(corners[3]);
    assert_eq!(app.active_command, None);
    let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created NURBS surface")
    };
    assert_eq!(surface.evaluate(0.5, 0.5).unwrap(), point(2.0, 1.5, 0.0));
    assert_eq!(app.document.undo_label(), Some("SrfPt"));
}

#[test]
fn interactive_extract_surface_face_click_targets_the_clicked_object() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 1,0,0 1,1,0 0,1,0");
    let first = app.document.objects().next().unwrap().id();
    app.execute_command("SrfPt 3,0,0 4,0,0 4,1,0 3,1,0");
    let second = app
        .document
        .objects()
        .find(|object| object.id() != first)
        .unwrap()
        .id();
    app.document
        .select_objects_direct([first, second], viboceros_document::SelectionMode::Replace)
        .unwrap();
    assert!(app.try_start_interactive_command("ExtractSrf Copy=No"));
    app.accept_face_click(second, 0);
    assert!(app.document.object(second).is_some());
    app.command_input.clear();
    app.run_command();
    assert!(app.document.object(first).is_some());
    assert!(app.document.object(second).is_none());
    assert_eq!(app.document.undo_label(), Some("ExtractSrf"));

    app.execute_command("Undo");
    app.document
        .select_objects_direct([first, second], viboceros_document::SelectionMode::Replace)
        .unwrap();
    assert!(app.try_start_interactive_command("DupFaceBorder"));
    app.accept_face_click(second, 0);
    assert_eq!(app.document.objects().len(), 3);
    assert!(app.document.object(first).is_some());
    assert!(app.document.object(second).is_some());
    assert_eq!(app.document.undo_label(), Some("DupFaceBorder"));
}

#[test]
fn interactive_extract_surface_accumulates_a_face_location_until_enter() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,2 4,0,2 4,3,2 0,3,2");
    let source = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ExtractSurface Copy=No OutputLayer=Input"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtractSrf {
            copy: false,
            output_on_current_layer: false,
        })
    );
    assert!(app.command_log.back().unwrap().contains("select faces"));
    app.accept_drafting_point(point(2.0, 1.0, 5.0));
    assert!(app.document.object(source).is_some());
    app.command_input.clear();
    app.run_command();

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source).is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.selected_object_count(), 1);
    assert!(matches!(
        app.document.selected_objects().next().unwrap().geometry(),
        Geometry::NurbsSurface(_)
    ));
    assert_eq!(app.document.undo_label(), Some("ExtractSrf"));
    assert!(!app.try_start_interactive_command("ExtractSrf Copy=Maybe"));
    assert!(!app.try_start_interactive_command("ExtractSrf OutputLayer=Other"));
}

#[test]
fn interactive_curvature_evaluates_one_pick_and_can_cancel_without_mutation() {
    let mut app = test_app();
    app.execute_command("Circle 0,0,0 2");
    app.execute_command("SelAll");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(app.try_start_interactive_command("Curvature MarkCurvature=Yes"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Curvature { mark: true })
    );
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.try_start_interactive_command("Curvature"));
    app.accept_drafting_point(point(2.0, 0.0, 0.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.try_start_interactive_command("Curvature MarkCurvature=Yes"));
    app.accept_drafting_point(point(2.0, 0.0, 0.0));
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.undo_label(), Some("Curvature"));
    app.execute_command("Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.try_start_interactive_command("Curvature MarkCurvature=Maybe"));
    assert!(!app.try_start_interactive_command("Curvature 2,0,0"));
}

#[test]
fn interactive_duplicate_face_border_uses_one_face_location_pick() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,2 4,0,2 4,3,2 0,3,2");
    let source = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("DuplicateFaceBorder OutputLayer=Input"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::DupFaceBorder {
            output_on_current_layer: false,
        })
    );
    assert!(app.command_log.back().unwrap().contains("face location"));
    app.accept_drafting_point(point(2.0, 1.0, 5.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source).is_some());
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.selected_object_count(), 1);
    assert!(matches!(
        app.document.selected_objects().next().unwrap().geometry(),
        Geometry::Polyline(_)
    ));
    assert_eq!(app.document.undo_label(), Some("DupFaceBorder"));
    assert!(!app.try_start_interactive_command("DupFaceBorder OutputLayer=Other"));
    assert!(
        !app.try_start_interactive_command("DupFaceBorder OutputLayer=Input OutputLayer=Current")
    );
}

#[test]
fn interactive_duplicate_edge_uses_one_edge_location_pick() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,2 4,0,2 4,3,2 0,3,2");
    let source = app.document.objects().next().unwrap().id();
    let input_layer = app.document.object(source).unwrap().attributes().layer_id();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("DuplicateEdge OutputLayer=Input"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::DupEdge {
            output_on_current_layer: false,
        })
    );
    assert!(app.command_log.back().unwrap().contains("edge location"));
    app.accept_drafting_point(point(2.0, -0.25, 2.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source).is_some());
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.objects().len(), 2);
    let output = app.document.selected_objects().next().unwrap();
    assert_eq!(output.attributes().layer_id(), input_layer);
    assert!(matches!(output.geometry(), Geometry::NurbsCurve(_)));
    assert_eq!(app.document.undo_label(), Some("DupEdge"));
    assert!(!app.try_start_interactive_command("DupEdge OutputLayer=Other"));
    assert!(!app.try_start_interactive_command("DupEdge OutputLayer=Input OutputLayer=Current"));
}
