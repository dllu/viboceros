use super::*;
#[test]
fn shallow_subd_creases_keep_distinct_native_sector_normals() {
    let points = [
        [0., 0., 0.],
        [2., 0., 0.],
        [4., 0., 0.5],
        [0., 3., 0.],
        [2., 3., 0.],
        [4., 3., 0.5],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let subd = viboceros_geometry::SubD::try_from_polygons(
        points,
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap();
    let seam = subd
        .edges()
        .iter()
        .position(|e| *e == [1, 4] || *e == [4, 1])
        .unwrap();
    let subd = subd.try_with_edge_tags(&[(seam, 2)]).unwrap();
    let display =
        display_cache::DisplayGeometry::new(Geometry::SubD(subd).into(), 1, Tolerance::DEFAULT);
    let mesh = display.mesh().unwrap();
    let normals = display.normals();
    let mut seam_normals = Vec::new();
    for (triangle, normals) in mesh.triangles().iter().zip(normals) {
        for (&index, &normal) in triangle.iter().zip(normals) {
            let p = mesh.vertices()[index as usize];
            if p.distance_to(Point3::try_new(2., 1.5, 0.).unwrap())
                .unwrap()
                < 1e-12
            {
                seam_normals.push(normal);
            }
            assert!((normal.norm() - 1.).abs() < 1e-12);
        }
    }
    assert!(seam_normals.len() >= 2);
    assert!(
        seam_normals
            .iter()
            .any(|a| seam_normals.iter().any(|b| a.dot(b) < 0.99)),
        "crease normals were averaged across sectors"
    );
}
