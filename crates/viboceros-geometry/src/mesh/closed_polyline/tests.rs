use super::*;

#[test]
fn large_boundary_retains_every_segment_without_quadratic_interior_tests() {
    let n = 8192;
    let mut points = (0..n)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as Real / n as Real;
            Point3::try_new(angle.cos() * 100., angle.sin() * 100., 0.).unwrap()
        })
        .collect::<Vec<_>>();
    points.push(points[0]);
    let polyline = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap();
    let mesh = TriangleMesh::try_from_closed_polyline(&polyline)
        .unwrap()
        .unwrap();
    assert_eq!(mesh.vertices().len(), n);
    assert_eq!(mesh.faces().len(), n - 2);
    assert_eq!(mesh.topology().boundary_edge_count, n);
}

#[test]
fn open_and_collinear_inputs_return_no_mesh_and_oversize_inputs_are_bounded() {
    for points in [
        vec![[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]],
        vec![[0., 0., 0.], [2., 0., 0.], [4., 0., 0.], [0., 0., 0.]],
    ] {
        let polyline = Polyline3::try_new(
            points
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert!(
            TriangleMesh::try_from_closed_polyline(&polyline)
                .unwrap()
                .is_none()
        );
    }
    let mut points = (0..MAX_MESH_PATCH_POINTS)
        .map(|i| Point3::try_new(i as Real, 0., 0.).unwrap())
        .collect::<Vec<_>>();
    points.push(points[0]);
    let polyline = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap();
    assert!(matches!(
        TriangleMesh::try_from_closed_polyline(&polyline),
        Err(GeometryError::MeshPatchLimit)
    ));
}
