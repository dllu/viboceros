use super::*;
fn source() -> SubD {
    SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [8., 0., 0.],
            [0., 3., 0.],
            [4., 3., 1.],
            [8., 3., 2.],
            [0., 6., 0.],
            [4., 6., 2.],
            [8., 6., 1.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![
            vec![0, 1, 4, 3],
            vec![1, 2, 5, 4],
            vec![3, 4, 7, 6],
            vec![4, 5, 8, 7],
        ],
    )
    .unwrap()
}
#[test]
fn subd_extract_retains_unrelated_soft_sharpness_and_source_archive() {
    let s = source();
    let edge = s
        .edges()
        .iter()
        .position(|e| *e == [4, 5] || *e == [5, 4])
        .unwrap();
    let s = s
        .try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
        .unwrap()
        .0;
    let bytes = s.archive().to_vec();
    let outputs = s.try_extract_faces(&[0, 0], false).unwrap();
    assert_eq!(s.archive(), bytes);
    let remaining = &outputs.remainder[0];
    let ends = s.edges()[edge].map(|v| s.control_points()[v as usize]);
    let index = remaining
        .edges()
        .iter()
        .position(|e| e.map(|v| remaining.control_points()[v as usize]) == ends)
        .unwrap();
    assert_eq!(remaining.edge_sharpness()[index], [1.25, 2.5]);
    assert!(s.try_extract_faces(&[4], false).is_err());
    assert!(s.try_extract_faces(&[], false).is_err());
    assert!(s.try_retain_faces(&[0], &[99]).is_err());
    assert_eq!(s.archive(), bytes);
}
#[test]
fn subd_extract_fragmented_inputs_reject_before_materializing_archives() {
    let mut points = vec![];
    let mut faces = vec![];
    for i in 0..1500 {
        let base = points.len() as u32;
        for (x, y) in [(0., 0.), (1., 0.), (1., 1.), (0., 1.)] {
            points.push(Point3::try_new(3. * i as f64 + x, y, 0.).unwrap());
        }
        faces.push(vec![base, base + 1, base + 2, base + 3]);
    }
    let s = SubD::try_from_polygons(points, faces).unwrap();
    let bytes = s.archive().to_vec();
    assert!(s.try_extract_faces(&[0], false).is_err());
    assert_eq!(s.archive(), bytes);
}
