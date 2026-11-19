use super::super::circle_preview::viewport_frame;
#[cfg(feature = "native-smlib")]
use super::super::circle_preview::{screen_point, viewport_click};
use super::*;
use viboceros_geometry::NurbsCurve;

fn scene(curve: NurbsCurve) -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let surface = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    point(0., 0., 2.),
                    point(2., 0., 2.),
                    point(0., 3., 2.),
                    point(2., 3., 2.),
                ],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    let rail = app
        .document
        .add_geometry(Geometry::NurbsCurve(curve))
        .unwrap();
    app.document
        .select_objects_direct([surface], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, surface, rail)
}

fn quadratic() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![
            point(8., -3., 0.),
            point(8.4, -2., 3.8),
            point(12., -1., 5.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[cfg(feature = "native-smlib")]
fn frame_rails(app: &mut VibocerosApp, ctx: &egui::Context) {
    viewport_frame(app, ctx, vec![]);
    viewport_frame(app, ctx, vec![]);
    let bounds =
        viboceros_geometry::BoundingBox3::from_points([point(-2., -5., -4.), point(14., 5., 9.)])
            .unwrap();
    app.viewports[0].zoom_bounding_box(bounds).unwrap();
}

#[cfg(feature = "native-smlib")]
fn hover(
    app: &mut VibocerosApp,
    ctx: &egui::Context,
    target: Point3,
) -> (egui::Pos2, Vec<[egui::Pos2; 2]>) {
    viewport_frame(app, ctx, vec![]);
    for _ in 0..8 {
        let pointer = screen_point(app, target);
        assert!(app.viewports[0].screen_rect().unwrap().contains(pointer));
        let lines = viewport_frame(app, ctx, vec![egui::Event::PointerMoved(pointer)]);
        if screen_point(app, target) == pointer {
            return (pointer, lines);
        }
    }
    panic!("viewport layout did not settle");
}

#[cfg(feature = "native-smlib")]
fn assert_result(app: &VibocerosApp, min: Point3, max: Point3, solid: bool, epsilon: f64) {
    assert!(app.surface_path_prompt.is_none(), "{:?}", app.command_log);
    let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!("expected the path extrusion B-rep")
    };
    assert_eq!(result.is_solid(), solid);
    assert_eq!(result.faces().len(), if solid { 6 } else { 4 });
    let bounds = result.bounds();
    assert!(
        bounds.min().distance_to(min).unwrap() < epsilon,
        "{bounds:?}"
    );
    assert!(
        bounds.max().distance_to(max).unwrap() < epsilon,
        "{bounds:?}"
    );
    assert_eq!(app.document.undo_label(), Some("ExtrudeSrfAlongCrv"));
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_viewport_pick_uses_the_curve_parameter_and_explicit_reverse_overrides_it() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(8., -3., 0.),
            point(8.2, -2.6, 0.3),
            point(12., -1., 5.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    // The pick at t=.6 is physically closer to the start. Rhino still uses the end.
    for (input, parameter, reverse) in [
        ("ExtrudeSrfAlongCrv", 0.1, false),
        ("ExtrudeSrfAlongCrv", 0.6, true),
        ("ExtrudeSrfAlongCrv Reverse=No", 0.9, false),
        ("ExtrudeSrfAlongCrv Reverse=Yes", 0.1, true),
    ] {
        let (mut app, source, rail) = scene(curve.clone());
        let ctx = egui::Context::default();
        enter(&mut app, input);
        frame_rails(&mut app, &ctx);
        let (pointer, _) = hover(&mut app, &ctx, curve.evaluate(parameter).unwrap());
        viewport_click(&mut app, &ctx, pointer);
        let (min, max) = if reverse {
            (point(-4., -2., -3.), point(2., 3., 2.))
        } else {
            (point(0., 0., 2.), point(6., 5., 7.))
        };
        assert_result(&app, min, max, false, 1e-6);
        assert!(app.document.object(source).is_some() && app.document.object(rail).is_some());
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_subcurve_mouse_preview_and_click_use_the_same_translated_rail() {
    for reverse in [false, true] {
        let curve = quadratic();
        let (mut app, source, rail) = scene(curve.clone());
        let ctx = egui::Context::default();
        enter(&mut app, "ExtrudeSrfAlongCrv");
        enter(&mut app, "_SubCurve");
        frame_rails(&mut app, &ctx);
        let (pick, _) = hover(
            &mut app,
            &ctx,
            curve.evaluate(if reverse { 0.92 } else { 0.1 }).unwrap(),
        );
        viewport_click(&mut app, &ctx, pick);
        assert!(
            app.surface_path_prompt
                .as_ref()
                .unwrap()
                .hint()
                .starts_with("Starting")
        );
        enter(&mut app, "w8.528,-2.4,2.046");
        assert_eq!(
            app.surface_path_prompt.as_ref().unwrap().hint(),
            "Ending point on curve"
        );
        let (_, first) = hover(&mut app, &ctx, curve.evaluate(0.45).unwrap());
        let (cursor, second) = hover(&mut app, &ctx, curve.evaluate(0.65).unwrap());
        assert!(!first.is_empty() && !second.is_empty() && first != second);
        let displaced = if reverse {
            point(0.784, -0.7, -1.5245)
        } else {
            point(1.344, 0.7, 3.7955)
        };
        let projected = screen_point(&app, displaced);
        assert!(
            second
                .iter()
                .flatten()
                .any(|p| p.distance(projected) < 0.015)
        );
        assert!(!app.document.can_undo());
        assert_eq!(app.document.objects().count(), 2);
        viewport_click(&mut app, &ctx, cursor);
        let (min, max) = if reverse {
            (point(0., -0.7, -1.5245), point(2.784, 3., 2.))
        } else {
            (point(0., 0., 2.), point(3.344, 3.7, 3.7955))
        };
        assert_result(&app, min, max, false, 2e-4);
        enter(&mut app, "Undo");
        assert!(app.document.object(source).is_some() && app.document.object(rail).is_some());
        assert_eq!(app.document.objects().count(), 2);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().count(), 3);
        assert!(app.commands.surface_path_settings().subcurve);
        assert_eq!(app.commands.surface_path_settings().reverse, None);
        assert_eq!(app.commands.surface_path_settings().path_range, None);
    }
}

#[test]
fn surface_path_stale_rail_and_second_point_enter_leave_document_history_intact() {
    let (mut app, _, path) = scene(quadratic());
    let ctx = egui::Context::default();
    enter(
        &mut app,
        &format!("ExtrudeSrfAlongCrv SubCurve=Yes Path={path}"),
    );
    enter(&mut app, "w8.528,-2.4,2.046");
    app.document
        .replace_object_geometries([(path, Geometry::Point(point(8., -3., 0.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "w9.872,-1.7,3.8415");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("path curve changed")
    );
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
    assert!(app.surface_path_prompt.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_first_point_enter_uses_the_whole_rail_in_its_picked_direction() {
    let (mut app, _, path) = scene(quadratic());
    enter(
        &mut app,
        &format!("ExtrudeSrfAlongCrv SubCurve=Yes Solid=Yes Reverse=Yes Path={path}"),
    );
    assert!(
        app.surface_path_prompt
            .as_ref()
            .unwrap()
            .hint()
            .starts_with("Starting")
    );
    enter(&mut app, "_Enter");
    assert_result(&app, point(-4., -2., -3.), point(2., 3., 2.), true, 1e-6);
    assert!(app.commands.surface_path_settings().solid);
}
