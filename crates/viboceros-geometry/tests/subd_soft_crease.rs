use viboceros_geometry::{AffineTransform3, Point3, SubD, Vector3};
fn cube() -> SubD {
    let points = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 3., 0.],
        [0., 3., 0.],
        [0., 0., 2.],
        [4., 0., 2.],
        [4., 3., 2.],
        [0., 3., 2.],
    ];
    SubD::try_from_polygons(
        points
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![
            vec![0, 3, 2, 1],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
            vec![4, 5, 6, 7],
        ],
    )
    .unwrap()
}
#[test]
fn endpoint_weights_roundtrip_preserve_orientation_and_are_distinct_from_hard_tags() {
    let source = cube();
    let bytes = source.archive().to_vec();
    let (soft, changed) = source
        .try_with_soft_crease_weights(&[(0, [10., 90.]), (3, [100., 100.])])
        .unwrap();
    assert!(changed);
    assert_eq!(source.archive(), bytes);
    assert_eq!(soft.edge_sharpness()[0], [0.4_f32 as f64, 3.6_f32 as f64]);
    assert_eq!(soft.edge_sharpness()[3], [4., 4.]);
    assert_eq!(soft.edge_tags()[3], 1);
    let restored = SubD::try_from_archive(soft.archive()).unwrap();
    assert_eq!(restored.edge_sharpness(), soft.edge_sharpness());
    assert_eq!(restored.surface_points(), soft.surface_points());
    assert_eq!(restored.tessellate(2).unwrap(), soft.tessellate(2).unwrap());
    let (hard, _) = soft.try_crease_components(&[3], &[], true).unwrap();
    assert_eq!(hard.edge_tags()[3], 2);
    assert_eq!(hard.edge_sharpness()[3], [0., 0.]);
    let (relaxed, changed) = hard.try_with_soft_crease_weights(&[(3, [0., 0.])]).unwrap();
    assert!(changed);
    assert_eq!(relaxed.edge_tags()[3], 1);
    assert_eq!(relaxed.edge_sharpness()[0], soft.edge_sharpness()[0]);
}
#[test]
fn invalid_or_ambiguous_edits_leave_the_source_unchanged() {
    let source = cube();
    let before = source.clone();
    for edits in [
        vec![(usize::MAX, [50.; 2])],
        vec![(0, [-1., 50.])],
        vec![(0, [50., 101.])],
        vec![(0, [f64::NAN, 0.])],
        vec![(0, [0., f64::INFINITY])],
        vec![(0, [20.; 2]), (0, [80.; 2])],
        vec![(0, [50.; 2]), (usize::MAX, [50.; 2])],
    ] {
        assert!(source.try_with_soft_crease_weights(&edits).is_err());
        assert_eq!(source, before);
    }
    let boundary = SubD::try_from_polygons(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
        ],
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    assert!(
        boundary
            .try_with_soft_crease_weights(&[(0, [50.; 2])])
            .is_err()
    );
    assert_eq!(
        source.try_with_soft_crease_weights(&[]).unwrap(),
        (source.clone(), false)
    );
    assert!(
        !source
            .try_with_soft_crease_weights(&[(0, [0.; 2])])
            .unwrap()
            .1
    );
}
#[test]
fn repeated_weights_and_position_edits_reuse_the_stored_sharpness() {
    let source = cube();
    let (soft, _) = source
        .try_with_soft_crease_weights(&[(0, [20., 80.]), (1, [40., 60.])])
        .unwrap();
    let (same, changed) = soft
        .try_with_soft_crease_weights(&[(0, [20., 80.]), (1, [40., 60.])])
        .unwrap();
    assert!(!changed);
    assert_eq!(same, soft);
    let moved = soft
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(8., -3., 2.).unwrap(),
        ))
        .unwrap();
    assert_eq!(moved.edge_sharpness(), soft.edge_sharpness());
    let mut points = soft.control_points().to_vec();
    points[0] = Point3::try_new(-1., 0., 0.).unwrap();
    let edited = soft.try_with_control_points(&points).unwrap();
    assert_eq!(edited.edge_sharpness(), soft.edge_sharpness());
    let display = viboceros_opennurbs::subd_tessellate(soft.archive(), 2).unwrap();
    assert!(display.edge_sharpness.is_empty());
    assert!(display.edge_face_counts.is_empty());
}

#[test]
fn native_float_rounding_cannot_change_internal_snapshots_at_weight_thresholds() {
    let source = cube();
    for weight in [0.25, 24.75, 25.25, 49.75, 50.25, 74.75, 75.25, 99.75] {
        let (soft, _) = source
            .try_with_soft_crease_weights(&[(0, [weight; 2]), (1, [10., weight])])
            .unwrap();
        let points = soft.surface_points().to_vec();
        let mesh = soft.tessellate(2).unwrap();
        let lines = soft.edge_polylines(2).unwrap();
        let mut restored = soft.clone();
        for _ in 0..8 {
            restored = SubD::try_from_archive(restored.archive()).unwrap();
            assert_eq!(restored.edge_sharpness(), soft.edge_sharpness());
            assert_eq!(restored.surface_points(), points);
            assert_eq!(restored.tessellate(2).unwrap(), mesh);
            assert_eq!(restored.edge_polylines(2).unwrap(), lines);
        }
    }
    let (soft, _) = source
        .try_with_soft_crease_weights(&[(0, [24.75, 25.25])])
        .unwrap();
    assert_eq!(soft.edge_sharpness()[0], [0.99_f32 as f64, 1.01_f32 as f64]);
}
#[test]
fn exact_sharpness_envelopes_reject_truncation_duplicates_and_invalid_values() {
    let (source, _) = cube()
        .try_with_soft_crease_weights(&[(0, [24.75, 25.25])])
        .unwrap();
    let bytes = source.archive();
    assert_eq!(&bytes[..8], b"VIBOSUB2");
    let payload = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let record = 24 + payload;
    for length in [0, 7, 16, 23, 24, bytes.len() - 21, bytes.len() - 1] {
        assert!(SubD::try_from_archive(&bytes[..length]).is_err());
    }
    for (start, replacement) in [(16, u32::MAX), (20, u32::MAX), (record, u32::MAX)] {
        let mut bad = bytes.to_vec();
        bad[start..start + 4].copy_from_slice(&replacement.to_le_bytes());
        assert!(SubD::try_from_archive(&bad).is_err());
    }
    for value in [f64::NAN, f64::INFINITY, -1., 5., 0.995] {
        let mut bad = bytes.to_vec();
        bad[record + 4..record + 12].copy_from_slice(&value.to_le_bytes());
        assert!(SubD::try_from_archive(&bad).is_err());
    }
    let mut repeated = bytes.to_vec();
    repeated[20..24].copy_from_slice(&2_u32.to_le_bytes());
    repeated.extend_from_slice(&bytes[record..record + 20]);
    assert!(SubD::try_from_archive(&repeated).is_err());
    assert_eq!(
        SubD::try_from_archive(bytes).unwrap().edge_sharpness(),
        source.edge_sharpness()
    );
}
