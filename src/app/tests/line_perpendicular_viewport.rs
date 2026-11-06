//! Windowless egui events through source, constrained-base and endpoint picks.
use super::*;
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}
fn frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let curve = app.line_perpendicular_curve().cloned();
    let picking = app.picking_line_perpendicular();
    let input = ViewportInput {
        drafting: app.viewport_drafting_input(false, false),
        object_filter: picking.then_some(viboceros_command::ObjectSelectionFilter::Curves),
        edge_pick: picking,
        curve_or_edge_pick: picking,
        edge_curve: curve.as_ref(),
        perpendicular_input: app.line_perpendicular.as_ref().map(|s| &s.input),
        ..Default::default()
    };
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
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
fn click(app: &mut VibocerosApp, context: &egui::Context, p: egui::Pos2) -> ViewportOutput {
    frame(app, context, vec![egui::Event::PointerMoved(p)]);
    frame(
        app,
        context,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    frame(
        app,
        context,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
    .0
}
fn scene() -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    let source = app
        .document
        .add_geometry(Geometry::Circle(
            viboceros_geometry::Circle3::try_new(
                point(0., 0., 0.),
                5.,
                viboceros_command::CommandContext::default()
                    .construction_plane
                    .z_axis(),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let sentinel = app
        .document
        .add_geometry(Geometry::Point(point(-10., -10., 0.)))
        .unwrap();
    app.document
        .select_objects_direct([sentinel], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    frame(&mut app, &egui::Context::default(), vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    (app, source, sentinel)
}
#[test]
fn sliding_and_fixed_contacts_commit_the_exact_mouse_preview_without_editing_sources() {
    for fixed in [false, true] {
        let (mut app, id, sentinel) = scene();
        let parent = app.document.object(id).unwrap().clone();
        let context = egui::Context::default();
        let (_, rect) = frame(&mut app, &context, vec![]);
        enter(&mut app, "Line Perpendicular");
        assert!(app.picking_line_perpendicular());
        assert!(!app.viewport_drafting_input(false, false).active);
        let p = app.viewports[0].project(point(5., 0., 0.), rect).unwrap();
        let output = click(&mut app, &context, p);
        assert_eq!(output.selection_click.unwrap().object_id, Some(id));
        app.handle_viewport_action(output);
        assert!(app.line_perpendicular.as_ref().unwrap().input.endpoint());
        assert!(app.document.is_selected(sentinel));
        let initial = app
            .line_perpendicular
            .as_ref()
            .unwrap()
            .input
            .anchor()
            .unwrap();
        if fixed {
            enter(&mut app, "FromFirstPoint");
        }
        let p = app.viewports[0].project(point(8., 6., 0.), rect).unwrap();
        let (hover, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(p)]);
        let preview = app
            .line_perpendicular
            .as_ref()
            .unwrap()
            .input
            .preview(hover.drafting_hover.unwrap())
            .unwrap();
        if fixed {
            assert!(preview.line.start().distance_to(initial).unwrap() < 1e-10);
        } else {
            assert!(preview.line.start().distance_to(point(4., 3., 0.)).unwrap() < 0.05);
        }
        let output = click(&mut app, &context, p);
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
        assert!(app.document.is_selected(sentinel));
        app.document.undo().unwrap();
        assert!(!app.document.can_undo());
        assert_eq!(app.document.objects().len(), 2);
    }
}
#[test]
fn a_mouse_base_on_a_constrained_curve_is_the_relative_input_origin() {
    let (mut app, id, _) = scene();
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "Line Perpendicular PointOnCurve");
    assert!(app.picking_line_perpendicular());
    let p = app.viewports[0].project(point(5., 0., 0.), rect).unwrap();
    let output = click(&mut app, &context, p);
    app.handle_viewport_action(output);
    assert!(app.line_perpendicular_curve().is_some());
    let p = app.viewports[0].project(point(3., 4., 0.), rect).unwrap();
    let output = click(&mut app, &context, p);
    assert!(output.edge_parameter.is_some());
    app.handle_viewport_action(output);
    let base = app
        .line_perpendicular
        .as_ref()
        .unwrap()
        .input
        .anchor()
        .unwrap();
    assert_eq!(app.last_point, Some(base));
    let target = base
        .translated(viboceros_geometry::Vector3::try_new(4., 2., 2.).unwrap())
        .unwrap();
    let expected = app
        .line_perpendicular
        .as_ref()
        .unwrap()
        .input
        .preview(target)
        .unwrap();
    enter(&mut app, "r4,2,2");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &Geometry::Line(expected.line)
    );
    assert!(app.document.object(id).is_some());
}
#[test]
fn a_free_point_can_snap_to_either_perpendicular_branch_on_a_curve() {
    for far in [false, true] {
        let (mut app, _, sentinel) = scene();
        let context = egui::Context::default();
        let (_, rect) = frame(&mut app, &context, vec![]);
        enter(&mut app, "Line Perpendicular Point");
        enter(&mut app, "w8,6,2");
        let p = app.viewports[0]
            .project(point(if far { -5. } else { 5. }, 0.2, 0.), rect)
            .unwrap();
        let output = click(&mut app, &context, p);
        assert!(output.perpendicular_curve_click.is_some());
        app.handle_viewport_action(output);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(line.start(), point(8., 6., 2.));
        let end = if far {
            point(-4., -3., 0.)
        } else {
            point(4., 3., 0.)
        };
        assert!(line.end().distance_to(end).unwrap() < 1e-10);
        assert_eq!(app.last_point, Some(line.end()));
        assert!(app.line_accepted_point.is_none());
        assert!(app.document.is_selected(sentinel));
    }
}
#[test]
fn planar_mouse_tracking_slides_in_height_while_a_fixed_contact_keeps_its_normal_plane() {
    use viboceros_geometry::{NurbsCurve, WeightedPoint3};
    for fixed in [false, true] {
        let (mut app, id, _) = scene();
        app.document
            .replace_object_geometries([(
                id,
                Geometry::NurbsCurve(
                    NurbsCurve::try_new_rational(
                        2,
                        [[0., 0., 0.], [4., 6., 3.], [10., 0., 2.]]
                            .map(|p| {
                                WeightedPoint3::try_new(
                                    viboceros_geometry::Point3::try_from(p).unwrap(),
                                    1.,
                                )
                                .unwrap()
                            })
                            .to_vec(),
                        vec![0., 0., 0., 1., 1., 1.],
                    )
                    .unwrap(),
                ),
            )])
            .unwrap();
        app.document.clear_history().unwrap();
        app.planar = true;
        let context = egui::Context::default();
        let (_, rect) = frame(&mut app, &context, vec![]);
        enter(&mut app, "Line Perpendicular");
        let p = app.viewports[0].project(point(4.5, 3., 2.), rect).unwrap();
        let output = click(&mut app, &context, p);
        app.handle_viewport_action(output);
        if fixed {
            enter(&mut app, "FromFirstPoint");
        }
        let p = app.viewports[0].project(point(9., 8., 6.), rect).unwrap();
        let (hover, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(p)]);
        let preview = app
            .line_perpendicular
            .as_ref()
            .unwrap()
            .input
            .preview(hover.drafting_hover.unwrap())
            .unwrap();
        if fixed {
            assert!((preview.line.end().z() - preview.line.start().z()).abs() > 1.);
        } else {
            assert!((preview.line.end().z() - preview.line.start().z()).abs() < 1e-12);
        }
        let output = click(&mut app, &context, p);
        app.handle_viewport_action(output);
        assert!(app.active_command.is_none());
        assert_eq!(
            app.document.objects().last().unwrap().geometry(),
            &Geometry::Line(preview.line)
        );
    }
}
