use super::*;
use viboceros_geometry::SurfaceIso;

#[test]
fn sweep_miter_review_edits_the_preview_and_commits_the_selected_face_trims() {
    let mut app = test_app();
    for input in ["Polyline 0,0,0 5,0,0 5,5,0", "Line 0,0,0 0,1.5,1"] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    assert!(app.command_hint().contains("UntrimmedMiters"));
    let trimmed = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "_UntrimmedMiters");
    enter(&mut app, "_Yes");
    let untrimmed = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != trimmed
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == trimmed);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "UntrimmedMiters=Yes");
    wait_for_wires(&mut app, &context, |wires| wires == untrimmed);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a committed Sweep1 surface: {:?}", app.command_log);
    };
    // Rhino 8.32 joins the two natural faces at the bisector cross section.
    assert_eq!(
        (
            sweep.faces().len(),
            sweep.edges().len(),
            sweep.vertices().len()
        ),
        (2, 7, 6)
    );
    for expected in [
        point(0., 0., 0.),
        point(0., 1.5, 1.),
        point(5., 0., 0.),
        point(3.5, 1.5, 1.),
        point(5., 5., 0.),
        point(3.5, 5., 1.),
    ] {
        assert!(
            sweep
                .vertices()
                .iter()
                .any(|vertex| vertex.point().distance_to(expected).unwrap() < 1e-8)
        );
    }
    assert!(
        sweep
            .faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .all(|t| t.iso() != SurfaceIso::NotIso)
    );
    assert!((sweep.area(tolerance).unwrap() - 15.323592920721954).abs() < 1e-7);
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), untrimmed);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}
