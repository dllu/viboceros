use super::handle_selection::{begin, click, preview_when};
use super::*;
use crate::app::tests::circle_preview::screen_point;
use crate::viewport::edge_blend::{HandleLocation, SizeHandle};

fn interior(app: &VibocerosApp) -> Vec<SizeHandle> {
    app.edge_blend_size_handles()
        .into_iter()
        .filter(|handle| matches!(handle.id.location, HandleLocation::Interior(_)))
        .collect()
}

fn handle_at(app: &VibocerosApp, position: Point3) -> SizeHandle {
    interior(app)
        .into_iter()
        .find(|handle| handle.point.distance_to(position).is_ok_and(|d| d < 1e-6))
        .expect("expected a handle at the requested location")
}

#[test]
fn edge_fillet_accepts_cartesian_relative_and_copied_handle_locations() {
    for command in [
        "FilletEdge 1",
        "ChamferEdge 1 RailType=RollingBall",
        "BlendEdge 1",
    ] {
        let (mut app, context, first, second) = begin(command);
        let original = app
            .document
            .object(first.object)
            .unwrap()
            .geometry()
            .clone();
        let plane = app.viewports[0]
            .construction_plane()
            .with_origin(point(1., 2., 0.));
        app.viewports[0].set_construction_plane(plane);
        let current = if command.starts_with("ChamferEdge") {
            "CurrentChamferDistance"
        } else {
            "CurrentRadius"
        };
        submit(&mut app, "AddHandle");
        submit(&mut app, &format!("{current}=1"));
        submit(&mut app, "3mm,0");
        let first_position = point(4., 0., 0.);
        let source = handle_at(&app, first_position);
        assert_eq!(source.id.pick, first);
        assert_eq!(source.size, 1.);

        for rejected in ["w0,0,0", "w4,5,5", "w20,0,0"] {
            submit(&mut app, rejected);
            assert!(
                app.command_log.back().unwrap().starts_with("Error:"),
                "{:?}",
                app.command_log
            );
            assert_eq!(interior(&app).len(), 1);
            assert!(app.last_point.unwrap().distance_to(first_position).unwrap() < 1e-6);
        }
        submit(&mut app, &format!("{current}=1.4"));
        submit(&mut app, "rw2,10,10");
        let other = handle_at(&app, point(6., 10., 10.));
        assert_eq!((other.id.pick, other.size), (second, 1.4));
        submit(&mut app, "");
        assert!(app.adjusting_edge_blend());

        submit(&mut app, "CopyHandle");
        let cursor = screen_point(&app, source.point);
        click(&mut app, &context, cursor, egui::Modifiers::NONE);
        submit(&mut app, "w7,0,0");
        let copied = handle_at(&app, point(7., 0., 0.));
        assert_eq!((copied.id.pick, copied.size), (first, 1.));
        assert_eq!(interior(&app).len(), 3);
        assert!(!app.document.can_undo());
        assert_eq!(
            app.document.object(first.object).unwrap().geometry(),
            &original
        );

        submit(&mut app, "");
        assert!(app.adjusting_edge_blend());
        submit(&mut app, "");
        assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
        let recipe = app
            .document
            .object(first.object)
            .unwrap()
            .geometry_snapshot()
            .edge_blend_recipe()
            .unwrap();
        assert_eq!(
            recipe
                .edges
                .iter()
                .map(|(_, profile)| profile.interior.len())
                .sum::<usize>(),
            3
        );
        assert!(app.document.can_undo());
    }
}

#[test]
fn edge_fillet_typed_handle_updates_and_commits_the_staged_preview() {
    let (mut app, context, first, _) = begin("FilletEdge 1 Preview=Yes");
    let original = app
        .document
        .object(first.object)
        .unwrap()
        .geometry()
        .clone();
    let baseline = preview_when(&mut app, &context, |_| true);
    submit(&mut app, "AddHandle");
    submit(&mut app, "CurrentRadius=1.6");
    submit(&mut app, "w4,0,0");
    assert_eq!(handle_at(&app, point(4., 0., 0.)).size, 1.6);
    let staged = preview_when(&mut app, &context, |curves| curves != baseline);
    assert!(!app.document.can_undo());
    assert_eq!(
        app.document.object(first.object).unwrap().geometry(),
        &original
    );
    submit(&mut app, "");
    submit(&mut app, "");
    let Geometry::Brep(solid) = app.document.object(first.object).unwrap().geometry() else {
        unreachable!()
    };
    assert!(solid.is_solid());
    assert_eq!(solid.edges().len(), staged.len());
    assert!(
        solid
            .edges()
            .iter()
            .zip(&staged)
            .all(|(edge, curve)| edge.curve() == curve)
    );
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
}
