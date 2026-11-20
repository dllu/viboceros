use super::*;

#[test]
fn sweep_two_rail_closed_profile_seams_drag_and_commit_the_preview() {
    let mut app = test_app();
    for command in [
        "Line 1,0,0 2,0,5",
        "Line -1,0,0 -2,0,5",
        "Circle 0,0,0 1",
        "Circle 0,0,5 2",
    ] {
        enter(&mut app, command);
    }
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(before.len(), 4);
    app.document.clear_history().unwrap();
    app.restore_curve_selection(before.iter().map(|object| object.id()).collect());
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    let markers = |app: &VibocerosApp| {
        app.sweep_two_rails
            .as_ref()
            .unwrap()
            .seam_editor()
            .unwrap()
            .markers()
    };
    assert_eq!(markers(&app).len(), 2);
    assert!(app.command_hint().contains("SnapToKnots=Yes"));
    let natural = wait_for_draft(&mut app, &context);
    let start = point(1., 0., 0.);
    let finish = point(0., 1., 0.);
    let from = screen_point(&app, start);
    let to = screen_point(&app, finish);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(to)]);
    assert!(markers(&app)[0].point.distance_to(finish).unwrap() < 1e-8);
    wait_for_edit(&mut app, &context, &natural);
    assert_eq!(app.document.undo_label(), None);
    viewport_frame(&mut app, &context, vec![button(to, false)]);
    enter(&mut app, "Undo");
    assert!(markers(&app)[0].point.distance_to(start).unwrap() < 1e-8);

    let original = markers(&app)[0].tangent.unwrap();
    enter(&mut app, "Flip");
    assert!(app.command_hint().contains("flip"));
    enter(&mut app, "w1,0,0");
    assert!(markers(&app)[0].tangent.unwrap().dot(original).unwrap() < -0.99);
    enter(&mut app, "Natural");
    assert!(markers(&app)[0].tangent.unwrap().dot(original).unwrap() > 0.99);
    enter(&mut app, "SnapToKnots=No");
    enter(&mut app, &format!("SelID {}", before[2].id()));
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    enter(&mut app, &format!("w{diagonal},{diagonal},0"));
    let target = point(diagonal, diagonal, 0.);
    assert!(markers(&app)[0].point.distance_to(target).unwrap() < 1e-8);
    let preview = wait_for_edit(&mut app, &context, &natural);
    enter(&mut app, "");
    assert!(app.command_hint().contains("AddSlash"));
    assert_eq!(wait_for_draft(&mut app, &context), preview);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);

    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(result) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected the preview to commit one Sweep2 B-rep");
    };
    assert_eq!(result.faces().len(), 1);
    assert_eq!(
        result
            .wireframe_curves(1, app.document.tolerance())
            .unwrap(),
        preview
    );
    let surface = result.faces()[0].surface();
    assert!(surface.is_closed_v().unwrap());
    let seam = surface
        .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
        .unwrap();
    assert!(seam.distance_to(target).unwrap() < 1e-8);
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

fn wait_for_edit(
    app: &mut VibocerosApp,
    context: &egui::Context,
    previous: &[NurbsCurve],
) -> Vec<NurbsCurve> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let wires = wait_for_draft(app, context);
        if wires != previous {
            return wires;
        }
        assert!(Instant::now() < deadline, "{:?}", app.command_log);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn sweep_two_rail_point_end_advances_to_closed_profile_seams() {
    let mut app = test_app();
    for command in ["Line 1,0,0 0,0,5", "Line -1,0,0 0,0,5", "Circle 0,0,0 1"] {
        enter(&mut app, command);
    }
    let inputs = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    for id in inputs {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "Point");
    assert!(app.command_hint().contains("end point"));
    enter(&mut app, "w0,0,5");
    assert!(app.command_hint().contains("Drag seam"));
    let draft = wait_for_draft(&mut app, &context);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    enter(&mut app, "");
    let Geometry::Brep(result) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected the draft point end to commit");
    };
    assert_eq!(
        result
            .wireframe_curves(1, app.document.tolerance())
            .unwrap(),
        draft
    );
    assert_eq!(result.edges().len(), 2);
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
}
