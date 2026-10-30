use super::*;

fn wall(ring: &[[f64; 2]], bottom: f64, top: f64) -> SubD {
    let n = ring.len();
    let points = [bottom, top]
        .into_iter()
        .flat_map(|z| {
            ring.iter()
                .map(move |&[x, y]| Point3::try_new(x, y, z).unwrap())
        })
        .collect();
    let faces = (0..n)
        .map(|i| {
            vec![
                i as u32,
                ((i + 1) % n) as u32,
                ((i + 1) % n + n) as u32,
                (i + n) as u32,
            ]
        })
        .collect();
    SubD::try_from_polygons(points, faces).unwrap()
}

#[test]
fn caps_preserve_original_component_ids_face_properties_and_corner_uvw() {
    let original = wall(&[[0., 0.], [4., 0.], [4., 3.], [0., 3.]], 0., 2.)
        .try_with_face_properties(&[(
            1,
            SubDFaceProperties {
                color: Some([17, 29, 43, 71]),
                material_channel: 65535,
            },
        )])
        .unwrap()
        .try_with_face_texture_points(&[(
            1,
            Some(vec![
                [0., 0., 0.25],
                [3., 0., 0.5],
                [3., 2., 0.75],
                [0., 2., 1.],
            ]),
        )])
        .unwrap();
    let snapshot = original.archive().to_vec();
    for crease in [false, true] {
        for triangles in [false, true] {
            let (capped, count) = original
                .cap_planar_holes_with_options(Tolerance::default(), crease, triangles)
                .unwrap();
            assert_eq!(count, 2);
            assert_eq!(&capped.control_points()[..8], original.control_points());
            assert_eq!(&capped.vertex_ids()[..8], original.vertex_ids());
            assert_eq!(&capped.edge_ids()[..12], original.edge_ids());
            assert_eq!(&capped.face_ids()[..4], original.face_ids());
            assert_eq!(&capped.face_properties()[..4], original.face_properties());
            assert_eq!(
                &capped.face_texture_points()[..4],
                original.face_texture_points()
            );
            assert_eq!(
                capped.texture_coordinate_type(),
                original.texture_coordinate_type()
            );
            assert!(capped.edge_face_counts().iter().all(|&n| n == 2));
            assert_eq!(capped.faces().len(), if triangles { 12 } else { 6 });
            assert_eq!(original.archive(), snapshot);
        }
    }
}

#[test]
fn nonplanar_openings_are_skipped_and_closed_results_and_sheets_are_noops() {
    let source = wall(&[[0., 0.], [4., 0.], [4., 3.], [0., 3.]], 0., 2.);
    let (closed, count) = source
        .cap_planar_holes_with_options(Tolerance::default(), true, false)
        .unwrap();
    assert_eq!(count, 2);
    let (same, count) = closed
        .cap_planar_holes_with_options(Tolerance::default(), false, true)
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(same.archive(), closed.archive());
    let sheet = SubD::try_from_polygons(
        source.control_points()[..4].to_vec(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let (same, count) = sheet
        .cap_planar_holes_with_options(Tolerance::default(), true, false)
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(same.archive(), sheet.archive());
    let mut points = source.control_points().to_vec();
    points[6] = Point3::try_new(4., 3., 2.25).unwrap();
    let warped = SubD::try_from_polygons(points, source.faces().to_vec()).unwrap();
    let (capped, count) = warped
        .cap_planar_holes_with_options(Tolerance::default(), true, false)
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(capped.faces().len(), 5);
    assert_eq!(
        capped
            .edge_face_counts()
            .iter()
            .filter(|&&n| n == 1)
            .count(),
        4
    );
}

#[test]
fn spatially_separated_components_cap_independently_without_welding_controls() {
    let n = 128usize;
    let mut points = Vec::new();
    let mut faces = Vec::new();
    for i in 0..n {
        let x = i as f64 * 16.;
        let part = wall(&[[x, 0.], [x + 4., 0.], [x + 4., 3.], [x, 3.]], 0., 2.);
        let offset = points.len() as u32;
        points.extend_from_slice(part.control_points());
        faces.extend(
            part.faces()
                .iter()
                .map(|f| f.iter().map(|&v| v + offset).collect::<Vec<_>>()),
        );
    }
    let source = SubD::try_from_polygons(points, faces).unwrap();
    let (capped, count) = source
        .cap_planar_holes_with_options(Tolerance::default(), true, false)
        .unwrap();
    assert_eq!(count, 2 * n);
    assert_eq!(capped.control_points(), source.control_points());
    assert_eq!(capped.faces().len(), 6 * n);
    assert!(capped.edge_face_counts().iter().all(|&n| n == 2));
}

#[test]
fn intersecting_and_coincident_component_boundaries_are_left_open() {
    for shift in [0., 2.] {
        let first = wall(&[[0., 0.], [4., 0.], [4., 3.], [0., 3.]], 0., 2.);
        let second = wall(
            &[[shift, 0.], [shift + 4., 0.], [shift + 4., 3.], [shift, 3.]],
            0.,
            2.,
        );
        let mut points = first.control_points().to_vec();
        points.extend_from_slice(second.control_points());
        let mut faces = first.faces().to_vec();
        faces.extend(
            second
                .faces()
                .iter()
                .map(|f| f.iter().map(|&v| v + 8).collect::<Vec<_>>()),
        );
        let source = SubD::try_from_polygons(points, faces).unwrap();
        let (same, count) = source
            .cap_planar_holes_with_options(Tolerance::default(), true, false)
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(same.archive(), source.archive());
    }
}

#[test]
fn region_planning_budget_exhaustion_leaves_the_native_source_unchanged() {
    let source = wall(&[[0., 0.], [4., 0.], [4., 3.], [0., 3.]], 0., 2.);
    let archive = source.archive().to_vec();
    assert!(matches!(
        regions::prepare_with_budget(&source, Tolerance::default(), false, 8),
        Err(GeometryError::SubDCapResourceLimit { .. })
    ));
    assert_eq!(source.archive(), archive);
    assert_eq!(source.faces().len(), 4);
}
