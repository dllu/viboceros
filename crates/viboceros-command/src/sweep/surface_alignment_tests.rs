use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn sweep_one_surface_aligned_frame_matches_rhino_on_a_twisting_face() {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    let tolerance = document.tolerance();
    let support = NurbsSurface::try_bilinear([
        p(0., 0., 0.),
        p(-2., 0., 0.),
        p(-2., 6., -2.),
        p(0., 6., 0.),
    ])
    .unwrap();
    let brep = Brep::try_surface_face(support, tolerance).unwrap();
    let edge = brep
        .edges()
        .iter()
        .position(|edge| {
            edge.curve()
                .control_points()
                .iter()
                .all(|c| c.point().x() == 0.)
        })
        .unwrap();
    let rail = document.add_geometry(Geometry::Brep(brep)).unwrap();
    let profile = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap(),
        ))
        .unwrap();
    let end_profile = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 6., 0.), p(1., 6., 1.), tolerance).unwrap(),
        ))
        .unwrap();

    // Rhino 8.32 uses the same control net for a single profile with either refit setting.
    let one_profile = [
        p(2., 0., 0.),
        p(2.0006867594139157, 2., 0.7046705282163714),
        p(1.6344072365646087, 4., 1.2010065773232816),
        p(std::f64::consts::SQRT_2, 6., std::f64::consts::SQRT_2),
    ];
    let two_profiles = [
        p(2., 0., 0.),
        p(1.9719441398737456, 2., 0.7835621144521392),
        p(1.1125812915057853, 4., 0.8961434059579246),
        p(1., 6., 1.),
    ];
    for (refit, selected, expected, area) in [
        ("Yes", &[profile][..], one_profile, 12.142261645193928),
        ("No", &[profile][..], one_profile, 12.142261645193928),
        (
            "No",
            &[profile, end_profile][..],
            two_profiles,
            10.344600422833881,
        ),
    ] {
        document
            .select_objects_direct(selected.iter().copied(), SelectionMode::Replace)
            .unwrap();
        document.clear_history().unwrap();
        let originals = document.objects().cloned().collect::<Vec<_>>();
        commands
            .execute(
                &mut document,
                &format!(
                    "Sweep1 RailEdge={rail},{edge},0 Style=AlignWithSurface RefitRail={refit}",
                ),
            )
            .unwrap();
        let Geometry::Brep(sweep) = document.objects().last().unwrap().geometry() else {
            panic!("expected Sweep1 B-rep");
        };
        assert_eq!((sweep.faces().len(), sweep.edges().len()), (1, 4));
        let surface = sweep.faces()[0].surface();
        assert_eq!([surface.degree_u(), surface.degree_v()], [3, 1]);
        for (u, far) in expected.into_iter().enumerate() {
            let near = surface.control_point(u, 0).unwrap().point();
            let actual = surface.control_point(u, 1).unwrap().point();
            assert!(
                near.distance_to(p(0., 2. * u as Real, 0.)).unwrap() < 1e-8,
                "{near:?}"
            );
            assert!(
                actual.distance_to(far).unwrap() < 1e-8,
                "{actual:?} != {far:?}"
            );
        }
        assert!((sweep.area(tolerance).unwrap() - area).abs() < 1e-5);
        assert_eq!(document.undo_label(), Some("Sweep1"));
        document.undo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), originals);
    }
}

#[test]
fn sweep_one_surface_alignment_requires_a_supporting_face() {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    for input in ["Line 0,0,0 0,6,0", "Line 0,0,0 2,0,0"] {
        commands.execute(&mut document, input).unwrap();
    }
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let result = commands.execute(&mut document, "Sweep1 FrameStyle=Surface");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("one supporting face")
    );
    assert_eq!(document.objects().count(), 2);
    assert_eq!(document.undo_label(), None);
}
