use super::*;
use crate::app::tests::circle_preview::{screen_point, viewport_click, viewport_frame};

pub(super) fn top_app() -> VibocerosApp {
    let mut app = test_app();
    app.active_viewport = 0;
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.smart_track = false;
    app.grid_snap = true;
    app
}

fn warm(app: &mut VibocerosApp, ctx: &egui::Context) {
    viewport_frame(app, ctx, Vec::new());
    viewport_frame(app, ctx, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(app, ctx, Vec::new());
}

#[test]
fn align_drafts_picked_layout_and_curve_projection_without_history() {
    for to_curve in [false, true] {
        let mut app = top_app();
        let ctx = egui::Context::default();
        for command in [
            "Point w0,0,0",
            "Point w8,6,2",
            "SelAll",
            "Line w0,3,0 w8,3,0",
        ] {
            enter(&mut app, command);
        }
        let sources = app
            .document
            .objects()
            .take(2)
            .map(|o| o.id())
            .collect::<Vec<_>>();
        app.document
            .select_objects_direct(sources.iter().copied(), SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        warm(&mut app, &ctx);
        enter(
            &mut app,
            if to_curve {
                "Align ToCurve"
            } else {
                "Align Bottom"
            },
        );
        viewport_frame(&mut app, &ctx, Vec::new());
        let cursor = screen_point(&app, point(4., 3., 0.));
        viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
        let layout = app
            .align_session
            .as_ref()
            .unwrap()
            .preview()
            .expect("layout preview");
        let translations = layout.translations().collect::<Vec<_>>();
        assert_eq!(translations.len(), 2);
        assert!(!app.document.can_undo());
        let original = positions(&app);
        viewport_click(&mut app, &ctx, cursor);
        assert!(app.command_line_idle(), "{:?}", app.command_log);
        for (id, transform) in translations {
            let index = sources.iter().position(|&source| source == id).unwrap();
            let Geometry::Point(source) = original[index] else {
                panic!("source point")
            };
            assert_eq!(
                app.document.object(id).unwrap().geometry(),
                &Geometry::Point(transform.transform_point(source).unwrap())
            );
        }
        app.document.undo().unwrap();
        assert_eq!(positions(&app), original);
    }
}

#[test]
fn three_point_plane_shows_the_typed_width_before_accepting_direction() {
    let mut app = top_app();
    let ctx = egui::Context::default();
    for command in ["Point w1,2,4", "Point w8,6,2", "SelAll"] {
        enter(&mut app, command);
    }
    app.document.clear_history().unwrap();
    warm(&mut app, &ctx);
    for input in ["Align ToPlane 3Point", "w0,0,0", "w8,0,4", "2"] {
        enter(&mut app, input);
    }
    assert!(!app.command_line_idle());
    let before = positions(&app);
    let cursor = screen_point(&app, point(4., 4., 0.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
    let session = app.align_session.as_ref().unwrap();
    let corners = session.rectangle_preview().unwrap().corners;
    assert!((corners[1].distance_to(corners[2]).unwrap() - 2.).abs() < 1e-12);
    let transforms = session
        .preview()
        .unwrap()
        .translations()
        .collect::<Vec<_>>();
    assert_eq!(transforms.len(), 2);
    assert!(!app.document.can_undo());
    let expected = transforms
        .into_iter()
        .map(|(id, transform)| {
            let Geometry::Point(source) = *app.document.object(id).unwrap().geometry() else {
                panic!("source point")
            };
            (
                id,
                Geometry::Point(transform.transform_point(source).unwrap()),
            )
        })
        .collect::<Vec<_>>();
    viewport_click(&mut app, &ctx, cursor);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    for (id, expected) in expected {
        assert_eq!(app.document.object(id).unwrap().geometry(), &expected);
    }
    app.document.undo().unwrap();
    assert_eq!(positions(&app), before);
}
