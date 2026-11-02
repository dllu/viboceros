//! Drive egui's actual point/selection dispatch with production drafting state.
use super::*;
use crate::viewport::ViewportInput;

fn frame(app: &mut VibocerosApp, context: &egui::Context, events: Vec<egui::Event>) -> egui::Rect {
    let drafting = app.viewport_drafting_input(false, false);
    let object_filter = app.viewport_object_filter();
    let analysis = app.commands.draft_angle_analysis_view(&app.document);
    let edge_pick = app.picking_draft_pull_curve();
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                rect = ui.available_rect_before_wrap();
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        drafting,
                        object_filter,
                        draft_angle_analysis: analysis.as_ref(),
                        edge_pick,
                        curve_or_edge_pick: edge_pick,
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    app.handle_viewport_action(output);
    rect
}
fn click(app: &mut VibocerosApp, context: &egui::Context, rect: egui::Rect, at: Point3) {
    let pointer = app.viewports[0].project(at, rect).unwrap();
    frame(app, context, vec![egui::Event::PointerMoved(pointer)]);
    for pressed in [true, false] {
        frame(
            app,
            context,
            vec![egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
}

#[test]
fn viewport_clicks_switch_between_direction_points_and_curve_selection() {
    let (mut app, _) = setup();
    let context = egui::Context::default();
    let rect = frame(&mut app, &context, vec![]);
    enter(&mut app, "DraftAngleAnalysis Direction=TwoPoints");
    click(&mut app, &context, rect, point(3., 3., 0.));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::DraftPullPoints { start: Some(_) })
    ));
    click(&mut app, &context, rect, point(6., 7., 0.));
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    close(up(&app), [3., 4., 0.]);
    let curve = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(4., 4., 0.),
                point(7., 4., 0.),
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
    click(&mut app, &context, rect, point(5.5, 4., 0.));
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    close(up(&app), [1., 0., 0.]);
    assert_eq!(app.document.objects().count(), 2);
    assert!(source.shares_storage_with(app.document.object(curve).unwrap().geometry_snapshot()));
}
