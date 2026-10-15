use super::*;
use egui::{Pos2, Rect, Vec2};
use viboceros_document::ControlPointId;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn frame(
    app: &VibocerosApp,
    view: &mut Viewport,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> (crate::viewport::ViewportOutput, Rect) {
    let mut result = crate::viewport::ViewportOutput::default();
    let mut rect = Rect::NOTHING;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
            events,
            ..Default::default()
        },
        |ui| {
            rect = ui.available_rect_before_wrap();
            result = view.show(
                ui,
                &app.document,
                crate::viewport::ViewportInput {
                    affine_preview: app.affine_preview(),
                    drafting: crate::viewport::DraftingInput {
                        active: true,
                        planar: true,
                        osnap: viboceros_drafting::ObjectSnapModes::NONE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                &[],
                2,
                true,
            );
        },
    )
    .drop_without_applying_deltas();
    (result, rect)
}

#[test]
fn set_point_preview_mouse_hover_click_and_commit_agree_without_preview_edits() {
    for edit in [false, true] {
        for copy in [false, true] {
            let mut app = test_app();
            app.active_viewport = 2;
            enter(&mut app, "ControlPointCurve 2 0,0,1 1,3,2 3,-2,3 4,0,4");
            let id = app.document.objects().next().unwrap().id();
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, if edit { "EditPtOn" } else { "PointsOn" });
            app.document
                .select_control_points(
                    [ControlPointId {
                        object: id,
                        index: 1,
                    }],
                    SelectionMode::Replace,
                )
                .unwrap();
            let source = app.document.control_point_locations(id).unwrap().to_vec();
            enter(
                &mut app,
                &format!(
                    "SetPt XSet=No YSet=No ZSet=Yes Copy={}",
                    if copy { "Yes" } else { "No" }
                ),
            );
            let before = format!("{:?}", app.document);
            let defaults = app.commands.copy_default("SetPt");
            let last_point = app.last_point;
            let ctx = egui::Context::default();
            let mut view = Viewport::new(crate::viewport::ViewKind::Front);
            let (_, rect) = frame(&app, &mut view, &ctx, vec![]);
            let pixel = view.project(point(2., 0., 5.), rect).unwrap();
            let (hover, _) = frame(
                &app,
                &mut view,
                &ctx,
                vec![egui::Event::PointerMoved(pixel)],
            );
            let target = hover.drafting_hover.unwrap();
            let map = hover.affine_preview.flatten().unwrap();
            assert!(map.orientation_reversing().is_err());
            assert!((target.z() - 5.).abs() < 1e-4);
            assert!(app.update_affine_preview(Some(map)));
            assert_eq!(format!("{:?}", app.document), before);
            assert_eq!(app.commands.copy_default("SetPt"), defaults);
            assert_eq!(app.last_point, last_point);
            let (gone, _) = frame(&app, &mut view, &ctx, vec![egui::Event::PointerGone]);
            assert_eq!(gone.affine_preview, None);
            assert_eq!(app.affine_preview().unwrap().last_transform, Some(map));
            for pressed in [true, false] {
                let (output, _) = frame(
                    &app,
                    &mut view,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(pixel),
                        egui::Event::PointerButton {
                            pos: pixel,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                if !pressed {
                    assert_eq!(output.picked_point, Some(target));
                    assert!(app.accept_drafting_point(output.picked_point.unwrap()));
                }
            }
            let result = app
                .document
                .objects()
                .find(|o| !copy || o.id() != id)
                .unwrap()
                .id();
            for (i, q) in app
                .document
                .control_point_locations(result)
                .unwrap()
                .iter()
                .enumerate()
            {
                let expected = if i == 1 {
                    map.transform_point(source[i]).unwrap()
                } else {
                    source[i]
                };
                assert!(q.distance_to(expected).unwrap() < 1e-8);
            }
            if copy {
                enter(&mut app, "");
            }
            assert!(app.affine_preview().is_none());
            assert_eq!(app.document.undo_label(), Some("SetPt"));
            enter(&mut app, "Undo");
            assert_eq!(app.document.control_point_locations(id).unwrap(), source);
        }
    }
}

#[test]
fn set_point_preview_option_changes_clear_obsolete_maps_and_cancel_clears_preview() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "SelAll");
    enter(&mut app, "SetPt");
    let map = app
        .affine_preview()
        .unwrap()
        .definition
        .transform_at(
            app.viewports[0].construction_plane(),
            point(4., 5., 6.),
            app.document.tolerance(),
        )
        .unwrap();
    assert!(app.update_affine_preview(Some(map)));
    enter(&mut app, "Copy=Yes");
    assert!(app.affine_preview().unwrap().copy);
    assert_eq!(app.affine_preview().unwrap().last_transform, Some(map));
    enter(&mut app, "XSet=No");
    assert_eq!(app.affine_preview().unwrap().last_transform, None);
    app.update_affine_preview(Some(map));
    enter(&mut app, "Alignment=CPlane");
    assert_eq!(app.affine_preview().unwrap().last_transform, None);
    let before = format!("{:?}", app.document);
    app.cancel_interactive_command(false);
    assert!(app.affine_preview().is_none());
    assert_eq!(format!("{:?}", app.document), before);
}
