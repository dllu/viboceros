use super::*;

fn source_only() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    (app, ids)
}
fn output_point(app: &VibocerosApp) -> Point3 {
    let Geometry::Point(p) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    *p
}

#[test]
fn project_axis_planes_are_baked_at_selection_and_leave_cplanes_unchanged() {
    let (mut app, _) = source_only();
    let frame = app.viewports[0]
        .construction_plane()
        .with_origin(point(0., 0., 2.));
    app.viewports[0].set_construction_plane(frame);
    enter(&mut app, "Project");
    enter(&mut app, "IP");
    enter(&mut app, "CPlaneXY");
    let changed = frame.with_origin(point(0., 0., 7.));
    app.viewports[0].set_construction_plane(changed);
    enter(&mut app, "");
    assert_eq!(output_point(&app), point(5., 5., 2.));
    assert_eq!(app.viewports[0].construction_plane(), changed);
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("Project"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 1);
}

#[test]
fn project_three_point_plane_keeps_invalid_points_active_and_undo_is_local() {
    let (mut app, _) = source_only();
    let frame = app.viewports[0].construction_plane();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Project");
    enter(&mut app, "IPlane");
    enter(&mut app, "3Point");
    assert!(app.viewport_object_filter().is_none());
    assert!(app.accept_drafting_point(point(0., 0., 2.)));
    assert!(!app.accept_drafting_point(point(0., 0., 2.)));
    enter(&mut app, "Undo");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ProjectPlane { step: 0, .. })
    ));
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "w0,0,2");
    enter(&mut app, "w1,0,2");
    assert!(!app.accept_drafting_point(point(2., 0., 2.)));
    assert!(app.picking_project_plane_points());
    enter(&mut app, "w0,1,2");
    assert!(!app.picking_project_plane_points());
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "");
    assert_eq!(output_point(&app), point(5., 5., 2.));
    assert_eq!(app.viewports[0].construction_plane(), frame);
}

#[test]
fn project_rectangle_vertical_and_through_point_modes_use_shared_drafting() {
    for (mode, points, direction, expected) in [
        (
            "Rectangle",
            vec!["w0,0,1", "w2,2,1"],
            "0,0,1",
            point(5., 5., 1.),
        ),
        (
            "Vertical",
            vec!["w0,0,0", "w10,0,0", "w1,0,1"],
            "0,1,0",
            point(5., 0., -3.),
        ),
        ("ThroughPoint", vec!["w0,0,4"], "0,0,1", point(5., 5., 4.)),
    ] {
        let (mut app, _) = source_only();
        let frame = app.viewports[0].construction_plane();
        enter(&mut app, "Project");
        enter(&mut app, &format!("Direction={direction}"));
        enter(&mut app, "InfinitePlane");
        enter(&mut app, mode);
        for p in points {
            enter(&mut app, p);
        }
        assert!(!app.picking_project_plane_points());
        enter(&mut app, "");
        assert_eq!(output_point(&app), expected);
        assert_eq!(app.viewports[0].construction_plane(), frame);
    }
}

#[test]
fn project_object_and_curve_plane_references_use_filtered_picking() {
    let (mut app, ids) = source_only();
    let reference = app.document.object(ids[0]).unwrap().clone();
    enter(&mut app, "Project");
    enter(&mut app, "IP");
    enter(&mut app, "Object");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.object(ids[0]), Some(&reference));
    enter(&mut app, "");
    assert_eq!(output_point(&app), point(5., 5., 0.));
    let (mut app, ids) = source_only();
    enter(&mut app, "Project Direction=1,0,0");
    enter(&mut app, "IP");
    enter(&mut app, "AroundCurve");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    enter(&mut app, &ids[1].to_string());
    assert!(app.picking_project_plane_points());
    enter(&mut app, "w3,5,3");
    enter(&mut app, "");
    assert_eq!(output_point(&app), point(3., 5., -3.));
}

