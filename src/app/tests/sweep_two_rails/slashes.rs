use super::*;

#[test]
fn sweep_two_rail_slash_picker_previews_and_commits_rail_alignments() {
    let mut app = test_app();
    for input in ["Line 0,0,0 8,0,0", "Line 0,2,0 8,2,0"] {
        enter(&mut app, input);
    }
    for (x, height) in [(0., 0.8), (8., 1.2)] {
        let arch = NurbsCurve::try_new(
            2,
            vec![point(x, 0., 0.), point(x, 1., height), point(x, 2., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        app.document
            .add_geometry(Geometry::NurbsCurve(arch))
            .unwrap();
    }
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.document
        .select_objects_direct(before.iter().map(|o| o.id()), SelectionMode::Replace)
        .unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    let original = wait_for_draft(&mut app, &context);
    enter(&mut app, "AddSlash");
    assert!(app.command_hint().contains("either rail"));
    enter(&mut app, "w5,2,0");
    assert!(app.command_hint().contains("other rail"));
    let pixel = screen_point(&app, point(2., 2., 0.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pixel)]);
    let deadline = Instant::now() + Duration::from_secs(8);
    let guided = loop {
        let frame = wait_for_draft(&mut app, &context);
        if frame != original {
            break frame;
        }
        assert!(Instant::now() < deadline);
    };
    assert!(app.command_hint().contains("other rail"));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    viewport_click(&mut app, &context, pixel);
    assert!(app.command_hint().contains("either rail"));
    assert_eq!(wait_for_draft(&mut app, &context), guided);
    enter(&mut app, "");
    assert!(app.command_hint().contains("AddSlash"));
    enter(&mut app, "AddSlash");
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), original);

    // Typed points can start on either rail. An invalid second pair keeps
    // the first selected point and leaves the accepted guide in place.
    enter(&mut app, "w5,1.4,0");
    enter(&mut app, "w2,0.2,0");
    assert_eq!(app.last_point, Some(point(2., 0., 0.)));
    let accepted = wait_for_draft(&mut app, &context);
    enter(&mut app, "w6,0,0");
    enter(&mut app, "w3,2,0");
    assert!(app.command_hint().contains("other rail"));
    app.cancel_current_prompt_or_selection();
    assert!(app.command_hint().contains("AddSlash"));
    assert_eq!(wait_for_draft(&mut app, &context), accepted);
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    assert_eq!(
        brep.wireframe_curves(1, app.document.tolerance()).unwrap(),
        accepted
    );
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
