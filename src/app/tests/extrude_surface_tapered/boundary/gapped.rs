use super::*;
use crate::app::tests::circle_preview::viewport_click;
use viboceros_geometry::{NurbsCurve, Polyline3};

fn stock_curve(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::Polyline(
            Polyline3::try_new(
                [[0., 0.], [6., 0.], [6., 4.], [0., 4.], [0., 0.]]
                    .map(|[x, y]| point(x, y, 2.))
                    .to_vec(),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}

#[test]
fn curve_taper_boundary_gapped_pick_commits_cut_or_fallback_and_replays() {
    for solid in [false, true] {
        let mut app = test_app();
        let source = stock_curve(&mut app);
        let cutter = rectangle(&mut app, [-9., -9., 15., 13.], 5.);
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(
            &mut app,
            &format!(
                "ExtrudeCrvTapered DraftAngle=-10 Solid={} DeleteInput=Yes",
                if solid { "Yes" } else { "No" }
            ),
        );
        enter(&mut app, "Direction");
        enter(&mut app, "w0,0,0");
        enter(&mut app, "w1,0,6");
        let ctx = top_view(&mut app, [point(-12., -10., 0.), point(18., 15., 8.)]);
        move_pointer(&mut app, &ctx, point(-9., -2., 2.));
        let viewport = &app.viewports[0];
        let preview_x = viewport
            .pointer_ray()
            .unwrap()
            .on_plane(viewport.construction_plane())
            .unwrap()
            .unwrap()
            .x();
        enter(&mut app, "ToBoundary");
        assert!(app.picking_tapered_boundary());
        let pick = move_pointer(&mut app, &ctx, point(14.7, -2., 5.));
        viewport_click(&mut app, &ctx, pick);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert_eq!(
            app.command_log.back().unwrap().starts_with("Error: "),
            solid
        );
        assert!(app.document.object(source).is_none());
        assert!(app.document.is_selected(cutter));
        let Geometry::Brep(piece) = app
            .document
            .objects()
            .find(|o| o.id() != cutter)
            .unwrap()
            .geometry()
        else {
            panic!("expected the drafted result")
        };
        assert_eq!(
            (piece.faces().len(), piece.edges().len()),
            if solid { (5, 19) } else { (1, 4) }
        );
        let height = piece
            .vertices()
            .iter()
            .map(|v| v.point().z())
            .max_by(f64::total_cmp)
            .unwrap();
        let expected_height = if solid {
            2. + (preview_x - 3.).abs() * (6. + 10_f64.to_radians().tan())
        } else {
            5.
        };
        assert!(
            (height - expected_height).abs() < 1e-6,
            "{height} vs {expected_height}"
        );
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [source]
        );
        enter(&mut app, "Redo");
        assert!(app.document.object(source).is_none());
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

#[test]
fn curve_taper_boundary_partial_viewport_pick_preserves_a_side_edge_imprint() {
    let mut app = test_app();
    let source = stock_curve(&mut app);
    let cutter = rectangle(&mut app, [-1., -1., 8., 1.], 5.);
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "ExtrudeCrvTapered DraftAngle=-10 Solid=No DeleteInput=Yes",
    );
    enter(&mut app, "Direction");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,6");
    let context = top_view(&mut app, [point(-12., -10., 0.), point(18., 15., 8.)]);
    move_pointer(&mut app, &context, point(-9., -2., 2.));
    enter(&mut app, "ToBoundary");
    assert!(app.picking_tapered_boundary());
    let pick = move_pointer(&mut app, &context, point(7.8, 0.5, 5.));
    viewport_click(&mut app, &context, pick);
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(app.document.object(source).is_none());
    assert!(app.document.is_selected(cutter));
    let Geometry::Brep(piece) = app
        .document
        .objects()
        .find(|o| o.id() != cutter)
        .unwrap()
        .geometry()
    else {
        panic!("expected the uncut closing wall")
    };
    assert_eq!(
        (
            piece.faces().len(),
            piece.edges().len(),
            piece.vertices().len()
        ),
        (1, 5, 5)
    );
    assert!(piece.vertices().iter().any(|v| {
        v.point()
            .distance_to(point(-0.02815356979892647, 0., 5.))
            .unwrap()
            < 1e-6
    }));
    assert!(piece.vertices().iter().any(|v| v.point().z() > 70.));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [source]
    );
    enter(&mut app, "Redo");
    assert!(app.document.object(source).is_none());
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn curve_taper_boundary_viewport_pick_keeps_the_curved_base_outside_the_footprint() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![point(0., 0., 2.), point(2., 1., 2.), point(4., 0., 2.)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    let cutter = rectangle(&mut app, [0.19, -4., 5., 3.], 5.);
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "ExtrudeCrvTapered DraftAngle=-10 Solid=No DeleteInput=Yes",
    );
    enter(&mut app, "Direction");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,6");
    let context = top_view(&mut app, [point(-12., -10., 0.), point(18., 15., 8.)]);
    move_pointer(&mut app, &context, point(2.9, -8.9, 2.));
    enter(&mut app, "ToBoundary");
    assert!(app.picking_tapered_boundary());
    let pick = move_pointer(&mut app, &context, point(2.6, -3.8, 5.));
    viewport_click(&mut app, &context, pick);
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(
        app.document.object(source).is_none(),
        "{:?}",
        app.command_log
    );
    let Geometry::Brep(piece) = app
        .document
        .objects()
        .find(|o| o.id() != cutter)
        .unwrap()
        .geometry()
    else {
        panic!("expected the cut curved wall");
    };
    assert_eq!((piece.faces().len(), piece.edges().len()), (1, 4));
    let tolerance = app.document.tolerance();
    for expected in [point(0., 0., 2.), point(4., 0., 2.)] {
        assert!(
            piece
                .vertices()
                .iter()
                .any(|v| v.point().is_near(expected, tolerance))
        );
    }
}
