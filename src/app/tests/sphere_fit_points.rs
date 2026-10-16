use super::*;
use viboceros_document::{ControlPointId, SelectionMode};
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}
fn points() -> Vec<Point3> {
    [[1., 1., 1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.]]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec()
}
#[test]
fn sphere_fit_points_incremental_option_selects_points_and_records_one_edit() {
    let mut app = test_app();
    for p in points() {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    enter(&mut app, "Sphere");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Sphere { center: None })
    );
    enter(&mut app, "_FitPoints");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.command,
        "Sphere FitPoints"
    );
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("Sphere"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 4);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 5);
}
#[test]
fn sphere_fit_points_grip_picks_accumulate_without_consuming_the_parent() {
    let mut app = test_app();
    let source = Geometry::NurbsCurve(NurbsCurve::try_clamped_uniform(1, points()).unwrap());
    let id = app.document.add_geometry(source.clone()).unwrap();
    app.document.enable_control_points([id]).unwrap();
    enter(&mut app, "Sphere FitPoints");
    assert!(app.control_point_picking_available());
    for index in 0..4 {
        app.handle_viewport_action(crate::viewport::ViewportOutput {
            control_point_selection: Some(crate::viewport::ControlPointSelection {
                picks: vec![ControlPointId { object: id, index }],
                mode: SelectionMode::Replace,
            }),
            ..Default::default()
        });
    }
    assert_eq!(app.document.selected_control_points().count(), 4);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.object(id).unwrap().geometry(), &source);
}
#[test]
fn sphere_fit_points_rejects_whole_mesh_selection_and_cancellation_preserves_redo() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(points(), vec![[0, 1, 2]], app.document.tolerance()).unwrap(),
        ))
        .unwrap();
    enter(&mut app, "Sphere 0,0,0 2");
    enter(&mut app, "Undo");
    let redo = app.document.redo_label().map(str::to_owned);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Sphere FitPoints");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.selected_object_count(), 0);
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}
