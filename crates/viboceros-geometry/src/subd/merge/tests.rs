use super::*;
fn pair() -> SubD {
    SubD::try_from_polygons(
        [[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![vec![0, 1, 2], vec![0, 2, 3]],
    )
    .unwrap()
}
#[test]
fn rejects_late_invalid_selections_without_editing_the_source() {
    use SubDMergeComponent::*;
    let source = pair();
    let original = source.clone();
    for last in [
        Face(2),
        Vertex(4),
        Edge([1, 3]),
        Edge([0, 0]),
        Face(usize::MAX),
    ] {
        assert!(
            source
                .merge_face_components(&[Face(0), Face(1), last])
                .is_err()
        );
        assert_eq!(source, original);
    }
    let repeated = source
        .merge_face_components(&[Face(0), Face(1), Face(0)])
        .unwrap();
    assert_eq!(repeated.removed_faces, 1);
    assert_eq!(repeated.merged_faces, vec![0]);
    assert_eq!(repeated.subd.face_ids(), &[1]);
}
#[test]
fn empty_and_single_face_requests_preserve_the_complete_record() {
    let source = pair();
    for selection in [vec![], vec![SubDMergeComponent::Face(0)]] {
        let result = source.merge_face_components(&selection).unwrap();
        assert_eq!(result.subd, source);
        assert_eq!(result.removed_faces, 0);
        assert!(result.merged_faces.is_empty());
    }
}
#[test]
fn rejected_hole_region_does_not_block_a_disconnected_valid_pair() {
    let mut points = (0..4)
        .flat_map(|y| (0..4).map(move |x| Point3::try_new(x as f64, y as f64, 0.).unwrap()))
        .collect::<Vec<_>>();
    let mut faces = (0..3)
        .flat_map(|y| {
            (0..3)
                .filter(move |&x| (x, y) != (1, 1))
                .map(move |x| vec![4 * y + x, 4 * y + x + 1, 4 * y + x + 5, 4 * y + x + 4])
        })
        .collect::<Vec<_>>();
    points.extend(
        pair()
            .control_points()
            .iter()
            .map(|p| Point3::try_new(p.x() + 10., p.y(), p.z()).unwrap()),
    );
    faces.extend([vec![16, 17, 18], vec![16, 18, 19]]);
    let source = SubD::try_from_polygons(points, faces).unwrap();
    let result = source
        .merge_face_components(&(0..10).map(SubDMergeComponent::Face).collect::<Vec<_>>())
        .unwrap();
    assert_eq!(result.removed_faces, 1);
    assert_eq!(result.subd.face_ids(), &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(result.merged_faces, vec![8]);
    for i in 0..8 {
        assert_eq!(result.subd.faces()[i], source.faces()[i]);
    }
}
#[test]
fn many_disconnected_regions_retain_independent_component_identities() {
    let prototype = pair();
    let mut points = Vec::new();
    let mut faces = Vec::new();
    for i in 0..300 {
        let offset = points.len() as u32;
        points.extend(
            prototype
                .control_points()
                .iter()
                .map(|p| Point3::try_new(p.x() + 4. * i as f64, p.y(), p.z()).unwrap()),
        );
        faces.extend([
            vec![offset, offset + 1, offset + 2],
            vec![offset, offset + 2, offset + 3],
        ]);
    }
    let source = SubD::try_from_polygons(points, faces).unwrap();
    let result = source
        .merge_face_components(&(0..600).map(SubDMergeComponent::Face).collect::<Vec<_>>())
        .unwrap();
    assert_eq!(result.removed_faces, 300);
    assert_eq!(result.merged_faces.len(), 300);
    assert_eq!(
        result.subd.face_ids(),
        &(0..300).map(|i| 2 * i + 1).collect::<Vec<_>>()
    );
    assert_eq!(result.subd.control_points().len(), 900);
    let restored = SubD::try_from_archive(result.subd.archive()).unwrap();
    assert_eq!(restored.vertex_ids(), result.subd.vertex_ids());
    assert_eq!(restored.edge_ids(), result.subd.edge_ids());
    assert_eq!(restored.face_ids(), result.subd.face_ids());
    assert_eq!(restored.control_points(), result.subd.control_points());
    assert_eq!(restored.surface_points(), result.subd.surface_points());
    assert_eq!(restored.faces(), result.subd.faces());
    assert_eq!(restored.edges(), result.subd.edges());
    assert_eq!(restored.edge_sharpness(), result.subd.edge_sharpness());
}