#[test]
fn project_saved_cplanes_and_repeated_planes_do_not_restore_view_settings() {
    let (mut app, _) = source_only();
    let frame = app.viewports[0].construction_plane();
    app.viewports[0].set_construction_plane(frame.with_origin(point(0., 0., 4.)));
    enter(&mut app, "NamedCPlane Save target plane");
    assert!(app.named_cplanes.get("target plane").is_ok());
    app.viewports[0].set_construction_plane(frame);
    enter(&mut app, "Project");
    enter(&mut app, "IP");
    enter(&mut app, "");
    enter(&mut app, "IP");
    enter(&mut app, "NamedCPlane");
    enter(&mut app, "\"target plane\"");
    enter(&mut app, "");
    assert_eq!(app.document.selected_object_count(), 2);
    let points = app
        .document
        .selected_objects()
        .map(|o| match o.geometry() {
            Geometry::Point(p) => *p,
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(points, [point(5., 5., 0.), point(5., 5., 4.)]);
    assert_eq!(app.viewports[0].construction_plane(), frame);
}

#[test]
fn project_plane_cancel_enter_and_direction_switch_preserve_transient_state() {
    for stage in 0..=2 {
        let (mut app, ids) = source_only();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let history = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "Project");
        enter(&mut app, "IP");
        enter(&mut app, "3Point");
        if stage > 0 {
            enter(&mut app, "w0,0,2");
        }
        if stage > 1 {
            enter(&mut app, "w1,0,2");
        }
        app.cancel_current_prompt_or_selection();
        assert!(app.intersection_prompt.is_none());
        assert!(app.active_command.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.document.is_selected(ids[2]));
        assert_eq!(app.document.undo_label(), history.as_deref());
    }
    let (mut app, _) = source_only();
    enter(&mut app, "Project");
    enter(&mut app, "IP");
    enter(&mut app, "WorldXY");
    enter(&mut app, "IP");
    enter(&mut app, "3Point");
    enter(&mut app, "w0,0,2");
    enter(&mut app, "");
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .projection
            .as_ref()
            .unwrap()
            .planes
            .len(),
        1
    );
    enter(&mut app, "IP");
    enter(&mut app, "ThroughPoint");
    enter(&mut app, "Direction=Custom");
    assert!(app.picking_project_direction());
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w0,0,1");
    enter(&mut app, "");
    assert_eq!(output_point(&app), point(5., 5., 0.));
}

#[test]
fn project_through_point_plane_uses_a_real_viewport_endpoint_snap() {
    let (mut app, _) = source_only();
    app.viewports[0] = Viewport::new(crate::viewport::ViewKind::Top);
    app.active_viewport = 0;
    enter(&mut app, "Project");
    enter(&mut app, "IP");
    enter(&mut app, "ThroughPoint");
    let context = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
        let mut output = ViewportOutput::default();
        let input = ViewportInput {
            object_filter: app.viewport_object_filter(),
            drafting: DraftingInput {
                active: app.picking_project_plane_points(),
                osnap: viboceros_drafting::ObjectSnapModes::only(
                    viboceros_drafting::ObjectSnapKind::End,
                ),
                ..Default::default()
            },
            ..Default::default()
        };
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
                },
            )
            .drop_without_applying_deltas();
        output
    };
    frame(&mut app, vec![]);
    let expected = point(1., 5., 3.);
    let position = app.viewports[0].project(expected, rect).unwrap() + egui::vec2(1., 1.);
    let mut picked = false;
    for pressed in [true, false] {
        let output = frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        if let Some(p) = output.picked_point {
            assert_eq!(p, expected);
            picked = true;
        }
        app.handle_viewport_action(output);
    }
    assert!(picked);
    assert!(!app.picking_project_plane_points());
    enter(&mut app, "");
    assert_eq!(output_point(&app), point(5., 5., 3.));
}
