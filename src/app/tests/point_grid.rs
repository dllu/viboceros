use super::*;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}
fn cloud(document: &Document) -> Vec<Point3> {
    let Geometry::PointCloud(cloud) = document.objects().last().unwrap().geometry() else {
        panic!("expected grid")
    };
    cloud.points().to_vec()
}

#[test]
fn picked_grid_matches_typed_command_and_is_one_undo_step() {
    let mut app = test_app();
    enter(&mut app, "PointGrid XCount 3 YCount=2 ZCount 3");
    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(6.0, 4.0, 7.0)));
    assert!(app.accept_drafting_point(point(20.0, 30.0, -8.0)));
    assert!(app.active_command.is_none());
    let mut expected = Document::default();
    CommandRegistry::with_builtins()
        .execute(
            &mut expected,
            "PointGrid 0,0,0 6,4,7 -8 XCount=3 YCount=2 ZCount=3",
        )
        .unwrap();
    assert_eq!(cloud(&app.document), cloud(&expected));
    assert_eq!(app.document.undo_label(), Some("PointGrid"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "Redo");
    assert_eq!(cloud(&app.document), cloud(&expected));
}

#[test]
fn scalar_and_default_height_finish_the_same_draft() {
    for height in ["-8", ""] {
        let mut app = test_app();
        for input in [
            "PointGrid XCount=3 YCount=2 ZCount=3",
            "w0,0,0",
            "w6,4,0",
            height,
        ] {
            enter(&mut app, input);
        }
        assert!(app.active_command.is_none());
        assert!(app.command_input.is_empty());
        let mut expected = Document::default();
        CommandRegistry::with_builtins()
            .execute(
                &mut expected,
                &format!("PointGrid 0,0,0 6,4,0 {height} XCount=3 YCount=2 ZCount=3"),
            )
            .unwrap();
        assert_eq!(cloud(&app.document), cloud(&expected));
    }
}

#[test]
fn grid_uses_first_pick_plane_after_changing_viewports() {
    let mut app = test_app();
    app.active_viewport = 2;
    let plane = app.viewports[2].construction_plane();
    enter(&mut app, "PointGrid XCount=3 YCount=2 ZCount=2");
    assert!(app.accept_drafting_point(point(10.0, 20.0, 30.0)));
    app.active_viewport = 0;
    assert!(app.accept_drafting_point(point(16.0, 99.0, 34.0)));
    assert!(app.accept_drafting_point(point(99.0, 28.0, 99.0)));
    let mut expected = Document::default();
    CommandRegistry::with_builtins()
        .execute_in_context(
            &mut expected,
            "PointGrid 10,20,30 16,99,34 -8 XCount=3 YCount=2 ZCount=2",
            viboceros_command::CommandContext {
                construction_plane: plane,
            },
        )
        .unwrap();
    assert_eq!(cloud(&app.document), cloud(&expected));
}

#[test]
fn rejected_points_and_heights_preserve_draft_and_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "Undo");
    enter(&mut app, "PointGrid XCount=3 YCount=2 ZCount=2");
    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    let first = app.active_command;
    assert!(!app.accept_drafting_point(point(0.0, 4.0, 0.0)));
    assert_eq!(app.active_command, first);
    assert!(app.accept_drafting_point(point(6.0, 4.0, 0.0)));
    let draft = app.active_command;
    let frame = app.drafting_plane;
    let last_point = app.last_point;
    for input in ["0", "NaN", "inf"] {
        enter(&mut app, input);
        assert_eq!(app.active_command, draft);
        assert_eq!(app.drafting_plane, frame);
        assert_eq!(app.last_point, last_point);
        assert_eq!(app.document.objects().len(), 0);
        assert_eq!(app.document.redo_label(), Some("Point"));
    }
    assert!(!app.accept_drafting_point(point(1.0, 2.0, 0.0)));
    assert_eq!(app.active_command, draft);
    enter(&mut app, "3");
    assert!(app.active_command.is_none());
    assert_eq!(cloud(&app.document).len(), 12);
}

#[test]
fn cancellation_does_not_change_remembered_counts() {
    let mut app = test_app();
    enter(
        &mut app,
        "PointGrid 0,0,0 6,4,0 1 XCount=3 YCount=2 ZCount=1",
    );
    enter(&mut app, "Clear");
    enter(&mut app, "PointGrid XCount=20 YCount=30 ZCount=2");
    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    app.cancel_interactive_command(true);
    assert!(app.drafting_plane.is_none());
    for input in ["PointGrid", "0", "6,4", ""] {
        enter(&mut app, input);
    }
    assert_eq!(cloud(&app.document).len(), 6);
}

#[test]
fn tiny_valid_grids_do_not_acquire_a_ui_only_tolerance_threshold() {
    let mut app = test_app();
    for input in [
        "PointGrid XCount=2 YCount=2 ZCount=2",
        "0",
        "1e-12,2e-12",
        "3e-12",
    ] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    assert_eq!(cloud(&app.document).len(), 8);
}

#[test]
fn remembered_count_budget_failure_does_not_discard_picked_corners() {
    let mut app = test_app();
    // X alone is within the minimum possible grid budget, but the remembered
    // default Y count of ten makes the complete request exceed one million.
    for input in ["PointGrid XCount=200000", "0", "6,4"] {
        enter(&mut app, input);
    }
    let draft = app.active_command;
    let plane = app.drafting_plane;
    enter(&mut app, "2");
    assert!(matches!(
        draft,
        Some(InteractiveCommand::PointGrid {
            opposite: Some(_),
            ..
        })
    ));
    assert_eq!(app.active_command, draft);
    assert_eq!(app.drafting_plane, plane);
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.command_log.back().unwrap().contains("1000000"));
    app.cancel_interactive_command(true);
    assert!(app.active_command.is_none());
}
