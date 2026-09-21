use super::*;

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn cap_missing_box_faces_preserves_edges_and_closes_with_consistent_orientation() {
    let cube = Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    for omitted in 0..6 {
        let faces = (0..6).filter(|&i| i != omitted).collect::<Vec<_>>();
        let source = cube.sub_brep(&faces, Tolerance::DEFAULT).unwrap();
        let before = source.clone();
        let capped = source
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(capped.is_solid());
        assert_eq!(capped.faces.len(), 6);
        assert_eq!(capped.vertices, source.vertices);
        assert_eq!(capped.edges, source.edges);
        assert!((capped.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-10);
        assert!((capped.area(Tolerance::DEFAULT).unwrap() - 62.).abs() < 1e-10);
        assert_eq!(source, before);
        assert!(
            capped
                .try_cap_planar_holes(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn cap_tube_groups_nested_rational_boundaries_into_annuli() {
    let tube = Brep::try_tube(frame(), [3., 1.], 5., Tolerance::DEFAULT).unwrap();
    let source = tube.sub_brep(&[0, 1], Tolerance::DEFAULT).unwrap();
    let result = source
        .try_cap_planar_holes(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!(result.is_solid());
    assert_eq!(result.faces.len(), 4);
    assert!(result.faces[2..].iter().all(|f| f.loops.len() == 2));
    assert_eq!(result.vertices, source.vertices);
    assert_eq!(result.edges, source.edges);
    assert!(
        (result.signed_volume(Tolerance::DEFAULT).unwrap() - 40. * std::f64::consts::PI).abs()
            < 1e-8
    );
}

#[test]
fn entirely_planar_breps_are_noops_but_flat_parts_of_nonplanar_breps_can_cap() {
    let cube = Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let sheet = cube.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    assert!(
        sheet
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    let open = cube.sub_brep(&[0, 1, 2, 3, 4], Tolerance::DEFAULT).unwrap();
    let source = Brep::try_combine(vec![sheet.clone(), open], Tolerance::DEFAULT).unwrap();
    let result = source
        .try_cap_planar_holes(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(result.faces.len(), 8);
    assert_eq!(result.faces[0].surface, sheet.faces[0].surface);
    assert!(result.is_solid());
    assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-10);
}

#[test]
fn cap_kernel_preserves_inward_shell_sense_and_input_surfaces() {
    let cube = Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let source = cube
        .sub_brep(&[0, 1, 2, 3, 4], Tolerance::DEFAULT)
        .unwrap()
        .reversed();
    let capped = source
        .try_cap_planar_holes(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!((capped.signed_volume(Tolerance::DEFAULT).unwrap() + 30.).abs() < 1e-10);
    for (a, b) in source.faces.iter().zip(&capped.faces) {
        assert_eq!(a.surface, b.surface);
        assert_eq!(a.reversed, b.reversed);
        for (a, b) in a
            .loops
            .iter()
            .flat_map(|l| &l.trims)
            .zip(b.loops.iter().flat_map(|l| &l.trims))
        {
            assert_eq!(a.curve, b.curve);
            assert_eq!(a.edge, b.edge);
            assert_eq!(a.vertices, b.vertices);
            assert_eq!(a.reversed_3d, b.reversed_3d);
        }
    }
}

#[test]
fn partial_cap_keeps_nonplanar_opening_and_exact_original_edges() {
    let vertices = [
        [0., 0., 0.],
        [2., 0., 0.],
        [2., 3., 0.],
        [0., 3., 0.],
        [0., 0., 5.],
        [2., 0., 5.],
        [2., 3., 6.],
        [0., 3., 5.],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let triangles = vec![
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let mesh = TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap();
    let source = Brep::try_from_mesh(&mesh, true, Tolerance::DEFAULT).unwrap();
    let capped = source
        .try_cap_planar_holes(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(capped.faces.len(), 9);
    assert!(!capped.is_closed());
    assert_eq!(capped.edges, source.edges);
    assert_eq!(capped.vertices, source.vertices);
    assert_eq!(
        capped.edge_use_counts().iter().filter(|&&n| n == 1).count(),
        4
    );
    assert!(
        capped
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
}

#[test]
fn disjoint_coplanar_boundaries_are_independent_caps() {
    let a = Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let b = Brep::try_box(frame(), [[4., 6.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let source = Brep::try_combine(
        vec![
            a.sub_brep(&[0, 1, 2, 3], Tolerance::DEFAULT).unwrap(),
            b.sub_brep(&[0, 1, 2, 3], Tolerance::DEFAULT).unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let capped = source
        .try_cap_planar_holes(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!(capped.is_solid());
    assert_eq!(capped.faces.len(), 12);
    assert!((capped.signed_volume(Tolerance::DEFAULT).unwrap() - 60.).abs() < 1e-10);
}

#[test]
fn inconsistent_nested_boundary_orientations_fail_without_mutation() {
    let tube = Brep::try_tube(frame(), [3., 1.], 5., Tolerance::DEFAULT).unwrap();
    let mut source = tube.sub_brep(&[0, 1], Tolerance::DEFAULT).unwrap();
    source.faces[1].reversed = !source.faces[1].reversed;
    let before = source.clone();
    assert!(matches!(
        source.try_cap_planar_holes(Tolerance::DEFAULT),
        Err(GeometryError::InvalidBrepTopology {
            context: "cap boundaries have inconsistent shell orientation"
        })
    ));
    assert_eq!(source, before);
}

#[test]
fn frame_and_caps_are_scale_and_translation_aware() {
    for (scale, origin) in [
        (1e-4, [0., 0., 0.]),
        (1., [1e7, -1e7, 1e7]),
        (1e4, [0., 0., 0.]),
    ] {
        // At 1e7 a binary64 coordinate ULP exceeds 1e-9. A model tolerance
        // must allow the rounded plane reconstruction at that translation.
        let absolute = if origin == [0.; 3] {
            1e-9 * scale
        } else {
            1e-7
        };
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap();
        let frame = Frame3::try_from_directions(
            Point3::try_from(origin).unwrap(),
            Vector3::try_new(1., 2., 3.).unwrap(),
            Vector3::try_new(-2., 1., 0.).unwrap(),
            tolerance,
        )
        .unwrap();
        let cube = Brep::try_box(
            frame,
            [[0., 2. * scale], [0., 3. * scale], [0., 5. * scale]],
            tolerance,
        )
        .unwrap();
        let source = cube.sub_brep(&[0, 1, 2, 3, 4], tolerance).unwrap();
        let capped = source
            .try_cap_planar_holes(tolerance)
            .unwrap_or_else(|e| panic!("scale={scale} origin={origin:?}: {e}"))
            .expect("planar opening");
        assert!(capped.is_solid());
        assert_eq!(capped.edges, source.edges);
        assert!(
            (capped.signed_volume(tolerance).unwrap() / (30. * scale.powi(3)) - 1.).abs() < 1e-7
        );
    }
}

#[test]
fn work_budget_is_checked_before_sampling_or_projecting_large_inputs() {
    let tube = Brep::try_tube(frame(), [3., 1.], 5., Tolerance::DEFAULT).unwrap();
    let source = tube.sub_brep(&[0, 1], Tolerance::DEFAULT).unwrap();
    let cycles = boundary::cycles(&source);
    let edges = &cycles[0];
    let points = edges
        .iter()
        .flat_map(|&(i, _)| source.edges[i].curve.control_points())
        .map(|p| p.point())
        .collect::<Vec<_>>();
    let frame = boundary::frame(&points, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!(
        source
            .cap_loop(
                edges,
                frame,
                BrepLoopType::Outer,
                Tolerance::DEFAULT,
                &mut WorkBudget(0)
            )
            .is_err()
    );
    let projected = source
        .cap_loop(
            edges,
            frame,
            BrepLoopType::Outer,
            Tolerance::DEFAULT,
            &mut WorkBudget(MAX_CAP_CONTROLS),
        )
        .unwrap()
        .unwrap();
    assert!(valid_region(&[&projected.boundary], &mut WorkBudget(0)).is_err());
}

#[test]
fn nested_similar_loops_use_physical_area_not_per_loop_normalized_area() {
    for radii in [[4., 2.], [1., 0.5], [10., 3.], [3., 2.999]] {
        let tube = Brep::try_tube(frame(), radii, 5., Tolerance::DEFAULT).unwrap();
        let source = tube.sub_brep(&[1, 0], Tolerance::DEFAULT).unwrap();
        let capped = source
            .try_cap_planar_holes(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(capped.is_solid());
        assert_eq!(capped.faces.len(), 4);
        assert!(capped.faces[2..].iter().all(|f| f.loops.len() == 2));
        let expected = 5. * std::f64::consts::PI * (radii[0] * radii[0] - radii[1] * radii[1]);
        assert!((capped.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-8);
    }
}

#[test]
fn sub_coordinate_precision_caps_are_rejected_without_relaxing_tolerance() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    let frame = Frame3::try_from_directions(
        Point3::try_new(1e7, -1e7, 1e7).unwrap(),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-2., 1., 0.).unwrap(),
        tolerance,
    )
    .unwrap();
    let cube = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], tolerance).unwrap();
    let source = cube.sub_brep(&[0, 1, 2, 3, 4], tolerance).unwrap();
    let before = source.clone();
    assert!(matches!(
        source.try_cap_planar_holes(tolerance),
        Err(GeometryError::InvalidBrepTopology {
            context: "a p-curve endpoint misses its model-space vertex"
        })
    ));
    assert_eq!(source, before);
}
