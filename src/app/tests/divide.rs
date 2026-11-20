use super::circle_preview::{screen_point, viewport_frame};
use super::curve_fillet::enter;
use super::curve_match::click_world;
use super::*;

mod portions;

fn assert_points(actual: &[Point3], expected: &[Point3]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            actual.distance_to(*expected).unwrap() < 1e-5,
            "expected {expected:?}, got {actual:?}"
        );
    }
}

fn top_view(app: &mut VibocerosApp, context: &egui::Context) {
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    for _ in 0..3 {
        viewport_frame(app, context, vec![]);
    }
}

#[test]
fn divide_viewport_pick_reverses_one_curve_and_recomputes_markers() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 10,0");
    enter(&mut app, "Line 0,5 10,5");
    enter(&mut app, "SelAll");
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "Divide");
    enter(&mut app, "Length 3 MarkEnds=Yes GroupOutput=Yes");

    let context = egui::Context::default();
    top_view(&mut app, &context);
    click_world(&mut app, &context, point(7., 5., 0.));
    let preview = app.divide_preview_points().unwrap();
    assert_points(
        &preview
            .iter()
            .copied()
            .filter(|p| p.y() == 5.)
            .collect::<Vec<_>>(),
        &[
            point(1., 5., 0.),
            point(4., 5., 0.),
            point(7., 5., 0.),
            point(10., 5., 0.),
        ],
    );
    assert_points(
        &preview
            .iter()
            .copied()
            .filter(|p| p.y() == 0.)
            .collect::<Vec<_>>(),
        &[
            point(0., 0., 0.),
            point(3., 0., 0.),
            point(6., 0., 0.),
            point(9., 0., 0.),
        ],
    );
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert!(!app.document.can_undo());

    enter(&mut app, "7,5,0");
    assert!(
        app.divide_preview_points()
            .unwrap()
            .contains(&point(0., 5., 0.))
    );
    enter(&mut app, "7,5,0");
    enter(&mut app, "NumberSegments");
    assert_eq!(app.divide_preview_points().unwrap().len(), 4);
    enter(&mut app, "Length");
    assert_points(&app.divide_preview_points().unwrap(), &preview);
    enter(&mut app, "");
    assert_eq!(app.document.groups().count(), 2);
    assert_eq!(app.document.objects().len(), 10);
    assert_eq!(app.document.undo_label(), Some("Divide"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn divide_chords_follow_natural_traversal_across_polyline_corners() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0 4,0 4,6",
        "SelAll",
        "Divide",
        "EqualChordLength 3",
    ] {
        enter(&mut app, input);
    }
    let context = egui::Context::default();
    top_view(&mut app, &context);
    click_world(&mut app, &context, point(4., 5., 0.));
    let preview = app.divide_preview_points().unwrap();
    let offset = 8.0_f64.sqrt();
    assert_points(
        &preview,
        &[
            point(0., 0., 0.),
            point(3., 0., 0.),
            point(4., offset, 0.),
            point(4., 3. + offset, 0.),
        ],
    );
    enter(&mut app, "");
    let committed = app
        .document
        .objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Point(point) => Some(*point),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_points(&committed, &preview);
}

#[test]
fn divide_backward_length_split_keeps_native_piece_domains() {
    let mut app = test_app();
    for input in [
        "Line 0,0 10,0",
        "SelAll",
        "Divide",
        "Length 3 Split=Yes",
        "9,0,0",
    ] {
        enter(&mut app, input);
    }
    let piece_ends = [1., 4., 4., 7., 7., 10.].map(|x| point(x, 0., 0.));
    assert_points(&app.divide_preview_points().unwrap(), &piece_ends);
    enter(&mut app, "13");
    assert_points(
        &app.divide_preview_points().unwrap(),
        &[point(0., 0., 0.), point(10., 0., 0.)],
    );
    enter(&mut app, "3");
    enter(&mut app, "DeleteRemainder=Yes");
    assert_points(&app.divide_preview_points().unwrap(), &piece_ends);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let ranges = [1.0..=4., 4.0..=7., 7.0..=10.];
    assert_eq!(app.document.objects().len(), ranges.len());
    for (piece, range) in app.document.objects().zip(ranges) {
        let curve = piece.geometry().curve_ref().unwrap();
        let domain = curve.domain();
        assert!((domain.start() - range.start()).abs() < 1e-10);
        assert!((domain.end() - range.end()).abs() < 1e-10);
        assert_points(
            &[curve.start_point().unwrap(), curve.end_point().unwrap()],
            &[point(*range.start(), 0., 0.), point(*range.end(), 0., 0.)],
        );
    }
}

