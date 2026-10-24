use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn mesh(vertices: Vec<Point3>, faces: Vec<MeshFace>) -> TriangleMesh {
    TriangleMesh::try_from_face_records(vertices, faces).unwrap()
}
#[test]
fn face_owned_channels_interpolate_uv_colors_and_unit_normals() {
    let source = mesh(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 4., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .try_with_texture_coordinates(Some(vec![[0., 0.], [4., 0.], [0., 4.]]))
    .unwrap()
    .try_with_vertex_colors(Some(vec![[0, 0, 0, 0], [200, 0, 0, 200], [0, 200, 0, 100]]))
    .unwrap()
    .try_with_vertex_normals(Some(vec![
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
    ]))
    .unwrap();
    let part = mesh(
        vec![p(1., 1., 0.), p(2., 1., 0.), p(1., 2., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    );
    let result = MeshChannelSource::new(&source)
        .unwrap()
        .transfer(part, &[0])
        .unwrap();
    assert_eq!(result.texture_coordinates().unwrap()[0], [1., 1.]);
    assert_eq!(result.vertex_colors().unwrap()[0], [50, 50, 0, 75]);
    let n = result.vertex_normals().unwrap()[0].to_array();
    for (a, b) in n
        .into_iter()
        .zip([2. / 6_f64.sqrt(), 1. / 6_f64.sqrt(), 1. / 6_f64.sqrt()])
    {
        assert!((a - b).abs() < 1e-7);
    }
}
#[test]
fn coincident_source_seams_keep_distinct_uv_normals_and_colors() {
    let source = mesh(
        vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(1., 1., 0.),
            p(0., 1., 0.),
            p(1., 0., 0.),
            p(2., 0., 0.),
            p(2., 1., 0.),
            p(1., 1., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
    )
    .try_with_texture_coordinates(Some(vec![
        [0., 0.],
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [10., 0.],
        [11., 0.],
        [11., 1.],
        [10., 1.],
    ]))
    .unwrap()
    .try_with_vertex_colors(Some(
        vec![[1, 2, 3, 4]; 4]
            .into_iter()
            .chain(vec![[50, 60, 70, 80]; 4])
            .collect(),
    ))
    .unwrap()
    .try_with_vertex_normals(Some(
        vec![Vector3::try_new(0., 0., 1.).unwrap(); 4]
            .into_iter()
            .chain(vec![Vector3::try_new(0., 1., 0.).unwrap(); 4])
            .collect(),
    ))
    .unwrap();
    let part = mesh(
        vec![
            p(0., 0.5, 0.),
            p(1., 0.5, 0.),
            p(1., 1., 0.),
            p(0., 1., 0.),
            p(2., 0.5, 0.),
            p(2., 1., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([1, 4, 5, 2])],
    );
    let result = MeshChannelSource::new(&source)
        .unwrap()
        .transfer(part, &[0, 1])
        .unwrap();
    let seam = result
        .vertices
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == p(1., 0.5, 0.))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert_eq!(seam.len(), 2);
    assert_eq!(result.texture_coordinates().unwrap()[seam[0]], [1., 0.5]);
    assert_eq!(result.texture_coordinates().unwrap()[seam[1]], [10., 0.5]);
    assert_eq!(result.vertex_colors().unwrap()[seam[0]], [1, 2, 3, 4]);
    assert_eq!(result.vertex_colors().unwrap()[seam[1]], [50, 60, 70, 80]);
    assert_ne!(
        result.vertex_normals().unwrap()[seam[0]],
        result.vertex_normals().unwrap()[seam[1]]
    );
    assert_eq!(result.area().unwrap(), 1.);
}
#[test]
fn original_corners_keep_nonunit_normals_verbatim_and_colors_can_stand_alone() {
    let source = mesh(
        vec![p(0., 0., 0.), p(2., 0., 0.), p(0., 2., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .try_with_vertex_normals(Some(vec![Vector3::try_new(2., 3., 4.).unwrap(); 3]))
    .unwrap();
    let result = MeshChannelSource::new(&source)
        .unwrap()
        .transfer(source.clone(), &[0])
        .unwrap();
    assert_eq!(result.vertex_normals(), source.vertex_normals());
    let source = source
        .try_with_vertex_normals(None)
        .unwrap()
        .try_with_vertex_colors(Some(vec![[30, 60, 90, 120]; 3]))
        .unwrap();
    let result = MeshChannelSource::new(&source)
        .unwrap()
        .transfer(source.clone(), &[0])
        .unwrap();
    assert_eq!(result.vertex_colors(), source.vertex_colors());
    assert!(result.vertex_normals().is_none());
    assert!(result.texture_coordinates().is_none());
}
#[test]
fn quad_interpolation_uses_the_native_diagonal_and_rejects_invalid_ownership() {
    let source = mesh(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(4., 3., 0.), p(0., 3., 0.)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )
    .try_with_texture_coordinates(Some(vec![[-1., 0.], [2., 0.], [4., 3.], [-1., 1.]]))
    .unwrap();
    let part = mesh(
        vec![p(2., 1.5, 0.), p(2., 0., 0.), p(4., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    );
    let prepared = MeshChannelSource::new(&source).unwrap();
    assert!(prepared.transfer(part.clone(), &[]).is_err());
    assert!(prepared.transfer(part.clone(), &[1]).is_err());
    let result = prepared.transfer(part, &[0]).unwrap();
    assert_eq!(result.texture_coordinates().unwrap()[0], [1.5, 1.5]);
}
#[test]
fn barycentrics_handle_overflowing_edges_and_extremely_thin_triangles() {
    let actual = weights::triangle(
        p(0., 5e307, 0.),
        [p(-1e308, 0., 0.), p(1e308, 0., 0.), p(0., 1e308, 0.)],
    )
    .unwrap();
    assert_eq!(actual, [0.25, 0.25, 0.5]);
    let actual = weights::triangle(
        p(0.75, 2.5e-301, 0.),
        [p(0., 0., 0.), p(1., 0., 0.), p(1., 1e-300, 0.)],
    )
    .unwrap();
    for (a, b) in actual.into_iter().zip([0.25, 0.5, 0.25]) {
        assert!((a - b).abs() < 1e-12);
    }
    assert!(
        weights::triangle(p(1., 0., 0.), [p(0., 0., 0.), p(1., 0., 0.), p(2., 0., 0.)]).is_err()
    );
}
