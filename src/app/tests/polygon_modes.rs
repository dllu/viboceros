use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use serde::Deserialize;
use viboceros_geometry::{Circle3, CircularArc3, LineSegment, Polyline3, Vector3};

#[derive(Deserialize)]
struct Capture {
    name: String,
    inputs: Vec<String>,
    guide: Option<String>,
    plane: Option<[[f64; 3]; 3]>,
    #[serde(default)]
    preselected: bool,
    vertices: Vec<[f64; 3]>,
}

#[test]
fn polygon_staged_and_scripted_modes_match_native_geometry() {
    let captures: Vec<Capture> =
        serde_json::from_str(include_str!("polygon_modes/rhino.json")).unwrap();
    for capture in captures {
        let mut app = test_app();
        app.active_viewport = 0;
        app.viewports[0] = Viewport::new(ViewKind::Top);
        let tolerance = app.document.tolerance();
        if let Some([origin, x, y]) = capture.plane {
            let frame = Frame3::try_from_directions(
                Point3::try_from(origin).unwrap(),
                Vector3::try_from(x).unwrap(),
                Vector3::try_from(y).unwrap(),
                tolerance,
            )
            .unwrap();
            assert!(app.viewports[0].set_construction_plane(frame));
        }
        let plane = app.viewports[0].construction_plane();
        let guide = capture.guide.as_deref().map(|guide| {
            let origin = point(0., 0., 0.);
            let geometry = if guide == "arc" {
                let circle =
                    Circle3::try_from_frame(origin, 8., plane.x_axis(), plane.z_axis(), tolerance)
                        .unwrap();
                Geometry::Arc(
                    CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
                        .unwrap(),
                )
            } else {
                Geometry::Line(LineSegment::try_new(origin, point(0., 0., 10.), tolerance).unwrap())
            };
            app.document.add_geometry(geometry).unwrap()
        });
        let before = usize::from(guide.is_some());
        if capture.preselected {
            app.document
                .select_object(guide.unwrap(), SelectionMode::Add)
                .unwrap();
        }
        let selected = usize::from(capture.preselected);
        let tokens: Vec<_> = capture
            .inputs
            .iter()
            .map(|token| {
                guide.map_or_else(
                    || token.clone(),
                    |id| token.replace("{guide}", &id.to_string()),
                )
            })
            .collect();

        // Run the script first so the staged replay starts with fresh preferences.
        enter(&mut app, &format!("Polygon {}", tokens.join(" ")));
        assert_eq!(
            app.document.objects().len(),
            before + 1,
            "{}: {:?}",
            capture.name,
            app.command_log
        );
        assert_native_outline(&app, &capture);
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Polygon"));
        app.commands
            .remember_polygon_preferences(viboceros_command::polygon::Preferences::default());

        enter(&mut app, "Polygon");
        for token in &tokens {
            assert_eq!(app.document.objects().len(), before, "{}", capture.name);
            if capture.preselected && token.starts_with("Curve=") {
                continue;
            }
            enter(&mut app, token);
        }
        assert!(
            app.polygon_session.is_none(),
            "{}: {:?}",
            capture.name,
            app.command_log
        );
        assert_eq!(
            app.document.selected_object_count(),
            selected,
            "{}",
            capture.name
        );
        assert_eq!(app.document.objects().len(), before + 1);
        assert_native_outline(&app, &capture);
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Polygon"));
        assert_eq!(app.document.objects().len(), before);
        assert_eq!(app.document.redo().unwrap().as_deref(), Some("Polygon"));
        assert_eq!(
            app.document.selected_object_count(),
            selected,
            "{}",
            capture.name
        );
        assert_native_outline(&app, &capture);
    }
}

fn outline(app: &VibocerosApp) -> &Polyline3 {
    let Geometry::Polyline(polyline) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a polygon");
    };
    polyline
}

fn assert_native_outline(app: &VibocerosApp, capture: &Capture) {
    let polygon = outline(app);
    assert!(polygon.is_closed(), "{}", capture.name);
    assert_eq!(
        polygon.vertices().len(),
        capture.vertices.len(),
        "{}",
        capture.name
    );
    for (i, (&actual, coordinates)) in polygon.vertices().iter().zip(&capture.vertices).enumerate()
    {
        let expected = Point3::try_from(*coordinates).unwrap();
        assert!(
            actual.distance_to(expected).unwrap() < 1e-8,
            "{}: vertex {i}: {actual:?} != {expected:?}",
            capture.name,
        );
    }
}

