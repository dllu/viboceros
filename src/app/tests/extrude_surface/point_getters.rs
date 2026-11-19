use super::super::circle_preview::viewport_frame;
use super::*;
use crate::app::extrude_surface::Stage;

#[test]
fn surface_extrusion_enter_resumes_the_parent_picker_with_the_current_axis() {
    for to_boundary in [false, true] {
        for second_point in [false, true] {
            let mut app = test_app();
            let id = surface_at(&mut app, 2., 0.);
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            app.document.clear_history().unwrap();
            enter(&mut app, "ExtrudeSrf Direction=1,0,2");
            let Some(InteractiveCommand::ExtrudeSurface { options, .. }) = app.active_command
            else {
                panic!("expected the distance getter")
            };
            if to_boundary {
                enter(&mut app, "ToBoundary");
            }
            let parent = app.active_command;
            enter(&mut app, "Direction");
            let Some(InteractiveCommand::ExtrudeSurface {
                stage: Stage::DirectionStart { .. },
                ..
            }) = app.active_command
            else {
                panic!("expected the direction base point")
            };
            assert!(
                app.command_log
                    .back()
                    .unwrap()
                    .ends_with("<0.447,0.000,0.894>")
            );
            if second_point {
                enter(&mut app, "w2,3,4");
                assert!(matches!(
                    app.active_command,
                    Some(InteractiveCommand::ExtrudeSurface {
                        stage: Stage::DirectionEnd { .. },
                        ..
                    })
                ));
                assert!(
                    app.command_log
                        .back()
                        .unwrap()
                        .ends_with("<0.447,0.000,0.894>")
                );
            }
            enter(&mut app, if second_point { "_Enter" } else { "" });
            assert_eq!(app.active_command, parent);
            assert_eq!(app.picking_surface_boundary(), to_boundary);
            assert!(!app.document.can_undo());
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                vec![id]
            );
            if !to_boundary {
                assert_eq!(
                    app.surface_extrusion_preview().unwrap().options.direction,
                    options.direction
                );
                enter(&mut app, "w4,4,8");
                if cfg!(feature = "native-smlib") {
                    let Geometry::Brep(result) =
                        app.document.selected_objects().next().unwrap().geometry()
                    else {
                        panic!("expected the oblique extrusion")
                    };
                    assert!(
                        result
                            .bounds()
                            .max()
                            .is_near(point(5., 3., 8.), app.document.tolerance())
                    );
                }
            }
        }
    }
}

#[test]
fn surface_extrusion_keyboard_enter_restores_the_source_center_after_set_base_point() {
    let mut app = test_app();
    let id = surface_at(&mut app, 2., 0.);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let ctx = egui::Context::default();
    enter(&mut app, "ExtrudeSrf");
    enter(&mut app, "SetBasePoint");
    enter(&mut app, "w1,1.5,5");
    assert_eq!(
        app.surface_extrusion_preview().unwrap().constraint.anchor,
        point(1., 1.5, 5.)
    );
    enter(&mut app, "SetBasePoint");
    assert!(app.surface_extrusion_preview().is_none());
    app.command_focus_requested = true;
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(
        app.surface_extrusion_preview().unwrap().constraint.anchor,
        point(1., 1.5, 2.)
    );
    assert_eq!(app.commands.surface_extrusion_settings().distance, None);
    assert!(!app.document.can_undo());
}