#[test]
fn divide_closed_curve_drag_and_flip_match_native_length_stations() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0 5");
    enter(&mut app, "SelAll");
    let source = app.document.selected_objects().next().unwrap().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Divide");
    let context = egui::Context::default();
    top_view(&mut app, &context);

    let from = screen_point(&app, point(5., 0., 0.));
    let to = screen_point(&app, point(0., 7., 0.));
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let markers = |app: &VibocerosApp| {
        app.object_prompt
            .as_ref()
            .unwrap()
            .divide
            .as_ref()
            .unwrap()
            .seam_editor(&app.document)
            .unwrap()
            .markers()
    };
    assert_points(&[markers(&app)[0].point], &[point(5., 0., 0.)]);
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(to)]);
    assert_points(&[markers(&app)[0].point], &[point(0., 5., 0.)]);
    viewport_frame(&mut app, &context, vec![button(to, false)]);
    enter(&mut app, "Flip");
    click_world(&mut app, &context, point(0., 5., 0.));
    assert!(markers(&app)[0].tangent.unwrap().x() > 0.);
    enter(&mut app, "");
    enter(&mut app, "4");
    assert_points(
        &app.divide_preview_points().unwrap(),
        &[
            point(-5., 0., 0.),
            point(0., -5., 0.),
            point(5., 0., 0.),
            point(0., 5., 0.),
        ],
    );
    enter(&mut app, "EqualChordLength 3");
    let chord_preview = app.divide_preview_points().unwrap();
    let chord_step = 2. * 0.3_f64.asin();
    let chord_expected = (0..=10)
        .map(|n| {
            let angle = std::f64::consts::FRAC_PI_2 + chord_step * f64::from(n);
            point(5. * angle.cos(), 5. * angle.sin(), 0.)
        })
        .collect::<Vec<_>>();
    assert_points(&chord_preview, &chord_expected);
    enter(&mut app, "Length 3 MarkEnds=Yes");
    let preview = app.divide_preview_points().unwrap();
    let expected = (0..=10)
        .rev()
        .map(|n| {
            let angle = std::f64::consts::FRAC_PI_2 - 0.6 * f64::from(n);
            point(5. * angle.cos(), 5. * angle.sin(), 0.)
        })
        .collect::<Vec<_>>();
    assert_points(&preview, &expected);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 12);
    assert_eq!(app.document.object(source.id()), Some(&source));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn divide_closed_typed_seam_split_keeps_increasing_shifted_domains() {
    let mut app = test_app();
    for input in ["Circle 0,0 5", "SelAll", "Divide"] {
        enter(&mut app, input);
    }
    let seam = 5. * std::f64::consts::FRAC_PI_2;
    enter(&mut app, &format!("Parameter={seam}"));
    enter(&mut app, "Natural");
    enter(&mut app, &format!("Parameter={seam}"));
    for input in ["Flip", "0,5,0", "", "Length 3 Split=Yes", ""] {
        enter(&mut app, input);
    }

    let end = seam + 5. * std::f64::consts::TAU;
    assert_eq!(app.document.objects().len(), 10);
    for (index, piece) in app.document.objects().enumerate() {
        let curve = piece.geometry().curve_ref().unwrap();
        let lo = end - 30. + 3. * index as f64;
        let hi = lo + 3.;
        assert!((curve.domain().start() - lo).abs() < 1e-5);
        assert!((curve.domain().end() - hi).abs() < 1e-5);
        let on_circle = |parameter: f64| {
            let angle = parameter / 5.;
            point(5. * angle.cos(), 5. * angle.sin(), 0.)
        };
        assert_points(
            &[curve.start_point().unwrap(), curve.end_point().unwrap()],
            &[on_circle(lo), on_circle(hi)],
        );
    }
}
