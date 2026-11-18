use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use serde::Deserialize;
use viboceros_command::cutting_options::Output;

#[derive(Deserialize)]
struct Capture {
    name: String,
    inputs: Vec<String>,
    cuts: Vec<Curve>,
    groups: Vec<usize>,
}

#[derive(Deserialize)]
struct Curve {
    bounds: [[f64; 3]; 2],
    closed: bool,
}

#[test]
fn section_and_contour_staged_runs_match_native_cut_extents_and_history() {
    let captures: Vec<Capture> =
        serde_json::from_str(include_str!("cutting_modes/rhino.json")).unwrap();
    for capture in captures {
        let mut app = test_app();
        app.active_viewport = 0;
        app.viewports[0] = Viewport::new(ViewKind::Top);
        enter(&mut app, "Box w0,0,0 w8,6,0 7");
        let source = app.document.objects().next().unwrap().id();
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
        assert_cut_extents(&app, source, &capture);
        assert!(!app.document.is_selected(source));
        assert_eq!(app.document.selected_object_count(), capture.cuts.len());
        assert_eq!(
            app.document.undo().unwrap().as_deref(),
            Some(capture.inputs[0].as_str())
        );
        assert_eq!(app.document.objects().len(), 1, "{}", capture.name);
        assert!(!app.document.is_selected(source), "{}", capture.name);
        assert_eq!(
            app.document.redo().unwrap().as_deref(),
            Some(capture.inputs[0].as_str())
        );
        assert_cut_extents(&app, source, &capture);
        assert_eq!(
            app.document.selected_object_count(),
            capture.cuts.len(),
            "{} redo",
            capture.name
        );
    }
}

fn assert_cut_extents(app: &VibocerosApp, source: ObjectId, capture: &Capture) {
    let mut curves = app
        .document
        .objects()
        .filter(|object| object.id() != source)
        .map(|object| {
            let curve = object.geometry().curve_ref().unwrap();
            let bounds = object
                .geometry()
                .tight_bounds(app.document.tolerance())
                .unwrap();
            Curve {
                bounds: [bounds.min().to_array(), bounds.max().to_array()],
                closed: curve.is_closed().unwrap(),
            }
        })
        .collect::<Vec<_>>();
    curves.sort_by(|a, b| {
        a.bounds
            .iter()
            .flatten()
            .zip(b.bounds.iter().flatten())
            .map(|(x, y)| x.total_cmp(y))
            .find(|order| !order.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(
        curves.len(),
        capture.cuts.len(),
        "{}: {:?}",
        capture.name,
        app.command_log
    );
    let mut groups = app
        .document
        .groups()
        .map(|group| group.members().len())
        .collect::<Vec<_>>();
    groups.sort_unstable();
    assert_eq!(groups, capture.groups, "{} groups", capture.name);
    for (actual, expected) in curves.iter().zip(&capture.cuts) {
        assert_eq!(actual.closed, expected.closed, "{}", capture.name);
        for (&value, &native) in actual
            .bounds
            .iter()
            .flatten()
            .zip(expected.bounds.iter().flatten())
        {
            assert!(
                (value - native).abs() < 1e-8,
                "{}: {:?} != {:?}",
                capture.name,
                actual.bounds,
                expected.bounds
            );
        }
    }
}

#[test]
fn contour_remembers_only_completed_settings_and_section_remembers_edited_options() {
    let mut app = test_app();
    app.active_viewport = 0;
    app.viewports[0] = Viewport::new(ViewKind::Top);
    for input in [
        "Line -5,0,0 5,0,0",
        "SelAll",
        "Contour",
        "0,0,0",
        "8,0,0",
        "AssignProperties",
        "Output=All",
        "GroupObjectsByContourPlane",
        "1,1,0",
        "3.5,1,0",
    ] {
        enter(&mut app, input);
    }
    let saved = app.commands.contour_preferences();
    assert_eq!(saved.spacing, 2.5);
    assert!(saved.options.input_properties && saved.options.group);
    assert_eq!(saved.options.output, Output::All);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Contour"));
    let source = app.document.objects().next().unwrap().id();
    for input in [
        "SelAll",
        "Contour",
        "Range",
        "AssignProperties",
        "Output",
        "Cancel",
    ] {
        enter(&mut app, input);
    }
    assert_eq!(app.commands.contour_preferences(), saved);
    enter(&mut app, "Contour");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Contour { options, .. }) if !options.range && options == saved.options
    ));
    for input in ["w0,0,0", "w8,0,0", "Enter"] {
        enter(&mut app, input);
    }
    assert_eq!(app.document.objects().len(), 6);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Contour"));

    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    for input in [
        "Section",
        "w0,-1,0",
        "ExtendSection",
        "AssignProperties",
        "Output",
        "Cancel",
    ] {
        enter(&mut app, input);
    }
    let remembered = app.commands.section_options();
    assert!(remembered.extend && remembered.input_properties);
    assert_eq!(remembered.output, Output::All);
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.document.can_redo());
}

