use super::*;
use viboceros_document::{ControlPointId, GripMode};

fn enter(app: &mut VibocerosApp, value: &str) {
    app.command_input = value.into();
    app.run_command();
}

#[test]
fn edit_points_support_preselection_and_command_first_move_and_history() {
    for command_first in [false, true] {
        let mut app = test_app();
        enter(&mut app, "ControlPointCurve 2 0,0,0 1,3,0 3,-2,0 4,0,0");
        let id = app.document.objects().next().unwrap().id();
        let original = app.document.object(id).unwrap().geometry().clone();
        let source = original.curve_ref().unwrap().to_nurbs().unwrap();
        if command_first {
            app.document.clear_selection();
            enter(&mut app, "EditPtOn");
            assert!(app.object_prompt.is_some());
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        } else {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "EditPtOn");
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.grip_mode(id), Some(GripMode::Edit));
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        let pick = ControlPointId {
            object: id,
            index: 1,
        };
        app.document
            .select_control_points([pick], SelectionMode::Replace)
            .unwrap();
        if command_first {
            enter(&mut app, "Move");
            enter(&mut app, "0,0,0");
            enter(&mut app, "0,2,0");
        } else {
            enter(&mut app, "Move 0,0,0 0,2,0");
        }
        assert_eq!(app.document.undo_label(), Some("Move"));
        let Geometry::NurbsCurve(output) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        for (i, edit) in source.edit_points().unwrap().iter().enumerate() {
            let target = if i == 1 {
                point(edit.point.x(), edit.point.y() + 2., edit.point.z())
            } else {
                edit.point
            };
            assert!(
                output
                    .evaluate_on_side(edit.parameter, edit.side)
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
                    < 1e-8
            );
        }
        let changed = output.clone();
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        assert_eq!(app.document.grip_mode(id), Some(GripMode::Edit));
        enter(&mut app, "Redo");
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &Geometry::NurbsCurve(changed)
        );
        enter(&mut app, "PointsOff");
        assert!(app.document.grip_mode(id).is_none());
    }
}

#[test]
fn smooth_rejects_edit_grips_without_changing_geometry_or_history() {
    let mut app = test_app();
    enter(&mut app, "ControlPointCurve 2 0,0,0 1,3,0 3,-2,0 4,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "EditPtOn");
    app.document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 1,
            }],
            SelectionMode::Replace,
        )
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "Smooth SmoothFactor=0.5");
    // The interactive picker may defer execution until its options are accepted.
    if app.object_prompt.is_some() {
        enter(&mut app, "");
    }
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.back().unwrap().contains("edit-point grips"));
}
