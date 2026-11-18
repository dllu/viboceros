use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use viboceros_command::paraboloid::Preferences;

#[test]
fn paraboloid_viewport_preview_and_click_share_the_exact_wall() {
    for mode in ["Focus", "Vertex", "Direction"] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.maximized_viewport = Some(0);
        app.osnap = false;
        let plane = app.viewports[0].construction_plane();
        enter(&mut app, "Paraboloid MarkFocus=Yes");
        if mode != "Focus" {
            enter(&mut app, "Vertex");
        }
        enter(&mut app, "w1,2,3");
        if mode == "Direction" {
            enter(&mut app, "Direction");
        }
        enter(&mut app, if mode == "Vertex" { "w1,2,4" } else { "w1,2,8" });
        enter(
            &mut app,
            if mode == "Vertex" {
                "Solid=No"
            } else {
                "Solid=Yes"
            },
        );
        if mode == "Direction" {
            enter(&mut app, "2");
        }

        let ui = egui::Context::default();
        viewport_frame(&mut app, &ui, vec![]);
        viewport_frame(&mut app, &ui, vec![]);
        let raw = point(4., 6., 0.);
        let target = screen_point(&app, raw);
        let first_cursor = screen_point(&app, point(5., 5., 0.));
        let first_wire =
            viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(first_cursor)]);
        let final_wire = viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(target)]);
        assert!(first_wire.len() > 32 && final_wire.len() > 32, "{mode}");
        assert_ne!(first_wire, final_wire, "{mode}");
        assert_eq!(app.document.objects().len(), 0);
        assert!(!app.document.can_undo());

        let picked = app
            .point_constraint
            .map_or(raw, |constraint| constraint.apply_cursor(raw).unwrap());
        let input = app.paraboloid_session.as_ref().unwrap().input;
        let expected = input.preview(picked).unwrap();
        let wall = expected.wall().unwrap();
        let seam = wall.control_point(0, 2).unwrap().point();
        let seam_screen = screen_point(&app, seam);
        assert!(
            final_wire
                .iter()
                .flatten()
                .any(|p| p.distance(seam_screen) < 0.01),
            "{mode}"
        );

        viewport_click(&mut app, &ui, target);
        assert!(
            app.active_command.is_none(),
            "{mode}: {:?}",
            app.command_log
        );
        assert!(app.paraboloid_session.is_none());
        assert_eq!(app.viewports[0].construction_plane(), plane);
        assert!(viewport_frame(&mut app, &ui, vec![]).is_empty());

        let mut objects = app.document.objects();
        let Geometry::Brep(brep) = objects.next().unwrap().geometry() else {
            panic!("{mode}: expected B-rep");
        };
        assert_eq!(brep.is_solid(), mode != "Vertex");
        for (actual, predicted) in brep.faces()[0]
            .surface()
            .control_points()
            .iter()
            .zip(wall.control_points())
        {
            assert_eq!(actual.weight(), predicted.weight());
            assert!(
                actual.point().distance_to(predicted.point()).unwrap() < 2e-5,
                "{mode}"
            );
        }
        let Geometry::Point(marker) = objects.next().unwrap().geometry() else {
            panic!("{mode}: expected focus marker");
        };
        assert!(
            marker
                .distance_to(expected.focus_marker().unwrap())
                .unwrap()
                < 2e-5,
            "{mode}"
        );
        assert!(objects.next().is_none());
        drop(objects);
        assert!(
            app.document
                .objects()
                .all(|obj| !app.document.is_selected(obj.id()))
        );
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Paraboloid"));
        assert_eq!(app.document.objects().len(), 0);
        assert_eq!(app.document.redo().unwrap().as_deref(), Some("Paraboloid"));
        assert_eq!(app.document.objects().len(), 2);
    }
}

#[test]
fn paraboloid_retries_invalid_picks_and_remembers_flags_after_escape() {
    let mut app = test_app();
    for token in ["Paraboloid _Vertex", "w1,2,3", "_Direction", "w1,2,3"] {
        enter(&mut app, token);
    }
    let input = |app: &VibocerosApp| app.paraboloid_session.as_ref().unwrap().input;
    let first = point(1., 2., 3.);
    assert_eq!(input(&app).markers(), [Some(first), None]);
    assert!(input(&app).prompt().starts_with("Paraboloid direction"));
    for token in ["w1,2,8", "MarkFocus", "Solid=Yes"] {
        enter(&mut app, token);
    }
    let ready = input(&app);
    assert!(ready.prompt().starts_with("Paraboloid end"));
    assert_eq!(ready.anchor(), Some(first));
    for invalid in ["w1,2,10", "w4,6,3", "Solid=Maybe"] {
        enter(&mut app, invalid);
        assert_eq!(input(&app), ready);
        assert_eq!(app.document.objects().len(), 0);
    }
    let reverse = ready.preview(point(4., 6., 1.)).unwrap();
    assert_eq!(reverse.focus_marker(), Some(point(1., 2., -0.125)));
    app.cancel_current_prompt_or_selection();
    assert!(app.paraboloid_session.is_none());
    assert!(!app.document.can_undo());

    enter(&mut app, "Paraboloid");
    assert!(input(&app).prompt().starts_with("Paraboloid focus (Vertex"));
    assert_eq!(
        input(&app).preferences(),
        Preferences {
            mark_focus: true,
            solid: true
        }
    );
    assert_eq!(input(&app).markers(), [None, None]);
    for token in ["w1,2,3", "w1,2,8", "2", "w4,6,5"] {
        enter(&mut app, token);
    }
    let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected a B-rep");
    };
    assert!(brep.is_solid());
    assert!((brep.vertices()[0].point().z() - 2.3713906763541033).abs() < 1e-12);
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn paraboloid_relative_end_uses_the_first_pick_as_confirmed_in_rhino() {
    let mut app = test_app();
    for token in [
        "Paraboloid Vertex MarkFocus=Yes",
        "w1,2,3",
        "Direction",
        "w1,2,8",
        "r3,4,-2",
    ] {
        enter(&mut app, token);
    }
    let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected a B-rep");
    };
    assert_eq!(brep.vertices()[1].point(), point(4., 6., 1.));
    let Geometry::Point(focus) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a focus marker");
    };
    assert_eq!(*focus, point(1., 2., -0.125));
}

#[test]
fn paraboloid_vertex_focus_projects_free_mouse_picks_to_the_cplane_direction() {
    for lock_radius in [false, true] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.maximized_viewport = Some(0);
        app.osnap = false;
        app.grid_snap = false;
        for token in ["Paraboloid Vertex", "w1,2,3", "w3,2,5"] {
            enter(&mut app, token);
        }
        if lock_radius {
            enter(&mut app, "2");
        }
        let ui = egui::Context::default();
        viewport_frame(&mut app, &ui, vec![]);
        viewport_frame(&mut app, &ui, vec![]);
        let target = screen_point(&app, point(5., 5., 0.));
        let wire = viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(target)]);
        let radius = if lock_radius { 2. } else { 3. };
        let axial = radius * radius / 16.;
        let expected = point(1. + axial, 2. + radius, 3. + axial);
        let seam_screen = screen_point(&app, expected);
        assert!(
            wire.iter()
                .flatten()
                .any(|p| p.distance(seam_screen) < 0.01)
        );

        viewport_click(&mut app, &ui, target);
        let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
            panic!("expected a B-rep");
        };
        assert!(brep.vertices()[1].point().distance_to(expected).unwrap() < 2e-5);
    }
}
