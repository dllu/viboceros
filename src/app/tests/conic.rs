use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn controls(app: &VibocerosApp) -> Vec<Point3> {
    let Geometry::NurbsCurve(c) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    c.control_points().iter().map(|p| p.point()).collect()
}
#[test]
fn conic_getters_preview_shape_point_defaults_cancel_and_history() {
    let mut app = test_app();
    enter(&mut app, "Conic");
    for text in ["0,0,0", "10,0,0", "4,6,0"] {
        enter(&mut app, text);
    }
    let before = format!("{:?}", app.document);
    let i = app.conic_session.as_ref().unwrap().input;
    assert!(i.preview(point(4., 2., 0.)).is_some());
    assert_eq!(before, format!("{:?}", app.document));
    enter(&mut app, "");
    assert!(app.conic_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "Conic");
    for text in ["0,0,0", "10,0,0", "4,6,0", "0", "1"] {
        enter(&mut app, text);
    }
    assert!(app.conic_session.is_some());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "3/4");
    assert!(app.conic_session.is_none());
    assert_eq!(app.commands.conic_options().rho, Some(0.75));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Conic"));
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    enter(&mut app, "Conic");
    for text in ["1,2,3", "Apex", "4,6,8", "Vertical", "11,9,4", ""] {
        enter(&mut app, text);
    }
    assert_eq!(
        controls(&app),
        [point(1., 2., 3.), point(4., 6., 8.), point(4., 6., 4.)]
    );
    enter(&mut app, "Conic");
    enter(&mut app, "0,0,0");
    let before = format!("{:?}", app.document);
    enter(&mut app, "Cancel");
    assert_eq!(before, format!("{:?}", app.document));
}
#[test]
fn conic_curve_subcurve_and_stale_capture_keep_sources_unchanged() {
    let mut app = test_app();
    let guide = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(0., 0., 0.),
                point(10., 0., 0.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let original = app.document.object(guide).unwrap().geometry().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Conic Tangent");
    assert!(app.picking_conic_curve());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    enter(&mut app, "SubCrv");
    app.pick_conic_curve(Some(guide), None);
    enter(&mut app, "2,0,0");
    enter(&mut app, "6,0,0");
    enter(&mut app, "9,2,0");
    assert_eq!(app.last_point, Some(point(6., 0., 0.)));
    for text in ["10,8,0", "5,3,0", "0.5"] {
        enter(&mut app, text);
    }
    assert_eq!(
        controls(&app),
        [point(6., 0., 0.), point(5., 0., 0.), point(10., 8., 0.)]
    );
    assert_eq!(app.document.object(guide).unwrap().geometry(), &original);
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Conic"));
    assert!(app.document.undo().unwrap().is_none());
    enter(&mut app, "Conic Perpendicular");
    app.pick_conic_curve(Some(guide), None);
    app.document.set_objects_visibility([guide], false).unwrap();
    app.validate_conic_curve();
    assert!(app.picking_conic_curve());
    assert!(!app.accept_drafting_point(point(2., 0., 0.)));
    enter(&mut app, "Cancel");
}
#[test]
fn conic_surface_edge_component_getter_and_failed_shape_leave_no_helpers() {
    let mut app = test_app();
    let surface = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                point(0., 0., 0.),
                point(10., 0., 0.),
                point(10., 10., 0.),
                point(0., 10., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let original = app.document.object(surface).unwrap().geometry().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Conic Tangent");
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: surface,
            index: 0,
            kind: viboceros_command::ComponentSelectionKind::BrepEdge,
        }],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    assert!(!app.picking_conic_curve());
    for text in ["2,0,0", "10,8,0", "5,3,0", "0.5"] {
        enter(&mut app, text);
    }
    assert_eq!(app.document.object(surface).unwrap().geometry(), &original);
    assert_eq!(app.document.objects().count(), 2);
    enter(&mut app, "Conic");
    for text in ["0,0,0", "10,0,0", "4,6,0", "-2,2,0"] {
        enter(&mut app, text);
    }
    assert!(app.conic_session.is_none());
    assert_eq!(app.document.objects().count(), 2);
}

#[test]
fn conic_reference_picker_yields_to_a_new_command() {
    let mut app = test_app();
    enter(&mut app, "Conic Tangent");
    enter(&mut app, "Point 1,2,3");
    assert!(app.conic_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.undo_label(), Some("Point"));
}

