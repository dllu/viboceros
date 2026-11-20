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

#[test]
fn sweep_miter_trimmed_preview_shows_extension_boundaries_on_a_picked_chain() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 5,0,0",
        "Line 5,0,0 5,5,0",
        "Line 0,0,0 0,-1.5,1",
    ] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(
        &mut app,
        "Sweep1 ChainEdges AutoChain=No ChainContinuity=Position",
    );
    for source in &sources[..2] {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    assert!(app.chain_prompt.is_none());
    enter(&mut app, &sources[2].id().to_string());
    enter(&mut app, "");
    assert!(app.command_hint().contains("UntrimmedMiters"));
    let trimmed = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    let extension_ends = [point(5., 0., 0.), point(5., -1.5, 1.)];
    assert!(trimmed.iter().any(|curve| {
        let bounds = curve.domain();
        let ends = [*bounds.start(), *bounds.end()].map(|t| curve.evaluate(t).unwrap());
        extension_ends
            .iter()
            .all(|p| ends.iter().any(|end| p.distance_to(*end).unwrap() < 1e-8))
    }));
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a committed Sweep1 surface: {:?}", app.command_log);
    };
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (4, 11));
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), trimmed);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_miter_review_uses_the_authored_corner_and_interior_profiles() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Line 0,0,0 0,1.5,1",
        "Line 2,0,0 2,1.25,1.5",
        "Line 5,0,0 4,1,1",
        "Line 5,5,0 4,5,1",
    ] {
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
    let preview = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "UntrimmedMiters=Yes");
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a joined Sweep1 BRep: {:?}", app.command_log);
    };
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (2, 7));
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), preview);
    let first = sweep.faces()[0].surface();
    let last = sweep.faces()[1].surface();
    assert_eq!((first.degree_u(), last.degree_u()), (3, 3));
    // Match the full Rhino 8.32 control nets in the common normalized V chart.
    for (surface, interior_knots, rail, tips) in [
        (
            first,
            &[1., 2., 3.][..],
            &[
                point(0., 0., 0.),
                point(0.333331616, 0., 0.),
                point(0.999999135, 0., 0.),
                point(2.000000606, 0., 0.),
                point(3.333331052, 0., 0.),
                point(4.333328793, 0., 0.),
                point(5., 0., 0.),
            ][..],
            &[
                point(0., 1.5, 1.),
                point(0.337057176, 1.503027018, 0.997671525),
                point(0.981371334, 1.359864911, 1.261642376),
                point(2.059609571, 1.235932284, 1.587744397),
                point(3.020383984, 1.183230508, 1.320591917),
                point(3.327963986, 0.995641094, 1.003353004),
                point(4., 1., 1.),
            ][..],
        ),
        (
            last,
            &[][..],
            &[
                point(5., 0., 0.),
                point(5., 1.666658263, 0.),
                point(5., 3.333346735, 0.),
                point(5., 5., 0.),
            ][..],
            &[
                point(4., 1., 1.),
                point(4., 2.666658263, 1.),
                point(4., 3.333346735, 1.),
                point(4., 5., 1.),
            ][..],
        ),
    ] {
        assert_eq!(surface.control_point_count_u(), rail.len());
        assert_eq!(surface.control_point_count_v(), 2);
        assert_eq!(surface.knots_v(), [0., 0., 1., 1.]);
        let actual_knots = &surface.knots_u()[4..surface.control_point_count_u()];
        assert_eq!(actual_knots.len(), interior_knots.len());
        for (&actual, &expected) in actual_knots.iter().zip(interior_knots) {
            assert!((actual - expected).abs() < 1e-9);
        }
        for (control, expected) in surface.control_points().iter().zip(rail.iter().chain(tips)) {
            assert!((control.weight() - 1.).abs() < 1e-12);
            assert!(control.point().distance_to(*expected).unwrap() < 2e-5);
        }
    }
    let first_v_end = *first.domain_v().end();
    let last_v_end = *last.domain_v().end();
    for (surface, u, v, expected) in [
        (first, 1., first_v_end, point(1., 1.375, 1.25)),
        (first, 2., first_v_end, point(2., 1.25, 1.5)),
        (first, 5., first_v_end, point(4., 1., 1.)),
        (last, 5., last_v_end, point(4., 1., 1.)),
        (last, 7.5, last_v_end, point(4., 3., 1.)),
    ] {
        assert!(
            surface
                .evaluate(u, v)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-8
        );
    }
    assert!((sweep.area(tolerance).unwrap() - 14.22547357).abs() < 5e-5);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_miter_preview_extends_to_the_hovered_point_end() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Line 5,0,0 4,1,1",
        "Line 5,5,0 4,5,1",
    ] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let pole = point(0., 0., 0.);
    app.document.add_geometry(Geometry::Point(pole)).unwrap();
    app.document.clear_history().unwrap();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let leg = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "StartPoint");
    enter(&mut app, "Point");
    let (pixel, whole) = hover_pole(&mut app, &context, pole, pole);
    assert_ne!(whole, leg);
    assert_eq!(app.document.undo_label(), None);
    viewport_click(&mut app, &context, pixel);
    wait_for_wires(&mut app, &context, |wires| wires == whole);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a joined Sweep1 BRep: {:?}", app.command_log);
    };
    assert_eq!(sweep.faces().len(), 2);
    assert_eq!(
        sweep.wireframe_curves(1, app.document.tolerance()).unwrap(),
        whole
    );
}

