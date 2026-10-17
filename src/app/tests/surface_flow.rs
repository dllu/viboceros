use super::*;
use viboceros_geometry::PointMorph;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, ObjectId, [ObjectId; 2]) {
    let mut app = test_app();
    let surface = |x: f64, z: f64| {
        Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear(
                [
                    [x, 0., 0.],
                    [x + 10., 0., 0.],
                    [x + 10., 10., z],
                    [x, 10., 0.],
                ]
                .map(|p| Point3::try_from(p).unwrap()),
            )
            .unwrap(),
        )
    };
    let guides = [surface(0., 0.), surface(20., 5.)].map(|g| app.document.add_geometry(g).unwrap());
    let source = app
        .document
        .add_geometry(Geometry::PointCloud(
            viboceros_geometry::PointCloud3::try_new(vec![
                Point3::try_new(2., 3., 1.).unwrap(),
                Point3::try_new(12., 13., -1.).unwrap(),
            ])
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, source, guides)
}
#[test]
fn surface_flow_picks_preview_cancel_and_single_history_edit() {
    let (mut app, source, [base, target]) = setup();
    let before = format!("{:?}", app.document);
    enter(&mut app, "FlowAlongSrf Copy=No");
    assert!(app.picking_surface_flow());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::SurfaceComponents)
    );
    app.pick_surface_flow(Some(source), None, None);
    assert!(app.picking_surface_flow());
    app.accept_component_face_hit(base, 0, Some(Point3::try_new(0., 1., 0.).unwrap()));
    app.accept_component_face_hit(target, 0, Some(Point3::try_new(21., 0., 0.).unwrap()));
    let preview = app
        .surface_flow_session
        .as_ref()
        .unwrap()
        .preview()
        .unwrap();
    assert!(preview.options.base_frame.swap_uv);
    assert!(!preview.options.target_frame.swap_uv);
    let original = app.document.object(source).unwrap().geometry().clone();
    let expected = original
        .morphed(&**preview.morph, app.document.tolerance())
        .unwrap();
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "");
    assert!(app.surface_flow_session.is_none());
    assert_eq!(app.document.object(source).unwrap().geometry(), &expected);
    assert_eq!(
        app.document.undo().unwrap().as_deref(),
        Some("FlowAlongSrf")
    );
    assert_eq!(app.document.object(source).unwrap().geometry(), &original);
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    assert_eq!(app.document.object(source).unwrap().geometry(), &expected);
    enter(&mut app, "FlowAlongSurface");
    app.pick_surface_flow(Some(base), None, None);
    app.pick_surface_flow(Some(target), None, None);
    let before = format!("{:?}", app.document);
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
}
#[test]
fn surface_flow_plane_and_normal_getters_leave_guides_out_of_the_model() {
    let (mut app, _, [_, target]) = setup();
    let count = app.document.objects().count();
    enter(&mut app, "FlowAlongSrf Plane Copy=No ConstrainNormal=Yes");
    assert!(app.drawing_surface_flow());
    assert_eq!(
        app.accept_surface_flow_point(Point3::try_new(10., 10., 0.).unwrap()),
        Some(true)
    );
    assert_eq!(
        app.accept_surface_flow_point(Point3::try_new(0., 0., 0.).unwrap()),
        Some(true)
    );
    assert!(app.picking_surface_flow());
    app.pick_surface_flow(Some(target), None, None);
    assert!(app.drawing_surface_flow());
    enter(&mut app, "");
    let preview = app
        .surface_flow_session
        .as_ref()
        .unwrap()
        .preview()
        .unwrap();
    assert_eq!(
        preview.options.normal,
        Some(viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap())
    );
    let p = preview
        .morph
        .morph_point(Point3::try_new(-2., -1., 0.).unwrap())
        .unwrap();
    assert!(p.to_array().iter().all(|v| v.is_finite()));
    assert_eq!(app.document.objects().count(), count);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "");
    assert_eq!(app.document.objects().count(), count);
}
#[test]
fn surface_flow_stale_reference_restarts_getter_and_explicit_frames_override_pick() {
    let (mut app, _, [base, target]) = setup();
    enter(&mut app, "FlowAlongSrf");
    app.pick_surface_flow(Some(base), None, Some(Point3::try_new(0., 1., 0.).unwrap()));
    app.pick_surface_flow(Some(target), None, None);
    enter(&mut app, "BaseCorner=2 BaseSwap=No");
    let preview = app
        .surface_flow_session
        .as_ref()
        .unwrap()
        .preview()
        .unwrap();
    assert!(!preview.options.base_frame.swap_uv);
    assert!(preview.options.base_frame.reverse_u && preview.options.base_frame.reverse_v);
    app.document
        .set_objects_visibility([target], false)
        .unwrap();
    app.validate_surface_flow();
    assert!(app.picking_surface_flow());
    assert!(
        app.surface_flow_session
            .as_ref()
            .unwrap()
            .preview()
            .is_none()
    );
}

