use super::plane_surface_native_support as native;
use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn plane_native_workflows_use_actual_getters_and_atomic_surface_history() {
    let capture = native::read("plane_surface_terminal");
    let guides = native::read("plane_surface_guides");
    let vertical = native::read("plane_surface_vertical_audit");
    assert_eq!(capture["results"].as_array().unwrap().len(), 51);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plane.3dm");
    let mut archived = 0;
    assert_eq!(guides["results"].as_array().unwrap().len(), 5);
    assert_eq!(vertical["results"].as_array().unwrap().len(), 7);
    for row in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .chain(guides["results"].as_array().unwrap())
        .chain(vertical["results"].as_array().unwrap())
    {
        let v = &row["value"];
        let mut app = test_app();
        app.document = native::setup(v);
        let parents = app.document.objects().cloned().collect::<Vec<_>>();
        let frame = native::frame(v);
        app.viewports[0].set_construction_plane(frame);
        let tokens = native::tokens(&app.document, v);
        enter(&mut app, "Plane");
        assert!(app.plane_surface_session.is_some(), "{:?}", app.command_log);
        for word in tokens {
            enter(&mut app, if word == "Enter" { "" } else { &word });
        }
        assert!(
            app.plane_surface_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        assert!(app.active_command.is_none());
        assert!(app.drafting_plane.is_none());
        assert_eq!(app.viewports[0].construction_plane(), frame);
        let mut error = native::check(&app.document, v, "after");
        let mut archive_objects = None;
        if v["success"] == true {
            assert_eq!(app.document.undo_label(), Some("Plane"));
            app.document.undo().unwrap();
            error = error.max(native::check(&app.document, v, "undo"));
            assert!(app.document.undo().unwrap().is_none());
            app.document.redo().unwrap();
            error = error.max(native::check(&app.document, v, "redo"));
        } else {
            assert!(!app.document.can_undo());
        }
        for p in parents {
            assert_eq!(app.document.object(p.id()), Some(&p));
            assert!(
                p.geometry_snapshot()
                    .shares_storage_with(app.document.object(p.id()).unwrap().geometry_snapshot())
            );
        }
        if v["success"] == true {
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(native::check_archive(&restored, v));
            archive_objects = Some(restored.objects().len());
            archived += 1;
        }
        println!(
            "PLANE_UI {}",
            serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error,"archive_objects":archive_objects})
        );
        assert!(error <= 1e-6, "{} {error:e}", row["id"]);
    }
    assert_eq!(archived, 53);
}

#[test]
fn plane_options_reject_invalid_values_without_consuming_points_and_cancel_keeps_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "Plane Deformable UDegree");
    assert!(app.plane_surface_setting_pending());
    for invalid in ["1", "12", "nan", "1000000000000000000000000"] {
        enter(&mut app, invalid);
        assert!(app.plane_surface_setting_pending());
    }
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    enter(&mut app, "2");
    assert!(!app.plane_surface_setting_pending());
    enter(&mut app, "w1,2,3");
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.can_redo());
    assert_eq!(app.commands.plane_preferences().degrees, [2, 3]);
}

#[test]
fn plane_drafts_latch_the_initial_viewport_and_invalidate_on_document_changes() {
    let mut app = test_app();
    enter(&mut app, "Plane");
    app.active_viewport = 2;
    enter(&mut app, "0");
    app.active_viewport = 0;
    enter(&mut app, "w4,8,5");
    let Geometry::NurbsSurface(s) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(s.control_points().iter().all(|p| p.point().y() == 0.));
    assert_eq!(*s.domain_u().end(), 4.);
    assert_eq!(*s.domain_v().end(), 5.);
    enter(&mut app, "Plane");
    enter(&mut app, "w0,0,0");
    app.document
        .add_geometry(Geometry::Point(point(10., 20., 30.)))
        .unwrap();
    assert!(!app.accept_drafting_point(point(3., 4., 0.)));
    assert!(app.plane_surface_session.is_none());
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn plane_guide_picker_clears_selection_without_consuming_pending_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    let source = app.document.objects().next().unwrap().id();
    enter(&mut app, "Line 0,0,0 0,0,10");
    app.document.set_transient_selection([source]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 8,9,10");
    app.document.undo().unwrap();
    let parents = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Plane AroundCurve");
    assert!(app.picking_rectangle_curve());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Cancel");
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    for parent in parents {
        assert_eq!(app.document.object(parent.id()), Some(&parent));
    }
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn plane_enter_cancels_point_prompts_and_retains_pending_redo() {
    for steps in [
        vec!["Plane", ""],
        vec!["Plane", "w1,2,3", "Enter"],
        vec!["Plane Vertical", "w1,2,3", "w4,6,8", "Enter", "Enter"],
    ] {
        let mut app = test_app();
        enter(&mut app, "Point 8,9,10");
        app.document.undo().unwrap();
        let before = format!("{:?}", app.document);
        for step in steps {
            enter(&mut app, step);
        }
        assert!(app.plane_surface_session.is_none(), "{:?}", app.command_log);
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.document.can_redo());
    }
}
