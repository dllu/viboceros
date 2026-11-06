//! Real egui pointer events through reference, base and endpoint getters.
use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
    let curve = app.line_normal_curve().cloned();
    let picking = app.picking_line_normal();
    let input = ViewportInput {
        drafting: app.viewport_drafting_input(false, false),
        object_filter: picking.then_some(viboceros_command::ObjectSelectionFilter::Parametric),
        face_pick: picking.then_some(crate::viewport::FacePickMode::SurfaceAndBrepAny),
        normal_surface: app.line_normal_surface(),
        edge_curve: curve.as_ref(),
        line_input: app.line_input,
        translation_constraint: app.line_axis_constraint(),
        ..Default::default()
    };
    context
        .run_ui(
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
fn click(app: &mut VibocerosApp, context: &egui::Context, pointer: egui::Pos2) -> ViewportOutput {
    frame(app, context, vec![egui::Event::PointerMoved(pointer)]);
    frame(
        app,
        context,
        vec![egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    frame(
        app,
        context,
        vec![egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
    .0
}
#[test]
fn curve_and_surface_mouse_getters_commit_the_exact_normal_preview() {
    use viboceros_geometry::{Circle3, NurbsSurface};
    for surface in [false, true] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.viewports[0].display_mode = DisplayMode::Shaded;
        app.grid_snap = false;
        app.osnap = false;
        app.smart_track = false;
        let geometry = if surface {
            Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(0., 0., 0.),
                    point(6., 0., 0.),
                    point(6., 6., 4.),
                    point(0., 6., 0.),
                ])
                .unwrap(),
            )
        } else {
            Geometry::Circle(
                Circle3::try_new(
                    point(0., 0., 0.),
                    3.,
                    viboceros_command::CommandContext::default()
                        .construction_plane
                        .z_axis(),
                    app.document.tolerance(),
                )
                .unwrap(),
            )
        };
        let id = app.document.add_geometry(geometry).unwrap();
        let parent = app.document.object(id).unwrap().clone();
        app.document.clear_history().unwrap();
        let context = egui::Context::default();
        let (_, rect) = frame(&mut app, &context, vec![]);
        enter(&mut app, "Line Normal");
        assert!(app.picking_line_normal());
        assert!(!app.viewport_drafting_input(false, false).active);
        let aim = if surface {
            point(3., 3., 1.)
        } else {
            point(3., 0., 0.)
        };
        let pointer = app.viewports[0].project(aim, rect).unwrap();
        let output = click(&mut app, &context, pointer);
        if surface {
            assert_eq!(output.face_click, Some((id, 0)));
        } else {
            assert_eq!(output.selection_click.unwrap().object_id, Some(id));
        }
        app.handle_viewport_action(output);
        assert!(!app.picking_line_normal());
        assert!(app.viewport_drafting_input(false, false).active);
        let base = if surface {
            point(3., 3., 1.)
        } else {
            point(1.8, 2.4, 0.)
        };
        let pointer = app.viewports[0].project(base, rect).unwrap();
        let output = click(&mut app, &context, pointer);
        if surface {
            assert!(output.picked_point.is_some());
        } else {
            assert!(output.edge_parameter.is_some());
        }
        app.handle_viewport_action(output);
        let input = app.line_input.unwrap();
        assert!(input.endpoint(), "{:?}", app.command_log);
        assert!(input.anchor().unwrap().distance_to(base).unwrap() < 0.05);
        assert_eq!(app.document.objects().len(), 1);
        let aim = if surface {
            point(1., 1., 0.)
        } else {
            point(5., 6., 0.)
        };
        let pointer = app.viewports[0].project(aim, rect).unwrap();
        let (hover, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(pointer)]);
        let preview = input.preview(hover.drafting_hover.unwrap()).unwrap();
        app.handle_viewport_action(hover);
        let output = click(&mut app, &context, pointer);
        assert!(output.picked_point.is_some());
        app.handle_viewport_action(output);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert_eq!(
            app.document.objects().last().unwrap().geometry(),
            &Geometry::Line(preview.line)
        );
        assert_eq!(app.document.object(id).unwrap(), &parent);
        assert!(
            parent
                .geometry_snapshot()
                .shares_storage_with(app.document.object(id).unwrap().geometry_snapshot())
        );
        assert!(!app.document.is_selected(id));
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.can_undo());
    }
}
#[test]
fn a_straight_curve_normal_plane_accepts_mouse_points_in_an_edge_on_view() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    enter(&mut app, "Line 0,0,0 10,0,0");
    app.document.clear_history().unwrap();
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "Line Normal");
    let pointer = app.viewports[0].project(point(5., 0., 0.), rect).unwrap();
    let output = click(&mut app, &context, pointer);
    app.handle_viewport_action(output);
    let output = click(&mut app, &context, pointer);
    app.handle_viewport_action(output);
    assert!(app.line_input.unwrap().mouse_plane().is_some());
    let pointer = app.viewports[0].project(point(7., 4., 0.), rect).unwrap();
    let (hover, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(pointer)]);
    let preview = app
        .line_input
        .unwrap()
        .preview(hover.drafting_hover.unwrap())
        .unwrap();
    assert!((preview.line.end().x() - preview.line.start().x()).abs() < 1e-12);
    assert!((preview.line.end().y() - 4.).abs() < 0.05);
    let output = click(&mut app, &context, pointer);
    app.handle_viewport_action(output);
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &Geometry::Line(preview.line)
    );
}
