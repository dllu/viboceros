mod sources;

use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use serde::Deserialize;
use viboceros_command::offset_curve::{Cap, Kind};

#[derive(Deserialize)]
struct Capture {
    name: String,
    setup: Vec<String>,
    inputs: Vec<String>,
    curves: Vec<Curve>,
    #[serde(default)]
    length_epsilon: Option<f64>,
}

#[derive(Deserialize)]
struct Curve {
    bounds: [[f64; 3]; 2],
    closed: bool,
    length: f64,
}

fn offset_app() -> VibocerosApp {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.grid_snap = true;
    app.osnap = false;
    app.smart_track = false;
    app
}

#[test]
fn staged_offset_and_multiple_match_native_curves_history_and_selection() {
    let captures: Vec<Capture> =
        serde_json::from_str(include_str!("curve_offset/rhino.json")).unwrap();
    for capture in captures {
        let mut app = offset_app();
        for setup in &capture.setup {
            enter(&mut app, setup);
        }
        let sources = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        enter(&mut app, "SelAll");
        for token in &capture.inputs {
            enter(&mut app, token);
        }
        assert!(
            app.active_command.is_none(),
            "{}: {:?}",
            capture.name,
            app.command_log
        );
        assert_eq!(app.document.selected_object_count(), 0, "{}", capture.name);
        check_curves(&app, &capture);
        let command = capture.inputs[0].split_whitespace().next().unwrap();
        assert_eq!(app.document.undo().unwrap().as_deref(), Some(command));
        assert_eq!(
            app.document.objects().map(|o| o.id()).collect::<Vec<_>>(),
            sources,
            "{}",
            capture.name
        );
        assert_eq!(app.document.redo().unwrap().as_deref(), Some(command));
        check_curves(&app, &capture);
    }
}

fn check_curves(app: &VibocerosApp, capture: &Capture) {
    assert_eq!(
        app.document.objects().len(),
        capture.curves.len(),
        "{}: {:?}",
        capture.name,
        app.command_log,
    );
    for (object, expected) in app.document.objects().zip(&capture.curves) {
        let geometry = object.geometry();
        let curve = geometry.curve_ref().unwrap();
        let tolerance = app.document.tolerance();
        let bounds = geometry.tight_bounds(tolerance).unwrap();
        let actual = [bounds.min().to_array(), bounds.max().to_array()];
        for (&a, &b) in actual
            .iter()
            .flatten()
            .zip(expected.bounds.iter().flatten())
        {
            assert!(
                (a - b).abs() < 1e-8,
                "{}: {actual:?} != {:?}",
                capture.name,
                expected.bounds
            );
        }
        assert_eq!(
            curve.is_closed().unwrap(),
            expected.closed,
            "{}",
            capture.name
        );
        let length = curve.length(tolerance).unwrap();
        assert!(
            (length - expected.length).abs() < capture.length_epsilon.unwrap_or(1e-7),
            "{}: {length} != {}",
            capture.name,
            expected.length
        );
    }
}

#[test]
fn offset_remembers_distance_and_caps_but_resets_run_modes() {
    let mut app = offset_app();
    for input in [
        "Line w0,0,0 w8,0,0",
        "SelAll",
        "Offset",
        "7",
        "Loose",
        "Cap=Round",
        "BothSides",
        "Cancel",
    ] {
        enter(&mut app, input);
    }
    let options = app.commands.offset_settings(Kind::Offset);
    assert_eq!(options.distance, 7.);
    assert_eq!(options.cap, Cap::Round);
    assert!(!options.loose && !options.both_sides);
    assert_eq!(app.document.objects().len(), 1);
    for input in ["Offset", "Cap=None", "ThroughPoint", "w4,-2.25,0"] {
        enter(&mut app, input);
    }
    assert_eq!(app.commands.offset_settings(Kind::Offset).distance, 2.25);
    assert_eq!(app.commands.offset_settings(Kind::Multiple).distance, 1.);
}

#[test]
fn command_first_offset_accepts_a_curve_and_multiple_waits_for_enter() {
    let mut app = offset_app();
    enter(&mut app, "Line w0,0,0 w8,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Offset Cap=Flat");
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([id], SelectionMode::Replace);
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CurveOffset {
            kind: Kind::Offset,
            ..
        })
    ));
    assert!(app.object_prompt.is_none());
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document.selected_object_count(),
        0,
        "{:?}",
        app.command_log
    );
    assert_eq!(app.document.objects().len(), 1);

    enter(&mut app, "OffsetMultiple OffsetCount=2");
    app.select_prompt_objects([id], SelectionMode::Replace);
    assert!(app.object_prompt.is_some());
    enter(&mut app, "");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CurveOffset {
            kind: Kind::Multiple,
            ..
        })
    ));
    enter(&mut app, "w4,4,0");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn offset_viewport_uses_the_current_side_then_commits_its_preview() {
    let mut app = offset_app();
    for input in ["Line w0,0,0 w8,0,0", "SelAll", "Offset", "0.2cm"] {
        enter(&mut app, input);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    let mut strokes = Vec::new();
    for y in [4., -4.] {
        let cursor = screen_point(&app, point(4., y, 0.));
        viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let preview = viewport_frame(&mut app, &context, Vec::new());
            if !preview.is_empty() && preview != strokes {
                strokes = preview;
                break;
            }
            assert!(std::time::Instant::now() < until, "{:?}", app.command_log);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.undo_label(), Some("Line"));
    }
    let cursor = screen_point(&app, point(4., -4., 0.));
    viewport_click(&mut app, &context, cursor);
    let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the committed line");
    };
    assert_eq!(line.start(), point(0., -2., 0.));
    assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
}

#[test]
fn offset_multiple_viewport_draws_every_requested_copy_and_clears_on_escape() {
    let mut app = offset_app();
    for input in [
        "Line w0,0,0 w8,0,0",
        "SelAll",
        "OffsetMultiple OffsetCount=3",
    ] {
        enter(&mut app, input);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    let cursor = screen_point(&app, point(4., 4., 0.));
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
    let mut rows = preview.iter().map(|line| line[0].y).collect::<Vec<_>>();
    rows.sort_by(f32::total_cmp);
    rows.dedup();
    assert_eq!(rows.len(), 3);
    assert_eq!(app.document.objects().len(), 1);
    app.cancel_current_prompt_or_selection();
    assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
    assert_eq!(app.document.undo_label(), Some("Line"));
}

#[test]
fn a_pending_curve_offset_replaces_stale_background_geometry() {
    let mut app = offset_app();
    for input in [
        "Polyline w0,0,0 w8,0,0 w8,6,0",
        "SelAll",
        "Offset Corner=Smooth",
    ] {
        enter(&mut app, input);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    let cursor = screen_point(&app, point(7., -3., 0.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let smooth = loop {
        let preview = viewport_frame(&mut app, &context, Vec::new());
        if preview.len() > 3 {
            break preview;
        }
        assert!(std::time::Instant::now() < until, "{:?}", app.command_log);
        std::thread::sleep(std::time::Duration::from_millis(2));
    };

    enter(&mut app, "Corner=Chamfer");
    let after_change = viewport_frame(&mut app, &context, Vec::new());
    assert!(after_change.is_empty());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let preview = viewport_frame(&mut app, &context, Vec::new());
        if !preview.is_empty() {
            assert_ne!(preview, smooth);
            break;
        }
        assert!(std::time::Instant::now() < until, "{:?}", app.command_log);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(app.document.objects().len(), 1);
    app.cancel_current_prompt_or_selection();
    assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
}
