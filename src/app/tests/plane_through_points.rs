use super::*;
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::PointCloud3;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn points() -> Vec<Point3> {
    [(0., 0.), (4., 0.), (0., 3.), (4., 3.)]
        .map(|(x, y)| Point3::try_new(x, y, 2.).unwrap())
        .to_vec()
}

#[test]
fn plane_through_points_postselection_filters_sources_and_accepts_one_history_edit() {
    let mut app = test_app();
    app.document.begin_transaction("sources").unwrap();
    let ids = points()
        .into_iter()
        .map(|p| app.document.add_geometry(Geometry::Point(p)).unwrap())
        .collect::<Vec<_>>();
    app.document.commit_transaction().unwrap();
    enter(&mut app, "PlaneThroughPt");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.filter,
        viboceros_command::ObjectSelectionFilter::PlaneFitPoints
    );
    enter(&mut app, "SelAll");
    assert_eq!(app.document.selected_object_count(), 4);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("PlaneThroughPt"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().map(|o| o.id()).collect::<Vec<_>>(),
        ids
    );
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 5);
}

#[test]
fn plane_through_points_preselected_cloud_and_grips_execute_without_reselecting() {
    for grips in [false, true] {
        let mut app = test_app();
        let geometry = if grips {
            Geometry::NurbsCurve(NurbsCurve::try_clamped_uniform(1, points()).unwrap())
        } else {
            Geometry::PointCloud(PointCloud3::try_new(points()).unwrap())
        };
        let id = app.document.add_geometry(geometry.clone()).unwrap();
        if grips {
            app.document.enable_control_points([id]).unwrap();
            app.document
                .select_control_points(
                    (0..4).map(|index| ControlPointId { object: id, index }),
                    SelectionMode::Replace,
                )
                .unwrap();
        } else {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "PlaneThroughPt");
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(app.document.object(id).unwrap().geometry(), &geometry);
        assert_eq!(
            app.document.selected_control_points().count(),
            if grips { 4 } else { 0 }
        );
        assert_eq!(app.document.is_selected(id), !grips);
    }
}

#[test]
fn plane_through_points_viewport_grip_picks_accumulate_until_enter() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(1, points()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([id]).unwrap();
    enter(&mut app, "PlaneThroughPt");
    assert!(app.control_point_picking_available());
    for index in 0..3 {
        let output = crate::viewport::ViewportOutput {
            control_point_selection: Some(crate::viewport::ControlPointSelection {
                picks: vec![ControlPointId { object: id, index }],
                mode: SelectionMode::Replace,
            }),
            ..Default::default()
        };
        assert!(app.handle_viewport_action(output));
        assert_eq!(app.document.selected_control_points().count(), index + 1);
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn plane_through_points_cancel_and_insufficient_selection_preserve_redo() {
    let mut app = test_app();
    for p in points().iter().take(2) {
        app.document.add_geometry(Geometry::Point(*p)).unwrap();
    }
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "PlaneThroughPt");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    app.cancel_interactive_command(true);
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}
