use super::*;

#[test]
fn sweep_align_cross_sections_previews_native_flip_and_restores_sources() {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,5", "Line 0,0,0 2,0,0", "Line 0,0,5 0,3,5"] {
        enter(&mut app, input);
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let profile = sources[2].id();
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Right);
    app.maximized_viewport = Some(0);

    enter(&mut app, "Sweep1");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let original = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "_AlignCrossSections");
    assert!(app.command_hint().contains("cross-section end"));
    let tip = point(0., 3., 5.);
    viewport_frame(&mut app, &context, vec![]);
    let pixel = screen_point(&app, tip);
    viewport_click(&mut app, &context, pixel);
    let reversed = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != original
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == original);
    enter(&mut app, "SelID");
    assert!(app.command_hint().contains("object ID"));
    enter(&mut app, &profile.to_string());
    wait_for_wires(&mut app, &context, |wires| wires == reversed);
    app.cancel_current_prompt_or_selection();
    assert!(app.command_hint().starts_with("Review the sweep"));
    wait_for_wires(&mut app, &context, |wires| wires == reversed);
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == original);

    enter(&mut app, "AlignCrossSections");
    viewport_click(&mut app, &context, pixel);
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
    assert_eq!(app.document.undo_label(), None);
    wait_for_wires(&mut app, &context, |wires| wires == reversed);
    enter(&mut app, "ShapeBlending=Global");
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != reversed
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == reversed);
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a committed Sweep1 surface: {:?}", app.command_log);
    };
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), reversed);
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (1, 4));

    // Rhino 8.32, normal Sweep1 dialog, second profile flipped in the Right view.
    let surface = sweep.faces()[0].surface();
    assert_eq!((surface.degree_u(), surface.degree_v()), (3, 1));
    assert_eq!(
        (
            surface.control_point_count_u(),
            surface.control_point_count_v()
        ),
        (4, 2)
    );
    let controls = [
        [point(0., 0., 0.), point(2., 0., 0.)],
        [point(0., 0., 5. / 3.), point(2., 0., 5. / 3.)],
        [point(0., 3., 10. / 3.), point(0., 0., 10. / 3.)],
        [point(0., 3., 5.), point(0., 0., 5.)],
    ];
    for (u, row) in controls.into_iter().enumerate() {
        for (v, native) in row.into_iter().enumerate() {
            let control = surface.control_point(u, v).unwrap();
            assert!((control.weight() - 1.).abs() < 1e-12);
            assert!(control.point().distance_to(native).unwrap() < 1e-8);
        }
    }
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_alignment_refits_a_curved_rail_and_keeps_the_flipped_profile_station() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let middle = 4. * std::f64::consts::FRAC_1_SQRT_2;
    let arc = CircularArc3::try_from_three_points(
        point(4., 0., 0.),
        point(middle, middle, 0.),
        point(0., 4., 0.),
        tolerance,
    )
    .unwrap();
    let rail = app.document.add_geometry(Geometry::Arc(arc)).unwrap();
    for input in ["Line 4,0,0 6,0,1", "Line 0,4,0 0,7,2"] {
        enter(&mut app, input);
    }
    let profiles = app
        .document
        .objects()
        .filter(|object| object.id() != rail)
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1");
    for id in [rail, profiles[0], profiles[1]] {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "");
    enter(&mut app, "AlignCrossSections");
    enter(&mut app, &format!("SelID {}", profiles[1]));
    enter(&mut app, "");
    let preview = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a committed Sweep1 surface: {:?}", app.command_log);
    };
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), preview);
    // The native alignment step refits even with the Refit Rail box cleared.
    let surface = sweep.faces()[0].surface();
    assert_eq!((surface.degree_u(), surface.degree_v()), (3, 1));
    for (u, v, expected) in [
        (
            surface.domain_u().start(),
            surface.domain_v().start(),
            point(4., 0., 0.),
        ),
        (
            surface.domain_u().start(),
            surface.domain_v().end(),
            point(6., 0., 1.),
        ),
        (
            surface.domain_u().end(),
            surface.domain_v().start(),
            point(0., 7., 2.),
        ),
        (
            surface.domain_u().end(),
            surface.domain_v().end(),
            point(0., 4., 0.),
        ),
    ] {
        assert!(
            surface
                .evaluate(*u, *v)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-8
        );
    }
}
