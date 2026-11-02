use super::*;
use crate::app::draft_angle_analysis::Source;
use viboceros_geometry::{LineSegment, Vector3};
mod camera_vectors;
mod native;
mod viewport;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn setup() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 2,0,0 2,2,0 0,2,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "SelAll");
    enter(&mut app, "DraftAngleAnalysis Range=-5,7 Up=9,8,7");
    (app, id)
}
fn up(app: &VibocerosApp) -> [f64; 3] {
    app.commands
        .draft_angle_analysis_view(&app.document)
        .unwrap()
        .options
        .up
}
fn close(actual: [f64; 3], expected: [f64; 3]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!((a - b).abs() < 1e-12, "{actual:?} vs {expected:?}");
    }
}

#[test]
fn frame_and_view_direction_controls_preserve_active_getters_and_redo() {
    let (mut app, id) = setup();
    let geometry = app.document.object(id).unwrap().geometry_snapshot().clone();
    let frame = Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_from([2., 3., 6.]).unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    app.viewports[0].plane.set(frame);
    enter(&mut app, "NamedCPlane Save Pull plane");
    let mut alias = app.named_cplanes.get("Pull plane").unwrap().clone();
    alias.name = "Pull  plane".into();
    app.named_cplanes.save(alias.name.clone(), alias).unwrap();
    app.viewports[0].plane.set(
        Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_from([1., 0., 0.]).unwrap(),
            app.document.tolerance(),
        )
        .unwrap(),
    );
    enter(&mut app, "Point 5,5,5");
    enter(&mut app, "Undo");
    enter(&mut app, "Line");
    let active = app.active_command;
    enter(&mut app, "DraftAngleAnalysis Direction=CPlaneZ");
    close(up(&app), [1., 0., 0.]);
    assert_eq!(app.active_command, active);
    enter(
        &mut app,
        "_DraftAngleAnalysis  _Direction=NamedCPlane \"Pull  plane\"",
    );
    close(up(&app), [2. / 7., 3. / 7., 6. / 7.]);
    assert_eq!(app.active_command, active);
    app.viewports[0].set_world_view(ViewKind::Front).unwrap();
    enter(&mut app, "DraftAngleAnalysis Direction=View");
    close(up(&app), [0., -1., 0.]);
    assert_eq!(app.active_command, active);
    enter(&mut app, "_DraftAngleAnalysis _Direction=WorldZ");
    close(up(&app), [0., 0., 1.]);
    assert_eq!(app.active_command, active);
    let before = up(&app);
    enter(&mut app, "DraftAngleAnalysis Direction=NamedCPlane missing");
    enter(&mut app, "DraftAngleAnalysis Direction=Bad");
    assert_eq!(up(&app), before);
    assert_eq!(app.active_command, active);
    app.cancel_interactive_command(false);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 2);
    assert!(geometry.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    assert_eq!(
        app.commands
            .draft_angle_analysis_view(&app.document)
            .unwrap()
            .options
            .range,
        [-5., 7.]
    );
}

#[test]
fn two_points_retry_and_cancel_preserve_analysis_and_source_geometry() {
    let (mut app, id) = setup();
    let before = up(&app);
    let source = app.document.object(id).unwrap().geometry_snapshot().clone();
    enter(&mut app, "DraftAngleAnalysis Direction=TwoPoints");
    enter(&mut app, "1,2,3");
    enter(&mut app, "1,2,3");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::DraftPullPoints { start: Some(_) })
    ));
    assert_eq!(up(&app), before);
    enter(&mut app, "3,5,9");
    close(up(&app), [2., 3., 6.]);
    assert!(app.active_command.is_none());
    enter(&mut app, "DraftAngleAnalysis Direction=TwoPoints");
    enter(&mut app, "9,8,7");
    enter(&mut app, "Cancel");
    close(up(&app), [2., 3., 6.]);
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert!(app.commands.object_has_draft_angle_analysis(id));
    assert!(source.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
}

#[test]
fn curve_picker_uses_the_start_tangent_and_rejects_missing_or_hidden_references() {
    let (mut app, _) = setup();
    let curve = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0., 0., 0.),
                point(2., 3., 6.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let source = app
        .document
        .object(curve)
        .unwrap()
        .geometry_snapshot()
        .clone();
    enter(&mut app, "DraftAngleAnalysis Direction=Curve");
    assert!(app.picking_draft_pull_curve());
    let prior = up(&app);
    app.pick_draft_pull_curve(None, None);
    assert!(app.picking_draft_pull_curve());
    assert_eq!(up(&app), prior);
    app.apply_selection_click(SelectionClick {
        object_id: Some(curve),
        mode: SelectionMode::Replace,
    });
    close(up(&app), [2. / 7., 3. / 7., 6. / 7.]);
    assert!(app.active_command.is_none());
    assert!(!app.document.is_selected(curve));
    assert!(source.shares_storage_with(app.document.object(curve).unwrap().geometry_snapshot()));
    app.document
        .select_objects_direct([curve], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Hide");
    enter(&mut app, "DraftAngleAnalysis Direction=Curve");
    app.pick_draft_pull_curve(Some(curve), None);
    assert!(app.picking_draft_pull_curve());
    close(up(&app), [2. / 7., 3. / 7., 6. / 7.]);
    app.cancel_interactive_command(false);
    app.set_draft_pull_source(Source::CPlaneZ);
}