fn top_app() -> VibocerosApp {
    let mut app = test_app();
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app
}

#[test]
fn polygon_fixed_size_preview_tracks_the_cursor_and_enter_matches_the_outline() {
    let mut app = top_app();
    enter(&mut app, "Poly Mode=Circumscribed NumSides=6");
    enter(&mut app, "0");
    enter(&mut app, "4mm");
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(6., 1., 0.));
    let b = screen_point(&app, point(2., 3., 0.));
    let first = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let last = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert_eq!(first.len(), 6);
    assert_eq!(last.len(), 6);
    assert_ne!(first, last);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(app.polygon_session.is_none());
    assert_drawn_outline(&app, &last);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Polygon"));
    enter(&mut app, "Polygon");
    assert_eq!(app.polygon_session.as_ref().unwrap().input.sides(), 6);
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("Center of circumscribed polygon"))
    );
    app.cancel_interactive_command(true);
    assert_eq!(app.document.redo_label(), Some("Polygon"));
}

#[test]
fn polygon_edge_flip_and_star_inner_radius_update_the_live_outline() {
    let mut app = top_app();
    let ctx = egui::Context::default();
    enter(&mut app, "Polygon Edge");
    enter(&mut app, "0");
    viewport_frame(&mut app, &ctx, vec![]);
    let edge_end = screen_point(&app, point(4., 0., 0.));
    let above = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge_end)]);
    enter(&mut app, "Flip");
    let below = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge_end)]);
    assert_eq!(above.len(), 5);
    assert_eq!(below.len(), 5);
    assert_ne!(above, below);
    let picked = click_outline(&mut app, &ctx, edge_end);
    assert!(outline(&app).vertices().iter().any(|p| p.y() < -1.));
    assert_drawn_outline(&app, &picked);

    enter(&mut app, "Polygon Star");
    enter(&mut app, "w-3,0,0");
    enter(&mut app, "w1,0,0");
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(-2., 0., 0.));
    let b = screen_point(&app, point(-1., 0., 0.));
    let little = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let large = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert_eq!(little.len(), 10);
    assert_eq!(large.len(), 10);
    assert_ne!(little, large);
    assert_eq!(app.document.objects().len(), 1);
    let picked = click_outline(&mut app, &ctx, b);
    assert_eq!(outline(&app).segment_count(), 10);
    assert_eq!(app.document.objects().len(), 2);
    assert_drawn_outline(&app, &picked);
}

fn click_outline(
    app: &mut VibocerosApp,
    ctx: &egui::Context,
    pos: egui::Pos2,
) -> Vec<[egui::Pos2; 2]> {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    viewport_frame(app, ctx, vec![button(true)]);
    viewport_frame(app, ctx, vec![button(false)])
}

fn assert_drawn_outline(app: &VibocerosApp, strokes: &[[egui::Pos2; 2]]) {
    let outline = outline(app);
    assert_eq!(strokes.len(), outline.segment_count());
    for (stroke, vertices) in strokes.iter().zip(outline.vertices().windows(2)) {
        for (&drawn, &vertex) in stroke.iter().zip(vertices) {
            let projected = screen_point(app, vertex);
            assert!(
                drawn.distance(projected) < 0.2,
                "{drawn:?} != {projected:?} for {vertex:?}"
            );
        }
    }
}

#[test]
fn polygon_count_accepts_enter_and_keeps_mouse_picks_out_of_the_number_prompt() {
    let mut app = top_app();
    let ctx = egui::Context::default();
    enter(&mut app, "Polygon NumSides");
    assert!(app.polygon_session.as_ref().unwrap().input.pending_count());
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(1., 0., 0.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    viewport_click(&mut app, &ctx, a);
    let input = app.polygon_session.as_ref().unwrap().input;
    assert!(input.pending_count());
    assert!(input.anchor().is_none());
    enter(&mut app, "2");
    assert!(app.polygon_session.as_ref().unwrap().input.pending_count());
    enter(&mut app, "");
    let input = app.polygon_session.as_ref().unwrap().input;
    assert!(!input.pending_count());
    assert_eq!(input.sides(), 5);
    assert!(!app.document.can_undo());
}