#[test]
fn surface_flow_command_first_sources_and_target_view_normal_are_retained() {
    let (mut app, source, [base, target]) = setup();
    app.document.clear_selection();
    enter(&mut app, "FlowAlongSrf Copy=No ConstrainNormal=Yes");
    assert!(app.object_prompt.is_some());
    enter(&mut app, &format!("SelID {source}"));
    enter(&mut app, "");
    assert!(app.picking_surface_flow());
    app.pick_surface_flow(Some(base), None, None);
    let plane = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(plane);
    app.pick_surface_flow(Some(target), None, None);
    app.active_viewport = (app.active_viewport + 1) % app.viewports.len();
    enter(&mut app, "");
    let preview = app
        .surface_flow_session
        .as_ref()
        .unwrap()
        .preview()
        .unwrap();
    assert_eq!(preview.options.normal, Some(plane.z_axis().as_vector()));
    enter(&mut app, "");
    assert!(app.surface_flow_session.is_none());
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.undo_label(), Some("FlowAlongSrf"));
}

#[test]
fn surface_flow_group_origin_options_change_preview_and_survive_one_commit() {
    use viboceros_command::surface_flow::GroupFrameOrigin;
    let (mut app, source, [base, target]) = setup();
    let peer = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(18., 7., 3.).unwrap()))
        .unwrap();
    app.document.add_group(None, [source, peer]).unwrap();
    app.document
        .select_objects_direct([source, peer], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "FlowAlongSrf Copy=No Rigid=Yes AutoAdjust=No");
    app.pick_surface_flow(Some(base), None, None);
    app.pick_surface_flow(Some(target), None, None);
    enter(&mut app, "GroupFrameOrigin");
    let p = app
        .surface_flow_session
        .as_ref()
        .unwrap()
        .preview()
        .unwrap();
    assert_eq!(
        p.options.group_frame_origin,
        GroupFrameOrigin::ObjectFrameAverage
    );
    let expected = viboceros_command::surface_flow::deformed_with_grips(
        &app.document,
        p.sources,
        p.grips,
        p.morph,
        p.options,
    )
    .unwrap();
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "");
    assert!(app.surface_flow_session.is_none());
    for (id, g) in expected {
        assert_eq!(app.document.object(id).unwrap().geometry(), &g);
    }
    assert_eq!(
        app.commands.surface_flow_options().group_frame_origin,
        GroupFrameOrigin::ObjectFrameAverage
    );
    assert_eq!(
        app.document.undo().unwrap().as_deref(),
        Some("FlowAlongSrf")
    );
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    enter(&mut app, "FlowAlongSurface");
    assert!(app.picking_surface_flow());
    app.pick_surface_flow(Some(base), None, None);
    app.pick_surface_flow(Some(target), None, None);
    assert_eq!(
        app.surface_flow_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap()
            .options
            .group_frame_origin,
        GroupFrameOrigin::ObjectFrameAverage
    );
    enter(&mut app, "GroupFrameOrigin=BoundingBoxCenter");
    assert_eq!(
        app.surface_flow_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap()
            .options
            .group_frame_origin,
        GroupFrameOrigin::BoundingBoxCenter
    );
    enter(&mut app, "Cancel");
}
