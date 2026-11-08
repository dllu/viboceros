//! Real egui pointer events pick source faces through wireframe and shaded previews.
use super::*;
use crate::viewport::{FacePickMode, ViewportInput, ViewportOutput};

fn frame(
    app: &mut VibocerosApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let allowed = app.offset_surface_session.as_ref().map(|s| s.ids());
    let input = ViewportInput {
        drafting: app.viewport_drafting_input(false, false),
        face_pick: Some(FacePickMode::SurfaceAndBrepAny),
        selection_allowed_objects: allowed.as_ref(),
        direction_analysis: app.offset_surface_session.as_ref().map(|s| s.directions()),
        ..Default::default()
    };
    let document = app
        .offset_surface_session
        .as_ref()
        .and_then(|s| s.scene())
        .unwrap_or(&app.document);
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
            output = app.viewports[0].show(ui, document, input, &[], 0, true);
        },
    )
    .drop_without_applying_deltas();
    (output, rect)
}
fn click(app: &mut VibocerosApp, ctx: &egui::Context, id: ObjectId) {
    let (_, rect) = frame(app, ctx, vec![]);
    let pos = app.viewports[0]
        .project(Point3::try_new(2., 1., 0.).unwrap(), rect)
        .unwrap();
    let (out, _) = frame(app, ctx, vec![egui::Event::PointerMoved(pos)]);
    app.handle_viewport_action(out);
    let mut seen = 0;
    for pressed in [true, false] {
        let (out, _) = frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        if let Some((picked, _)) = out.face_click {
            assert_eq!(picked, id);
            seen += 1;
        }
        app.handle_viewport_action(out);
    }
    assert_eq!(seen, 1);
}

#[test]
fn whole_face_picks_select_then_flip_without_selecting_preview_objects() {
    for mode in [DisplayMode::Wireframe, DisplayMode::Shaded] {
        let (mut app, id) = ready();
        app.document.clear_selection();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.viewports[0].display_mode = mode;
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        app.viewports[0]
            .zoom_extents(&app.document, Default::default())
            .unwrap();
        let before = app.document.object(id).unwrap().geometry().clone();
        enter(&mut app, "OffsetSrf");
        click(&mut app, &ctx, id);
        assert_eq!(app.document.selected_object_count(), 1);
        enter(&mut app, "");
        assert!(app.offset_surface_session.is_some());
        click(&mut app, &ctx, id);
        assert_eq!(app.document.object(id).unwrap().geometry(), &before);
        assert!(app.document.can_redo());
        assert!(!app.document.can_undo());
        let s = app.offset_surface_session.as_ref().unwrap();
        assert!(s.directions().markers[0].axes[2].unwrap().z() < 0.);
        assert_eq!(
            s.candidate
                .as_ref()
                .unwrap()
                .geometries()
                .next()
                .unwrap()
                .bounds()
                .unwrap()
                .min()
                .z(),
            -1.
        );
        enter(&mut app, "");
        assert!(app.command_line_idle());
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(app.document.selected_object_count(), 0);
    }
    println!("OFFSET_SURFACE_MOUSE_FACE_PICKS 4");
}