fn preview_app() -> VibocerosApp {
    let mut app = test_app();
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.osnap = false;
    app.smart_track = false;
    app.grid_snap = true;
    enter(&mut app, "MeshPlane 0,0,0 5,5,0 XCount=1 YCount=1");
    enter(&mut app, "SelAll");
    app
}

fn wait_for_preview(app: &mut VibocerosApp, context: &egui::Context, count: usize) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let strokes = viewport_frame(app, context, Vec::new());
        if strokes.len() >= count {
            return;
        }
        assert!(
            std::time::Instant::now() < until,
            "preview render: {:?}",
            app.command_log
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn section_viewport_refreshes_the_exact_cut_when_extend_changes() {
    let mut app = preview_app();
    let ctx = egui::Context::default();
    enter(&mut app, "Section");
    enter(&mut app, "w2,1,0");
    viewport_frame(&mut app, &ctx, Vec::new());
    let cursor = screen_point(&app, point(2., 4., 0.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
    wait_for_preview(&mut app, &ctx, 1);
    let sources = app.cutting_sources(app.cutting_draft().unwrap()).unwrap();
    let (_, finite) = app
        .cutting_preview
        .result(&sources, app.document.tolerance())
        .unwrap();
    let finite_length = cut_length(&finite.lines);
    assert!((finite_length - 3.).abs() < 1e-8, "{finite_length}");
    enter(&mut app, "ExtendSection");
    wait_for_preview(&mut app, &ctx, 1);
    let (_, extended) = app
        .cutting_preview
        .result(&sources, app.document.tolerance())
        .unwrap();
    let extended_length = cut_length(&extended.lines);
    assert!((extended_length - 5.).abs() < 1e-8, "{extended_length}");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), Some("MeshPlane"));
    viewport_click(&mut app, &ctx, cursor);
    enter(&mut app, "Cancel");
    assert_mesh_cut_lengths(&app, extended_length);
    assert!(viewport_frame(&mut app, &ctx, Vec::new()).is_empty());
}

#[test]
fn contour_viewport_tracks_spacing_with_live_grid_and_no_model_edits() {
    let mut app = preview_app();
    let ctx = egui::Context::default();
    for input in ["Contour", "w0,0,0", "w5,0,0", "w0,1,0"] {
        enter(&mut app, input);
    }
    viewport_frame(&mut app, &ctx, Vec::new());
    let mut accepted = None;
    for (distance, count) in [(1., 6), (2., 3)] {
        let cursor = screen_point(&app, point(distance, 1., 0.));
        viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
        wait_for_preview(&mut app, &ctx, count);
        let sources = app.cutting_sources(app.cutting_draft().unwrap()).unwrap();
        let (plan, cut) = app
            .cutting_preview
            .result(&sources, app.document.tolerance())
            .unwrap();
        let crate::viewport::CutPlan::Contour(frames) = plan else {
            panic!("expected contour planes")
        };
        assert_eq!(frames.len(), count);
        let length = cut_length(&cut.lines);
        assert!((length - (count as f64 * 5.)).abs() < 1e-8, "{length}");
        assert_eq!(frames[1].origin().x(), distance);
        assert_eq!(app.document.objects().len(), 1);
        accepted = Some((cursor, length));
    }
    let (cursor, length) = accepted.unwrap();
    viewport_click(&mut app, &ctx, cursor);
    assert_eq!(app.document.objects().len(), 4);
    assert_mesh_cut_lengths(&app, length);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Contour"));
}

fn assert_mesh_cut_lengths(app: &VibocerosApp, expected: f64) {
    let length: f64 = app
        .document
        .objects()
        .skip(1)
        .map(|object| match object.geometry() {
            Geometry::Polyline(curve) => curve
                .vertices()
                .windows(2)
                .map(|p| p[0].distance_to(p[1]).unwrap())
                .sum::<f64>(),
            geometry => panic!("expected a mesh section polyline, got {geometry:?}"),
        })
        .sum();
    assert!((length - expected).abs() < 1e-8, "{length} != {expected}");
}

fn cut_length(lines: &[[Point3; 2]]) -> f64 {
    lines.iter().map(|[a, b]| a.distance_to(*b).unwrap()).sum()
}
