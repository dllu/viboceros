use super::*;

#[test]
fn sweep_trimmed_preview_joins_intersection_spans_across_a_profile_crease() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Polyline 0,0,0 0,1,1 0,2,2.5",
        "Polyline 5,5,0 4.2,5,1 3.4,5,2.5",
    ] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(sources.len(), 3);
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1 UntrimmedMiters=No");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let corner = point(4.127459798, 0.925910491, 1.);
    let trimmed =
        wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(30), |wires| {
            wires.iter().any(|wire| {
                let domain = wire.domain();
                [*domain.start(), *domain.end()]
                    .into_iter()
                    .any(|u| wire.evaluate(u).unwrap().distance_to(corner).unwrap() < 1e-6)
            })
        });
    enter(&mut app, "UntrimmedMiters=Yes");
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != trimmed
    });
    enter(&mut app, "Undo");
    wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(30), |wires| {
        wires == trimmed
    });
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(result) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a joined Sweep1 BRep: {:?}", app.command_log);
    };
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!((result.faces().len(), result.edges().len()), (4, 12));
    assert_eq!(result.wireframe_curves(1, tolerance).unwrap(), trimmed);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_refit_review_keeps_the_section_domain_and_ends_creases_at_the_miter() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Polyline 0,0,0 0,1,0 0,1,0.5 0,0,0.5 0,0,0",
        "Curve 5,0,0 4.398959235991435,0.6010407640085654,0.85 5.601040764008565,-0.6010407640085654,0.85 5,0,0",
        "Curve 5,5,0 4.3,5,0.7 5.7,5,0.7 5,5,0",
    ] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(sources.len(), 4);
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1 Refit=0.01 UntrimmedMiters=Yes");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    assert!(app.command_hint().contains("Drag seam"));
    enter(&mut app, "Natural");
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    let preview =
        wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(20), |wires| {
            !wires.is_empty()
        });
    enter(&mut app, "RefitTolerance=0.001");
    wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(20), |wires| {
        !wires.is_empty() && wires != preview
    });
    enter(&mut app, "Undo");
    wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(20), |wires| {
        wires == preview
    });
    enter(&mut app, "UntrimmedMiters=No");
    wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(20), |wires| {
        wires.len() > preview.len()
    });
    enter(&mut app, "Undo");
    wait_for_wires_with_timeout(&mut app, &context, Duration::from_secs(20), |wires| {
        wires == preview
    });
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected one joined Sweep1 BRep: {:?}", app.command_log);
    };
    assert_eq!(app.document.objects().count(), 5);
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (5, 14));
    let mut v_domains = sweep
        .faces()
        .iter()
        .map(|face| {
            let surface = face.surface();
            (*surface.domain_u().start(), surface.domain_v())
        })
        .collect::<Vec<_>>();
    v_domains.sort_by(|(a, av), (b, bv)| a.total_cmp(b).then(av.start().total_cmp(bv.start())));
    assert_eq!(
        v_domains,
        [
            (0., 0. ..=1.),
            (0., 1. ..=1.5),
            (0., 1.5..=2.5),
            (0., 2.5..=3.),
            (5., 0. ..=3.),
        ]
    );
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), preview);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_trimmed_preview_preserves_creases_through_the_shared_miter() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Polyline 0,0,0 0,-1,1 0,-2,2.5",
        "Polyline 5,5,0 4,5,1 3,5,2.5",
    ] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(sources.len(), 3);
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1 UntrimmedMiters=No");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let trimmed = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "UntrimmedMiters=Yes");
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != trimmed
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == trimmed);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a committed Sweep1 BRep: {:?}", app.command_log);
    };
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (4, 12));
    let mut domains = sweep
        .faces()
        .iter()
        .map(|face| face.surface().domain_v())
        .collect::<Vec<_>>();
    domains.sort_by(|a, b| a.start().total_cmp(b.start()));
    assert_eq!(domains[0], domains[1]);
    assert_eq!(domains[2], domains[3]);
    assert_eq!(domains[0].end(), domains[2].start());
    assert!(
        sweep
            .vertices()
            .iter()
            .any(|vertex| { vertex.point().distance_to(point(5., 0., 1.)).unwrap() < 1e-8 })
    );
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), trimmed);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}
