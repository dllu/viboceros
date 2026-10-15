use viboceros_geometry::{Brep, Point3, Tolerance, TriangleMesh};
use viboceros_io::{
    ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file,
};
#[test]
fn merged_mesh_hole_and_colors_survive_three_dm_and_nurbs_conversion() {
    let t = Tolerance::DEFAULT;
    let vertices = [
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 3., 0.],
        [0., 3., 0.],
        [1., 1., 0.],
        [2., 1., 0.],
        [2., 2., 0.],
        [1., 2., 0.],
    ]
    .map(|p| Point3::try_from(p).unwrap())
    .to_vec();
    let source = TriangleMesh::try_new(
        vertices,
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
    .try_with_vertex_colors(Some(vec![[12, 34, 56, 78]; 8]))
    .unwrap();
    let (mesh, _) = source.merge_coplanar_faces(t).unwrap();
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [255; 3],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(ThreeDmGeometry::Mesh(mesh.clone()), 0)],
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ring.3dm");
    write_3dm_file(&path, &model).unwrap();
    let loaded = read_3dm_file(&path, t).unwrap();
    assert_eq!(loaded.unsupported_object_count(), 0);
    assert_eq!(loaded.objects[0].geometry, ThreeDmGeometry::Mesh(mesh));
    let ThreeDmGeometry::Mesh(m) = &loaded.objects[0].geometry else {
        panic!()
    };
    let b = Brep::try_from_mesh_with_ngons(m, true, true, t).unwrap();
    assert_eq!(b.faces().len(), 1);
    assert_eq!(b.faces()[0].loops().len(), 2);
    assert!((b.area(t).unwrap() - 8.).abs() < 1e-10);
}
