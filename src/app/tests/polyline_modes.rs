use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use serde::Deserialize;
use viboceros_geometry::{CurveSegment3, Vector3};

#[derive(Deserialize)]
struct Capture {
    name: String,
    inputs: Vec<String>,
    segments: Vec<NativeSegment>,
    plane: Option<[[f64; 3]; 3]>,
}

#[derive(Debug, Deserialize)]
struct NativeSegment {
    kind: String,
    domain: [f64; 2],
    start: [f64; 3],
    mid: [f64; 3],
    end: [f64; 3],
}

#[test]
fn polyline_scripted_and_staged_segments_match_rhino_geometry_and_domains() {
    let cases: Vec<Capture> =
        serde_json::from_str(include_str!("polyline_modes/rhino.json")).unwrap();
    for case in cases {
        let mut app = test_app();
        app.active_viewport = 0;
        app.viewports[0] = Viewport::new(ViewKind::Top);
        if let Some([origin, x, y]) = case.plane {
            let plane = Frame3::try_from_directions(
                Point3::try_from(origin).unwrap(),
                Vector3::try_from(x).unwrap(),
                Vector3::try_from(y).unwrap(),
                app.document.tolerance(),
            )
            .unwrap();
            assert!(app.viewports[0].set_construction_plane(plane));
        }
        enter(&mut app, &format!("Polyline {}", case.inputs.join(" ")));
        check_native_segments(&app, &case);
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Polyline"));
        app.commands
            .remember_polyline_preferences(Default::default());

        enter(&mut app, "PLine");
        for input in &case.inputs {
            if input != "Enter" || app.polyline_session.is_some() {
                assert_eq!(app.document.objects().len(), 0, "{}", case.name);
                enter(&mut app, input);
            }
        }
        assert!(
            app.polyline_session.is_none(),
            "{}: {:?}",
            case.name,
            app.command_log
        );
        check_native_segments(&app, &case);
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Polyline"));
        assert!(app.document.objects().next().is_none());
        assert_eq!(app.document.redo().unwrap().as_deref(), Some("Polyline"));
        check_native_segments(&app, &case);
    }
}

fn check_native_segments(app: &VibocerosApp, case: &Capture) {
    assert_eq!(
        app.document.objects().len(),
        1,
        "{}: {:?}",
        case.name,
        app.command_log
    );
    let geometry = app.document.objects().next().unwrap().geometry();
    let segments: Vec<_> = match geometry {
        Geometry::Polyline(c) => c
            .vertices()
            .windows(2)
            .zip(c.parameters().windows(2))
            .map(|(points, domain)| {
                (
                    "Line",
                    [domain[0], domain[1]],
                    [points[0], points[0].midpoint(points[1]).unwrap(), points[1]],
                )
            })
            .collect(),
        Geometry::PolyCurve(c) => c
            .segments()
            .iter()
            .map(|s| {
                let class = match s {
                    CurveSegment3::Line(_) => "Line",
                    CurveSegment3::Arc(_) => "Arc",
                    _ => panic!("unexpected segment in {:?}", case.name),
                };
                (
                    class,
                    [*s.domain().start(), *s.domain().end()],
                    [0., 0.5, 1.].map(|t| s.evaluate(s.parameter_at(t).unwrap()).unwrap()),
                )
            })
            .collect(),
        _ => panic!("unexpected Polyline geometry in {:?}", case.name),
    };
    assert_eq!(segments.len(), case.segments.len(), "{}", case.name);
    for ((kind, domain, points), expected) in segments.iter().zip(&case.segments) {
        assert_eq!(*kind, expected.kind, "{}", case.name);
        for (&parameter, native) in domain.iter().zip(expected.domain) {
            assert!(
                (parameter - native).abs() < 1e-8,
                "{}: {domain:?} != {:?}",
                case.name,
                expected.domain
            );
        }
        for (&point, coordinates) in points
            .iter()
            .zip([expected.start, expected.mid, expected.end])
        {
            let native = Point3::try_from(coordinates).unwrap();
            assert!(
                point.distance_to(native).unwrap() < 1e-8,
                "{}: {point:?} != {native:?}",
                case.name
            );
        }
    }
}

fn viewport_app() -> VibocerosApp {
    let mut app = test_app();
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.osnap = false;
    app.grid_snap = false;
    app.smart_track = false;
    app
}

#[test]
fn polyline_helper_tracking_and_tab_share_the_live_tangent_arc() {
    let mut app = viewport_app();
    for input in ["Polyline", "0", "w2,0,0", "Mode=Arc", "Helpers=Yes"] {
        enter(&mut app, input);
    }
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    let near = screen_point(&app, point(4., 0.03, 0.));
    let far = screen_point(&app, point(4., 2., 0.));
    let line = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(near)]);
    let arc = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(far)]);
    assert_eq!(line.len(), 2);
    assert!(arc.len() > 2);
    let tab = |pressed| egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    viewport_frame(&mut app, &ctx, vec![tab(true), tab(false)]);
    assert!(
        app.polyline_session
            .as_ref()
            .unwrap()
            .direction_lock
            .is_some()
    );
    let locked = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(near)]);
    assert!(locked.len() > 2);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &ctx, near);
    assert!(
        app.polyline_session
            .as_ref()
            .unwrap()
            .direction_lock
            .is_none()
    );
    enter(&mut app, "");
    let Geometry::PolyCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected a retained arc after the direction lock");
    };
    let CurveSegment3::Arc(arc) = curve.segments()[1] else {
        panic!("expected an arc");
    };
    let end = arc.point_at(1.).unwrap();
    assert!(end.y() > 0.8);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Polyline"));
}

#[test]
fn polyline_persistent_close_and_locked_line_keep_preview_edit_free() {
    let mut app = viewport_app();
    for input in ["Polyline PersistentClose=Yes", "0", "w2,0,0", "2mm"] {
        enter(&mut app, input);
    }
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(3., 1., 0.));
    let b = screen_point(&app, point(2.5, 2., 0.));
    let first = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let last = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert_eq!(first.len(), 3);
    assert_eq!(last.len(), 3);
    assert_ne!(first, last);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &ctx, b);
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "");
    let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an all-line polyline");
    };
    assert!(polyline.is_closed());
    assert_eq!(polyline.segment_count(), 3);
    assert!(
        (polyline.vertices()[1]
            .distance_to(polyline.vertices()[2])
            .unwrap()
            - 2.)
            .abs()
            < 1e-9
    );
}
