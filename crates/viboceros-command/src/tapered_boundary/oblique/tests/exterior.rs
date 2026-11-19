use super::*;

#[test]
fn oblique_boundary_keeps_the_captured_preview_when_a_warped_cutter_crosses_the_rim() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let mut doc = Document::new(tolerance);
    let commands = CommandRegistry::with_builtins();
    let source = doc
        .add_geometry(Geometry::NurbsCurve(circle(tolerance)))
        .unwrap();
    let support = NurbsSurface::try_bilinear([
        point(-3., -2., 4.3),
        point(5., -2., 5.5),
        point(5., 2., 6.8),
        point(-3., 2., 5.2),
    ])
    .unwrap();
    let cutter = doc.add_geometry(Geometry::NurbsSurface(support)).unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();

    let options =
        TaperedExtrusionOptions::parse(&["Direction=1,0,6", "DraftAngle=-10", "Solid=Yes"])
            .unwrap();
    assert_eq!(
        commands
            .execute_curve_tapered_boundary_pick(
                &mut doc,
                BoundaryRef {
                    object: cutter,
                    face: None
                },
                options,
                CommandContext::default().construction_plane,
                TaperedBoundaryPreview {
                    distance: Some(4.2705479452054815 * 37_f64.sqrt()),
                    pick_offset: Some(
                        Vector3::try_new(-3.0650684931506866, -0.9280821917808217, 4.525).unwrap()
                    ),
                },
            )
            .unwrap(),
        TaperedBoundaryOutcome::Fallback
    );
    assert!(doc.is_selected(source));
    assert!(doc.is_selected(cutter));
    let Geometry::Brep(preview) = doc
        .objects()
        .find(|obj| ![source, cutter].contains(&obj.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected the open preview stock");
    };
    assert_eq!(preview.faces().len(), 5);
    assert!(!preview.is_solid());
    let bounds = Shape::from_brep(preview, tolerance)
        .unwrap()
        .tight_bounds()
        .unwrap();
    assert!(
        bounds
            .min()
            .is_near(point(-3.247529005687447, -7.580398197649268, 0.), tolerance)
    );
    assert!(bounds.max().is_near(
        point(11.788624896098431, 7.580398197649268, 26.376300496381777),
        tolerance
    ));
    commands.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 2);
    commands.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 3);
}

#[test]
fn oblique_boundary_partial_cut_consumes_a_picked_wall_only_curve() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for heights in [
        [5.; 4],
        [
            4.8931818181818185,
            5.415909090909091,
            6.134848484848485,
            5.513636363636364,
        ],
    ] {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let source = doc
            .add_geometry(Geometry::NurbsCurve(circle(tolerance)))
            .unwrap();
        let support = NurbsSurface::try_bilinear([
            point(0., -6., heights[0]),
            point(10., -6., heights[1]),
            point(10., 7., heights[2]),
            point(0., 7., heights[3]),
        ])
        .unwrap();
        let pick = Vector3::try_from(support.evaluate(0.5, 0.002).unwrap().to_array()).unwrap();
        let cutter = doc.add_geometry(Geometry::NurbsSurface(support)).unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let options = TaperedExtrusionOptions::parse(&[
            "Direction=1,0,6",
            "DraftAngle=-10",
            "Solid=No",
            "DeleteInput=Yes",
        ])
        .unwrap();
        assert_eq!(
            commands
                .execute_curve_tapered_boundary_pick(
                    &mut doc,
                    BoundaryRef {
                        object: cutter,
                        face: None
                    },
                    options,
                    CommandContext::default().construction_plane,
                    TaperedBoundaryPreview {
                        distance: Some(1.832191780821918 * 37_f64.sqrt()),
                        pick_offset: Some(pick),
                    },
                )
                .unwrap(),
            TaperedBoundaryOutcome::Fallback,
        );
        assert!(doc.object(source).is_none());
        assert_eq!(doc.objects().len(), 1);
        assert!(doc.is_selected(cutter));
        commands.execute(&mut doc, "Undo").unwrap();
        assert!(doc.is_selected(source));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(doc.selected_object_count(), 0);
    }
}
