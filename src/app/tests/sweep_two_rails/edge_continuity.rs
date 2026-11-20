use super::*;
use viboceros_geometry::{Brep, NurbsSurface};

#[test]
fn sweep_two_rail_edge_continuity_uses_direct_picks_and_live_review() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let supports = [
        [[0., 0.], [-1., -0.4], [-2., -0.2]],
        [[4., 0.], [5., -0.3], [6., -0.2]],
    ]
    .map(|cross| {
        let points = [0., 6.]
            .into_iter()
            .flat_map(|y| cross.map(|[x, z]| point(x, y, z)))
            .collect();
        let support = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            points,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let x = cross[0][0];
        let brep = Brep::try_surface_face(support.clone(), tolerance).unwrap();
        let edge = brep
            .edges()
            .iter()
            .position(|e| {
                let c = e.curve();
                c.evaluate(*c.domain().start()).unwrap().x() == x
                    && c.evaluate(*c.domain().end()).unwrap().x() == x
            })
            .unwrap();
        let object = app
            .document
            .add_geometry(Geometry::NurbsSurface(support))
            .unwrap();
        (object, edge)
    });
    let profiles = [(0., 1.), (6., 1.4)].map(|(y, height)| {
        let curve = NurbsCurve::try_new(
            5,
            (0..=5)
                .map(|i| {
                    let t = f64::from(i) / 5.;
                    point(4. * t, y, height * (std::f64::consts::PI * t).sin())
                })
                .collect(),
            vec![0., 0., 0., 0., 0., 0., 1., 1., 1., 1., 1., 1.],
        )
        .unwrap();
        app.document
            .add_geometry(Geometry::NurbsCurve(curve))
            .unwrap()
    });
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    viewport_frame(&mut app, &context, vec![]);
    let rail = screen_point(&app, point(0., 3., 0.));
    viewport_click(&mut app, &context, rail);
    assert!(
        app.command_hint().contains("second rail"),
        "{:?}",
        app.command_log
    );
    let (right, edge) = supports[1];
    enter(&mut app, &format!("Edge={right},{edge}"));
    assert!(!app.picking_sweep_two_rail_edges());
    for object_id in profiles {
        app.apply_selection_click(SelectionClick {
            object_id: Some(object_id),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "");
    assert!(app.command_hint().contains("EdgeContinuity"));
    let base = wait_for_draft(&mut app, &context);

    enter(&mut app, "EdgeContinuity");
    assert!(app.command_hint().contains("EdgeContinuityA"));
    enter(&mut app, "EdgeContinuityA");
    assert!(app.command_hint().contains("Curvature"));
    enter(&mut app, "Tangency");
    let first = wait_for_draft(&mut app, &context);
    assert_ne!(base, first);
    enter(&mut app, "EdgeContinuityB=Curvature");
    let both = wait_for_draft(&mut app, &context);
    assert_ne!(both, first);
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), first);
    enter(&mut app, "EdgeContinuityB=Curvature");
    assert_eq!(wait_for_draft(&mut app, &context), both);
    enter(&mut app, "PreserveEnds");
    assert!(app.command_hint().contains("StartShape"));
    enter(&mut app, "StartShape=No");
    let draft = wait_for_draft(&mut app, &context);
    assert_ne!(draft, both);
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected Sweep2 B-rep");
    };
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), draft);
    let surface = brep.faces()[0].surface();
    assert!(
        (surface.control_point(0, 1).unwrap().point().z() - (std::f64::consts::PI / 5.).sin())
            .abs()
            < 1e-8
    );
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
}
