use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn repair_preserves_actual_area_attributes_and_native_validity() {
    let t = Tolerance::DEFAULT;
    let source = TriangleMesh::try_from_face_records(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(2., 2., 0.),
            p(0., 0., 0.),
            p(3., 0., 0.),
            p(4., 0., 0.),
            p(5., 0., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 5, 6])],
    )
    .unwrap()
    .try_with_vertex_colors(Some((0..7).map(|i| [i, 2, 3, 4]).collect()))
    .unwrap();
    let before = source.clone();
    let result = source.culled_degenerate_faces().unwrap();
    assert_eq!(
        (
            result.removed_faces,
            result.repaired_faces,
            result.removed_vertices
        ),
        (1, 1, 4)
    );
    assert!(result.changed());
    let mesh = result.mesh.unwrap();
    assert_eq!(mesh.face_count(), 1);
    assert!((mesh.area().unwrap() - 2.).abs() < 1e-12);
    assert_eq!(
        mesh.vertex_colors().unwrap(),
        &[[0, 2, 3, 4], [1, 2, 3, 4], [2, 2, 3, 4]]
    );
    mesh.validate_face_geometry(t).unwrap();
    assert_eq!(source, before);
    let again = mesh.culled_degenerate_faces().unwrap();
    assert!(!again.changed());
    assert_eq!(again.mesh, Some(mesh));
}
#[test]
fn empty_and_finite_native_unset_faces_have_explicit_absence() {
    for end in [p(2., 0., 0.), p(-1.23432101234321e308, 0., 0.)] {
        let source = TriangleMesh::try_from_face_records(
            vec![p(0., 0., 0.), p(1., 0., 0.), end],
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap();
        let result = source.culled_degenerate_faces().unwrap();
        assert_eq!(result.removed_faces, 1);
        assert!(result.mesh.is_none());
    }
}
#[test]
fn repaired_ngon_boundary_and_hole_free_region_remain_consistent() {
    let t = Tolerance::DEFAULT;
    let source = TriangleMesh::try_new_faces(
        vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(1., 1., 0.),
            p(0., 1., 0.),
            p(-1., 0., 0.),
            p(-1., 1., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([3, 4, 5])],
        t,
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0])])
    .unwrap();
    let mut points = source.vertices().to_vec();
    points[3] = points[0];
    let source = source.try_with_edited_vertices(points).unwrap();
    let result = source.culled_degenerate_faces().unwrap();
    assert_eq!(result.repaired_faces, 1);
    let mesh = result.mesh.unwrap();
    assert_eq!(mesh.ngons().len(), 1);
    assert_eq!(mesh.ngons()[0].vertices(), &[0, 1, 2]);
}

#[test]
fn cleanup_keeps_unaffected_annulus_hole_and_unrelated_remote_faces() {
    let t = Tolerance::DEFAULT;
    let ring = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(3., 0., 0.),
            p(3., 3., 0.),
            p(0., 3., 0.),
            p(1., 1., 0.),
            p(2., 1., 0.),
            p(2., 2., 0.),
            p(1., 2., 0.),
        ],
        vec![
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
        t,
    )
    .unwrap()
    .merge_coplanar_faces(t)
    .unwrap()
    .0;
    let remote = TriangleMesh::try_new(
        vec![p(1e6, 0., 0.), p(1e6 + 1., 0., 0.), p(1e6, 1., 0.)],
        vec![[0, 1, 2]],
        t,
    )
    .unwrap();
    let bad = TriangleMesh::try_from_face_records(
        vec![p(8., 0., 0.), p(9., 0., 0.), p(10., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let source = TriangleMesh::try_append(&[&ring, &remote, &bad]).unwrap();
    let result = source.culled_degenerate_faces().unwrap();
    assert_eq!(
        (
            result.removed_faces,
            result.repaired_faces,
            result.removed_vertices
        ),
        (1, 0, 3)
    );
    let mesh = result.mesh.unwrap();
    assert_eq!(mesh.ngons().len(), 1);
    assert_eq!(mesh.ngons()[0].faces().len(), 8);
    assert!((mesh.area().unwrap() - 8.5).abs() < 1e-12);
    assert_eq!(mesh.visible_wireframe_lines(t).unwrap().len(), 11);
    let brep = crate::Brep::try_from_mesh_with_ngons(&mesh, true, true, t).unwrap();
    let mut loops = brep
        .faces()
        .iter()
        .map(|f| f.loops().len())
        .collect::<Vec<_>>();
    loops.sort_unstable();
    assert_eq!(loops, vec![1, 2]);
    assert!((brep.area(t).unwrap() - 8.5).abs() < 1e-7);
}

#[test]
fn disconnected_ngon_dissolves_without_welding_remaining_facets() {
    let source = TriangleMesh::try_new_faces(
        (0..2)
            .flat_map(|y| (0..4).map(move |x| p(x as f64, y as f64, 0.)))
            .collect(),
        vec![
            MeshFace::Quad([0, 1, 5, 4]),
            MeshFace::Quad([1, 2, 6, 5]),
            MeshFace::Quad([2, 3, 7, 6]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(
        vec![0, 1, 2, 3, 7, 6, 5, 4],
        vec![0, 1, 2],
    )])
    .unwrap();
    let mut points = source.vertices().to_vec();
    points[1] = points[0];
    let source = source.try_with_edited_vertices(points).unwrap();
    let result = source.culled_degenerate_faces().unwrap();
    assert_eq!(
        (
            result.removed_faces,
            result.repaired_faces,
            result.removed_vertices
        ),
        (0, 1, 0)
    );
    let mesh = result.mesh.unwrap();
    assert!(mesh.ngons().is_empty());
    assert_eq!(mesh.face_count(), 3);
    assert_eq!(mesh.vertices().len(), 8);
    assert_eq!(mesh.vertices()[0], mesh.vertices()[1]);
    assert!((mesh.area().unwrap() - 3.).abs() < 1e-12);
}
