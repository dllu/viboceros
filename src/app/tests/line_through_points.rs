use super::*;
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::PointCloud3;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 1.).unwrap()
}

#[test]
fn line_through_points_preselected_mesh_cloud_and_two_grips_execute_directly() {
    for source in 0..3 {
        let mut app = test_app();
        let points = vec![p(0., 0.), p(2., 0.2), p(4., 0.)];
        let geometry = match source {
            0 => Geometry::Mesh(
                TriangleMesh::try_new(points.clone(), vec![[0, 1, 2]], app.document.tolerance())
                    .unwrap(),
            ),
            1 => Geometry::PointCloud(PointCloud3::try_new(vec![points[0], points[2]]).unwrap()),
            _ => Geometry::NurbsCurve(NurbsCurve::try_clamped_uniform(1, points).unwrap()),
        };
        let id = app.document.add_geometry(geometry.clone()).unwrap();
        if source == 2 {
            app.document.enable_control_points([id]).unwrap();
            app.document
                .select_control_points(
                    [0, 2].map(|index| ControlPointId { object: id, index }),
                    SelectionMode::Replace,
                )
                .unwrap();
        } else {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "LineThroughPt");
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(app.document.object(id).unwrap().geometry(), &geometry);
        assert!(matches!(
            app.document.objects().last().unwrap().geometry(),
            Geometry::Line(_)
        ));
        assert_eq!(
            app.document.selected_control_points().count(),
            if source == 2 { 2 } else { 0 }
        );
        assert_eq!(app.document.is_selected(id), source != 2);
    }
}

#[test]
fn line_through_points_pending_viewport_grip_picks_accumulate_only_selected_positions() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(1, vec![p(0., 0.), p(2., 0.2), p(4., 0.)]).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([id]).unwrap();
    enter(&mut app, "LineThroughPt");
    assert!(app.control_point_picking_available());
    for (index, count) in [(0, 1), (2, 2)] {
        let output = crate::viewport::ViewportOutput {
            control_point_selection: Some(crate::viewport::ControlPointSelection {
                picks: vec![ControlPointId { object: id, index }],
                mode: SelectionMode::Replace,
            }),
            ..Default::default()
        };
        assert!(app.handle_viewport_action(output));
        assert_eq!(app.document.selected_control_points().count(), count);
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
        panic!("missing line")
    };
    assert_eq!(line.start(), p(0., 0.));
    assert_eq!(line.end(), p(4., 0.));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn line_through_points_postselection_accepts_a_short_curve_and_one_history_edit() {
    let mut app = test_app();
    for x in [0., 1e-9] {
        app.document
            .add_geometry(Geometry::Point(p(x, 0.)))
            .unwrap();
    }
    enter(&mut app, "LineThroughPt");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("LineThroughPt"));
    let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
        panic!("missing short line")
    };
    assert_eq!(*line.domain().end(), 1e-9);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn line_through_points_cancel_and_coincident_noop_preserve_redo() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![p(1., 2.); 3]).unwrap(),
        ))
        .unwrap();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "Undo");
    let redo = app.document.redo_label().map(str::to_owned);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "LineThroughPt");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    app.document.clear_selection();
    enter(&mut app, "LineThroughPt");
    app.cancel_interactive_command(true);
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}
