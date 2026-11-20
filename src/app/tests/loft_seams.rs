use super::circle_preview::{screen_point, viewport_frame};
use super::curve_fillet::enter;
use super::*;
use std::time::{Duration, Instant};
use viboceros_geometry::NurbsCurve;

fn wait_for_wires(
    app: &mut VibocerosApp,
    context: &egui::Context,
    ready: impl Fn(&[NurbsCurve]) -> bool,
) -> Vec<NurbsCurve> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        viewport_frame(app, context, vec![]);
        let wires = app.loft_prompt.as_ref().unwrap().preview_wires();
        if ready(wires) {
            return wires.to_vec();
        }
        assert!(Instant::now() < deadline, "Loft seam wires did not update");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn connects(curve: &NurbsCurve, first: Point3, second: Point3) -> bool {
    let domain = curve.domain();
    let start = curve.evaluate(*domain.start()).unwrap();
    let end = curve.evaluate(*domain.end()).unwrap();
    let near = |a: Point3, b| a.distance_to(b).unwrap() < 1e-5;
    (near(start, first) && near(end, second)) || (near(start, second) && near(end, first))
}

#[test]
fn closed_loft_uses_dragged_seams_in_the_rendered_and_committed_surface() {
    let mut app = test_app();
    for text in ["Circle 0,0,0 2", "Circle 5,0,5 2"] {
        app.commands.execute(&mut app.document, text).unwrap();
    }
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.restore_curve_selection(ids.clone());
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);

    enter(&mut app, "Loft");
    assert_eq!(app.loft_prompt.as_ref().unwrap().seam_markers().len(), 2);
    enter(&mut app, "Natural");
    enter(&mut app, "SnapToKnots=No");
    let context = egui::Context::default();
    let original = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    let from = screen_point(&app, point(2., 0., 0.));
    let target = point(0., 2., 0.);
    let to = screen_point(&app, target);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(to)]);
    assert!(
        app.loft_prompt.as_ref().unwrap().seam_markers()[0]
            .point
            .distance_to(target)
            .unwrap()
            < 1e-5
    );
    let opposite = app.loft_prompt.as_ref().unwrap().seam_markers()[1].point;
    let dragged = wait_for_wires(&mut app, &context, |wires| {
        wires != original && wires.iter().any(|wire| connects(wire, target, opposite))
    });
    assert_eq!(app.document.undo_label(), None);
    viewport_frame(&mut app, &context, vec![button(to, false)]);

    enter(&mut app, "");
    assert_eq!(app.loft_prompt.as_ref().unwrap().preview_wires(), dragged);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "Type=Straight");
    let preview = app.loft_prompt.as_ref().unwrap().preview_wires().to_vec();
    assert!(!preview.is_empty(), "{:?}", app.command_log);
    enter(&mut app, "AlignCurves");
    wait_for_wires(&mut app, &context, |wires| wires == preview);
    enter(&mut app, "");
    assert_eq!(app.loft_prompt.as_ref().unwrap().preview_wires(), preview);

    enter(&mut app, "");
    let Geometry::Brep(output) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft preview must commit its B-rep");
    };
    let surface = output.faces()[0].surface();
    assert!(
        [*surface.domain_u().start(), *surface.domain_u().end()]
            .into_iter()
            .any(|u| {
                surface
                    .evaluate(u, *surface.domain_v().start())
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
                    < 1e-5
            })
    );
    assert_eq!(
        preview,
        output
            .wireframe_curves(1, app.document.tolerance())
            .unwrap()
    );
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), ids);
    assert_eq!(app.document.undo_label(), Some("Loft"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn loft_can_move_the_seam_of_one_typed_edge_without_editing_the_owner() {
    use viboceros_geometry::{Brep, NurbsSurface};

    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let cylinder =
        NurbsSurface::try_cylinder(app.viewports[0].construction_plane(), 2., 0., 7.).unwrap();
    let brep = Brep::try_surface_face(cylinder, tolerance).unwrap();
    let edges = [0., 7.].map(|z| {
        brep.edges()
            .iter()
            .position(|edge| {
                let curve = edge.curve();
                curve.is_closed().unwrap()
                    && (curve.evaluate(*curve.domain().start()).unwrap().z() - z).abs() < 1e-8
            })
            .unwrap()
    });
    let id = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    let original = app.document.object(id).unwrap().geometry_snapshot().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Loft");
    for edge in edges {
        enter(&mut app, &format!("Edge={id},{edge}"));
    }
    enter(&mut app, "");
    assert_eq!(app.loft_prompt.as_ref().unwrap().seam_markers().len(), 2);
    enter(&mut app, "Natural");
    let context = egui::Context::default();
    let aligned = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    for edge in edges {
        enter(&mut app, "Flip");
        enter(&mut app, &format!("Edge={id},{edge}"));
    }
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != aligned
    });
    enter(&mut app, "Natural");
    wait_for_wires(&mut app, &context, |wires| wires == aligned);

    enter(&mut app, &format!("Edge={id},{}", edges[1]));
    enter(&mut app, "w0,2,7");
    let expected = point(0., 2., 7.);
    let [lower, upper] = app
        .loft_prompt
        .as_ref()
        .unwrap()
        .seam_markers()
        .try_into()
        .unwrap();
    assert!(upper.point.distance_to(expected).unwrap() < 1e-8);
    let edited = wait_for_wires(&mut app, &context, |wires| {
        wires != aligned
            && wires
                .iter()
                .any(|wire| connects(wire, lower.point, expected))
    });
    enter(&mut app, "Natural");
    wait_for_wires(&mut app, &context, |wires| wires == aligned);
    enter(&mut app, &format!("Edge={id},{}", edges[1]));
    enter(&mut app, "w0,2,7");
    // Accept the current edit before another viewport frame runs the worker.
    enter(&mut app, "");
    assert_eq!(app.loft_prompt.as_ref().unwrap().preview_wires(), edited);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(out) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft B-rep")
    };
    assert!(
        out.vertices()
            .iter()
            .any(|vertex| vertex.point().distance_to(expected).unwrap() < 1e-8)
    );
    assert!(original.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
}
