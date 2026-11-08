//! Real headless egui pointer events choose opening faces in both display modes.
use super::*;
use crate::viewport::{ComponentPickFilter, FacePickMode, ViewportInput, ViewportOutput};

fn frame(
    app: &mut VibocerosApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let input = ViewportInput {
        drafting: app.viewport_drafting_input(false, false),
        face_pick: Some(FacePickMode::SurfaceAndBrepAny),
        component_pick: Some(ComponentPickFilter::Faces),
        ..Default::default()
    };
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            rect = ui.available_rect_before_wrap();
            output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
        },
    )
    .drop_without_applying_deltas();
    (output, rect)
}

#[test]
fn shell_pointer_picks_opening_faces_in_wireframe_and_shaded_views() {
    for display in [DisplayMode::Wireframe, DisplayMode::Shaded] {
        let mut app = test_app();
        let source = source(&mut app);
        app.document.clear_history().unwrap();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.viewports[0].display_mode = display;
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        app.viewports[0]
            .zoom_extents(&app.document, Default::default())
            .unwrap();
        enter(&mut app, "Shell Thickness=0 DeleteInput=No");
        let (_, rect) = frame(&mut app, &ctx, vec![]);
        let pos = app.viewports[0].project(point(5., 4., 6.), rect).unwrap();
        let (output, _) = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        app.handle_viewport_action(output);
        let mut seen = 0;
        for pressed in [true, false] {
            let (output, _) = frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
            if let Some(click) = &output.component_click {
                assert!(click.picks.iter().any(|p| p.object == source));
                seen += 1;
            }
            app.handle_viewport_action(output);
        }
        assert_eq!(seen, 1);
        let picks = app
            .component_selection
            .checked_picks(&app.document)
            .unwrap();
        assert_eq!(picks.len(), 1);
        let Geometry::Brep(b) = app.document.object(source).unwrap().geometry() else {
            panic!("source")
        };
        let face = &b.faces()[picks[0].index];
        let (u, v) = face
            .surface()
            .closest_parameters(point(5., 4., 6.), app.document.tolerance())
            .unwrap();
        assert!(
            face.surface()
                .evaluate(u, v)
                .unwrap()
                .distance_to(point(5., 4., 6.))
                .unwrap()
                < 1e-6
        );
        enter(&mut app, "");
        assert!(app.shell_prompt.is_none());
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("Shell"));
    }
    println!("SHELL_MOUSE_FACE_PICKS 2");
}