fn conic_line_guide(app: &mut VibocerosApp) -> ObjectId {
    let id = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(0., 0., 0.),
                point(10., 0., 0.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    id
}
#[test]
fn conic_locked_subcurve_preview_is_cached_and_opposite_side_returns_to_curve_selection() {
    let mut app = test_app();
    let id = conic_line_guide(&mut app);
    let original = app.document.object(id).unwrap().geometry().clone();
    enter(&mut app, "Conic Tangent");
    enter(&mut app, "SubCrv");
    app.pick_conic_curve(Some(id), None);
    enter(&mut app, "6,0,0");
    enter(&mut app, "D");
    assert_eq!(
        app.conic_session
            .as_ref()
            .unwrap()
            .reference
            .as_ref()
            .unwrap()
            .locked_forward(),
        None
    );
    app.update_subcurve_hover(point(8., 0., 0.));
    enter(&mut app, "D");
    let before = format!("{:?}", app.document);
    let first = app.subcurve_draft_preview().unwrap().geometry;
    let second = app.subcurve_draft_preview().unwrap().geometry;
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(first.endpoints, [point(6., 0., 0.), point(8., 0., 0.)]);
    assert_eq!(format!("{:?}", app.document), before);
    app.update_subcurve_hover(point(2., 0., 0.));
    assert!(app.subcurve_draft_preview().is_none());
    enter(&mut app, "2,0,0");
    assert!(app.picking_conic_curve());
    assert!(app.conic_session.as_ref().unwrap().reference.is_none());
    assert!(app.subcurve_draft_preview().is_none());
    app.pick_conic_curve(Some(id), None);
    for text in ["9,1,0", "10,8,0", "5,3,0", "0.5"] {
        enter(&mut app, text);
    }
    assert_eq!(
        controls(&app),
        [point(9., 0., 0.), point(5., 0., 0.), point(10., 8., 0.)]
    );
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Conic"));
    assert!(app.document.undo().unwrap().is_none());
}
#[test]
fn conic_locked_numeric_uses_live_hover_and_requires_no_extra_trim_point() {
    let mut app = test_app();
    let id = conic_line_guide(&mut app);
    enter(&mut app, "Conic Tangent");
    enter(&mut app, "SubCrv");
    app.pick_conic_curve(Some(id), None);
    enter(&mut app, "6,0,0");
    app.update_subcurve_hover(point(8., 0., 0.));
    enter(&mut app, "Direction=Locked");
    enter(&mut app, "3mm");
    assert_eq!(
        app.conic_session
            .as_ref()
            .unwrap()
            .reference
            .as_ref()
            .unwrap()
            .stage(),
        viboceros_command::conic::ReferenceStage::Endpoint
    );
    assert!(app.subcurve_draft_preview().is_none());
    for text in ["10,0,0", "10,8,0", "5,3,0", "0.5"] {
        enter(&mut app, text);
    }
    assert_eq!(
        controls(&app),
        [point(9., 0., 0.), point(5., 0., 0.), point(10., 8., 0.)]
    );
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Conic"));
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
}
#[test]
fn conic_nested_enter_and_zero_return_without_losing_the_first_endpoint() {
    for choice in ["", "0"] {
        let mut app = test_app();
        let id = conic_line_guide(&mut app);
        enter(&mut app, "Conic");
        enter(&mut app, "0,8,0");
        enter(&mut app, "Tangent");
        enter(&mut app, "SubCrv");
        app.pick_conic_curve(Some(id), None);
        enter(&mut app, "6,0,0");
        let before = format!("{:?}", app.document);
        enter(&mut app, choice);
        assert!(app.picking_conic_curve());
        assert_eq!(format!("{:?}", app.document), before);
        app.pick_conic_curve(Some(id), None);
        for text in ["9,1,0", "5,3,0", "0.5"] {
            enter(&mut app, text);
        }
        assert_eq!(
            controls(&app),
            [point(0., 8., 0.), point(5., 0., 0.), point(9., 0., 0.)]
        );
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Conic"));
        assert!(app.document.undo().unwrap().is_none());
    }
}
#[test]
fn conic_closed_numeric_waits_for_confirmation_and_hides_stale_preview_immediately() {
    let mut app = test_app();
    let plane = app.viewports[app.active_viewport].construction_plane();
    let id = app
        .document
        .add_geometry(Geometry::Circle(
            viboceros_geometry::Circle3::try_from_frame(
                point(0., 0., 0.),
                4.,
                plane.x_axis(),
                plane.z_axis(),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Conic Tangent");
    enter(&mut app, "SubCrv");
    app.pick_conic_curve(Some(id), None);
    enter(&mut app, "0,4,0");
    app.update_subcurve_hover(point(-4., 0., 0.));
    enter(&mut app, "D");
    enter(&mut app, "2");
    assert_eq!(
        app.conic_session
            .as_ref()
            .unwrap()
            .reference
            .as_ref()
            .unwrap()
            .stage(),
        viboceros_command::conic::ReferenceStage::TrimEnd
    );
    let preview = app.subcurve_draft_preview().unwrap().geometry;
    assert!((preview.curve.length(app.document.tolerance()).unwrap() - 2.).abs() < 1e-8);
    app.document.set_objects_visibility([id], false).unwrap();
    assert!(app.subcurve_draft_preview().is_none());
    app.validate_conic_curve();
    assert!(app.picking_conic_curve());
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().count(), 1);
}
