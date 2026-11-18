use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;
use crate::app::nested_measurement::Kind;
use viboceros_command::chain_selection::Target;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn nested_distance_undo_and_escape_preserve_the_parent_point() {
    let mut app = test_app();
    for input in ["Circle", "w10,20,2", "'Distance", "Units", "_Centimeter"] {
        enter(&mut app, input);
    }
    let center = point(10., 20., 2.);
    assert_eq!(app.nested_measurement_kind(), Some(Kind::Distance));
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.viewport_drafting_input(false, false).anchor,
        Some(point(0., 0., 0.))
    );
    enter(&mut app, "Undo");
    assert_eq!(app.last_point, Some(center));
    enter(&mut app, "w-3,0,5");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.last_point, Some(center));
    assert_eq!(app.nested_measurement_kind(), None);
    assert!(
        matches!(app.active_command, Some(InteractiveCommand::Circle { center: Some(p) }) if p == center)
    );
    assert!(!app.document.can_undo());

    for input in ["'Distance", "w0,0,0", "r3,4,0"] {
        enter(&mut app, input);
    }
    assert_eq!(app.nested_measurement_kind(), None);
    assert_eq!(app.active_command, None, "{:?}", app.command_log);
    let Geometry::Circle(circle) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected Circle");
    };
    assert_eq!(circle.center(), center);
    assert_eq!(circle.radius(), 5.);
    assert_eq!(app.document.undo_label(), Some("Circle"));
}

#[test]
fn nested_length_combines_a_viewport_pick_with_a_temporary_portion() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.execute_command("Line w-4,0,0 w4,0,0");
    let source = app.document.objects().next().unwrap().id();
    app.execute_command("Line w-2,2,0 w1,2,0");
    let whole = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();

    for input in [
        "Fillet Radius=0.5",
        "Radius",
        "'Length",
        "Units=Centimeters",
    ] {
        enter(&mut app, input);
    }
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(&mut app, &context, Vec::new());
    let location = screen_point(&app, point(0., 2., 0.));
    viewport_click(&mut app, &context, location);
    assert_eq!(app.nested_length_highlights(), vec![Target::Curve(whole)]);
    for input in ["SubCrv", &format!("SelID {source}"), "w-2,0,0", "w2,0,0"] {
        enter(&mut app, input);
    }
    assert!(app.subcurve_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.nested_measurement
            .as_ref()
            .unwrap()
            .length_guides()
            .len(),
        1
    );
    assert_eq!(app.nested_measurement_kind(), Some(Kind::Length));
    enter(&mut app, "Enter");
    assert_eq!(app.nested_measurement_kind(), None);
    assert_eq!(app.commands.fillet_preferences().radius, 7.);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert!(!app.document.can_undo());
}

#[test]
fn nested_length_chain_and_edge_stay_private_during_surface_offset() {
    let mut app = test_app();
    app.execute_command("Line w0,0,0 w3,0,0");
    let first = app.document.objects().next().unwrap().id();
    app.execute_command("Line w3,0,0 w5,0,0");
    app.execute_command("Plane w10,10,0 w14,14,0");
    let plane = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([plane], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    for input in ["OffsetSrf", "Distance", "'Length", "SelChain"] {
        enter(&mut app, input);
    }
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    assert_eq!(
        app.nested_measurement_kind(),
        Some(Kind::Length),
        "{:?}",
        app.command_log
    );
    assert!(app.chain_prompt.is_some(), "{:?}", app.command_log);
    app.pick_chain(vec![Target::Curve(first)]);
    enter(&mut app, "All");
    enter(&mut app, "Enter");
    assert!(app.chain_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.nested_length_highlights().len(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    enter(&mut app, &format!("Edge={plane},0"));
    assert_eq!(
        app.nested_length_highlights().len(),
        3,
        "{:?}",
        app.command_log
    );
    enter(&mut app, "");
    assert_eq!(app.nested_measurement_kind(), None);
    assert_eq!(app.commands.offset_surface_options().distance, 9.);
    assert!(app.offset_surface_session.is_some());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert!(
        app.commands
            .component_selection()
            .live_picks(&app.document)
            .is_empty()
    );
    assert!(!app.document.can_undo());
}
