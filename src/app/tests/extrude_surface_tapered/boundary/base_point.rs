use super::*;
use crate::app::tests::circle_preview::viewport_click;

#[test]
fn curve_taper_boundary_pick_resolves_the_pull_side_from_the_base_point() {
    for holed in [false, true] {
        let mut app = test_app();
        enter(&mut app, "Circle 0,0,0 3");
        let source = app.document.objects().next().unwrap().id();
        let outer = [-9., -9., 15., 13.];
        let cutter = if holed {
            holed_rectangle(&mut app, outer, [4.45, -0.25, 5.1, 0.25], 5.).0
        } else {
            rectangle(&mut app, outer, 5.)
        };
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let context = top_view(&mut app, [point(-12., -10., 0.), point(18., 15., 8.)]);
        start_boundary(&mut app, &context, holed, !holed);
        let left = move_pointer(&mut app, &context, point(-8.8, -2., 5.));
        viewport_click(&mut app, &context, left);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.command_log.back().unwrap().starts_with("Error: "));
        assert!(app.document.is_selected(cutter));
        assert_eq!(app.document.objects().len(), if holed { 1 } else { 2 });
        if holed {
            assert!(app.document.object(source).is_none());
            enter(&mut app, "Undo");
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                [source]
            );
            enter(&mut app, "Redo");
            assert!(app.document.object(source).is_none());
            assert_eq!(app.document.selected_object_count(), 0);
            enter(&mut app, "Undo");
        } else {
            assert!(app.document.is_selected(source));
            assert!(!app.document.can_undo());
        }

        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let preview_x = start_boundary(&mut app, &context, true, !holed);
        let right = move_pointer(&mut app, &context, point(14.4, -2., 5.));
        viewport_click(&mut app, &context, right);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(source).is_none());
        let Geometry::Brep(piece) = app
            .document
            .objects()
            .find(|object| object.id() != cutter)
            .unwrap()
            .geometry()
        else {
            panic!("expected the drafted result");
        };
        assert_eq!(
            (piece.faces().len(), piece.edges().len()),
            if holed { (5, 12) } else { (6, 12) }
        );
        assert_eq!(piece.is_solid(), !holed);
        let height = piece
            .vertices()
            .iter()
            .map(|vertex| vertex.point().z())
            .max_by(f64::total_cmp)
            .unwrap();
        let expected = if holed {
            (preview_x - 7.).abs() * (6. + 10_f64.to_radians().tan())
        } else {
            5.
        };
        assert!((height - expected).abs() < 1e-6, "{height} vs {expected}");
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [source]
        );
        enter(&mut app, "Redo");
        assert!(app.document.object(source).is_none());
    }
}

fn start_boundary(
    app: &mut VibocerosApp,
    context: &egui::Context,
    delete_input: bool,
    pick_base: bool,
) -> f64 {
    let delete_input = if delete_input { "Yes" } else { "No" };
    enter(
        app,
        &format!(
            "ExtrudeCrvTapered Direction=1,0,6 DraftAngle=-10 Solid=Yes DeleteInput={delete_input}"
        ),
    );
    enter(app, "SetBasePoint");
    if pick_base {
        let base = point(7., 1., 3.);
        assert!(app.handle_viewport_action(ViewportOutput {
            picked_point: Some(base),
            ..Default::default()
        }));
        let constraint = app.tapered_distance_constraint().unwrap();
        assert_eq!(constraint.anchor, base);
    } else {
        enter(app, "w7,1,3");
    }
    move_pointer(app, context, point(14.8, -2., 0.));
    let viewport = &app.viewports[0];
    let preview_x = viewport
        .pointer_ray()
        .unwrap()
        .on_plane(viewport.construction_plane())
        .unwrap()
        .unwrap()
        .x();
    enter(app, "ToBoundary");
    preview_x
}