#[test]
fn sweep_miter_review_wraps_the_rail_and_updates_both_seam_modes() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0 0,5,0 0,0,0",
        "Line 0,0,0 1,1,1",
        "Line 5,0,0 4,1,1",
        "Line 5,5,0 4,4,1",
        "Line 0,5,0 1,4,1",
    ] {
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
    assert!(app.command_hint().contains("Closed"));
    let open = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "_Closed");
    enter(&mut app, "_Yes");
    let trimmed = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != open
    });
    let has_end = |wires: &[NurbsCurve], target: Point3| {
        wires.iter().any(|wire| {
            let domain = wire.domain();
            [*domain.start(), *domain.end()]
                .into_iter()
                .any(|t| wire.evaluate(t).unwrap().distance_to(target).unwrap() < 1e-8)
        })
    };
    let extension = point(-6_f64.sqrt(), 0., 0.);
    assert!(has_end(&trimmed, extension));
    enter(&mut app, "UntrimmedMiters=Yes");
    let natural = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != open && wires != trimmed
    });
    assert!(has_end(&natural, point(1., -1., 1.)));
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == trimmed);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!(
            "expected a joined sweep around the rail: {:?}",
            app.command_log
        );
    };
    assert_eq!(
        (
            sweep.faces().len(),
            sweep.edges().len(),
            sweep.vertices().len()
        ),
        (6, 19, 14)
    );
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), trimmed);
    assert!((sweep.area(tolerance).unwrap() - 30.96983379).abs() < 5e-5);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_miter_review_updates_an_inferred_corner_when_blending_changes() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 3,0,0 3,7,0",
        "Line 0,0,0 0,1.5,1",
        "Line 3,7,0 0,7,2",
    ] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1 UntrimmedMiters=Yes");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let has_section_end = |wires: &[NurbsCurve], target: Point3| {
        wires.iter().any(|wire| {
            let domain = wire.domain();
            [*domain.start(), *domain.end()]
                .into_iter()
                .any(|t| wire.evaluate(t).unwrap().distance_to(target).unwrap() < 1e-8)
        })
    };
    let local = wait_for_wires(&mut app, &context, |wires| {
        has_section_end(wires, point(1.176, 1.824, 1.216))
    });
    enter(&mut app, "_ShapeBlending");
    enter(&mut app, "_Global");
    let global = wait_for_wires(&mut app, &context, |wires| {
        has_section_end(wires, point(1.05, 1.95, 1.3))
    });
    assert_ne!(local, global);
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == local);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "ShapeBlending=Global");
    wait_for_wires(&mut app, &context, |wires| wires == global);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected an inferred miter: {:?}", app.command_log);
    };
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (2, 7));
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), global);
    assert!((sweep.area(tolerance).unwrap() - 21.63183573).abs() < 2e-4);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_miter_review_keeps_both_failed_trim_pieces_in_one_undo_step() {
    let mut app = test_app();
    for input in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Line 0,0,0 0,1.5,1",
        "Line 5,5,0 3.5,5,2",
    ] {
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
    let trimmed = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "UntrimmedMiters=Yes");
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != trimmed
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == trimmed);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let outputs = app
        .document
        .objects()
        .skip(sources.len())
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut wires = Vec::new();
    for output in outputs {
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("expected two extended pieces: {:?}", app.command_log);
        };
        assert_eq!((brep.faces().len(), brep.edges().len()), (2, 7));
        wires.extend(brep.wireframe_curves(1, tolerance).unwrap());
    }
    assert_eq!(wires.len(), trimmed.len());
    assert!(wires.iter().all(|curve| trimmed.contains(curve)));
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("could not be joined")
    );
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_miter_review_extends_a_loop_with_closed_cross_sections() {
    use viboceros_geometry::{Circle3, UnitVector3};

    let mut app = test_app();
    let tolerance = app.document.tolerance();
    enter(&mut app, "Polyline 0,0,0 5,0,0 5,5,0 0,5,0 0,0,0");
    for (center, nx, ny, radius) in [
        (point(0., 0., 0.), 1., -1., 0.8),
        (point(5., 0., 0.), 1., 1., 1.),
        (point(5., 5., 0.), -1., 1., 1.2),
        (point(0., 5., 0.), -1., -1., 1.4),
    ] {
        let axis = UnitVector3::try_new(-ny, nx, 0., tolerance).unwrap();
        let normal = UnitVector3::try_new(nx, ny, 0., tolerance).unwrap();
        let circle = Circle3::try_from_frame(center, radius, axis, normal, tolerance).unwrap();
        app.document.add_geometry(Geometry::Circle(circle)).unwrap();
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1 Closed=Yes");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    assert!(app.command_hint().contains("Drag seam"));
    enter(&mut app, "Natural");
    enter(&mut app, "");
    assert!(app.command_hint().contains("UntrimmedMiters"));

    let seam = point(-2.000022497410038, 0.565685424949238, 0.);
    let trimmed = wait_for_wires(&mut app, &context, |wires| {
        wires.iter().any(|curve| {
            let domain = curve.domain();
            [*domain.start(), *domain.end()]
                .into_iter()
                .any(|t| curve.evaluate(t).unwrap().distance_to(seam).unwrap() < 1e-8)
        })
    });
    enter(&mut app, "UntrimmedMiters=Yes");
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != trimmed
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == trimmed);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(result) = app.document.objects().last().unwrap().geometry() else {
        panic!(
            "expected a Sweep1 BRep around the closed rail: {:?}",
            app.command_log
        );
    };
    assert_eq!((result.faces().len(), result.edges().len()), (6, 13));
    assert_eq!(result.wireframe_curves(1, tolerance).unwrap(), trimmed);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}
