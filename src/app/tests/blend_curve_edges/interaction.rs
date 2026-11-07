use super::*;

#[test]
fn surface_mode_restricts_sources_and_preserves_cancelled_candidates() {
    use viboceros_command::blend_curve::Handle;
    let data = native::read("blend_curve_edge_pilot");
    let value = &data["results"][0]["value"];
    let mut app = test_app();
    app.document = edges::setup(value);
    let parent = reference_inputs::guide(&app.document, 0);
    let line = reference_inputs::guide(&app.document, 1);
    enter(&mut app, "BlendCrv Edges");
    assert!(app.picking_blend_surface_edge());
    app.pick_blend_curve_at(line, None, Point3::try_new(10., 3., 0.).unwrap());
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .reference_count(),
        0
    );
    app.pick_blend_curve_at(parent, Some(2), Point3::try_new(0., -2.5, 0.).unwrap());
    enter(&mut app, "Curves");
    assert!(!app.picking_blend_surface_edge());
    app.pick_blend_curve_at(line, None, Point3::try_new(10., 3., 0.).unwrap());
    enter(&mut app, "AtAngle_1=30");
    let preview = app
        .blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .cloned()
        .unwrap();
    let tangent = preview
        .evaluate_with_derivative(*preview.domain().start())
        .unwrap()
        .1
        .normalized_nonzero()
        .unwrap()
        .as_vector();
    assert!((tangent.x() - 30_f64.to_radians().cos()).abs() < 1e-12);
    assert!((tangent.y() + 0.5).abs() < 1e-12);
    app.select_blend_curve_handle_with_symmetry(Handle::StartShape, true);
    app.hover_blend_curve_parameter(4.);
    assert_ne!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .preview_input()
            .preview(),
        Some(&preview)
    );
    assert!(!app.document.can_undo());
    enter(&mut app, "Cancel");
    assert_eq!(
        app.blend_curve_session.as_ref().unwrap().input.preview(),
        Some(&preview)
    );
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .paired_handle(),
        None
    );
    let geometry = app
        .document
        .object(parent)
        .unwrap()
        .geometry()
        .transformed(
            viboceros_geometry::AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
            ),
            app.document.tolerance(),
        )
        .unwrap();
    app.document
        .replace_object_geometries([(parent, geometry)])
        .unwrap();
    app.document.clear_undo_records().unwrap();
    app.validate_blend_curve();
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .reference_count(),
        0
    );
    assert!(!app.picking_blend_surface_edge());
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.can_undo());
}
