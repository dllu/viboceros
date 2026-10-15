#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Point3, Tolerance, TriangleMesh};
use viboceros_smlib::Shape;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn adjacent_triangles_merge_without_changing_area_or_source() {
    let t = Tolerance::DEFAULT;
    let mesh = TriangleMesh::try_new(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(4., 3., 0.), p(0., 3., 0.)],
        vec![[0, 1, 2], [0, 2, 3]],
        t,
    )
    .unwrap();
    let b = Brep::try_from_mesh(&mesh, true, t).unwrap();
    let original = b.clone();
    assert_eq!(b.faces().len(), 2);
    let shape = Shape::from_brep(&b, t).unwrap();
    let (merged, count) = shape.merge_coplanar_faces(t).unwrap().unwrap();
    let result = merged.to_brep(t).unwrap();
    assert_eq!(count, 1);
    assert_eq!(result.faces().len(), 1);
    assert!((result.area(t).unwrap() - 12.).abs() < 1e-8);
    assert!(!result.is_solid());
    assert_eq!(b, original);
    assert_eq!(shape.to_brep(t).unwrap().faces().len(), 2);
    assert!(merged.merge_coplanar_faces(t).unwrap().is_none());
}

fn mesh_brep(points: Vec<Point3>, triangles: Vec<[u32; 3]>) -> Brep {
    let t = Tolerance::DEFAULT;
    Brep::try_from_mesh(
        &TriangleMesh::try_new(points, triangles, t).unwrap(),
        true,
        t,
    )
    .unwrap()
}
fn check_merge(source: Brep, expected_faces: usize, area: f64, loops: Option<usize>) -> Brep {
    let t = Tolerance::DEFAULT;
    let before = source.clone();
    let shape = Shape::from_brep(&source, t).unwrap();
    let (merged, removed) = shape.merge_coplanar_faces(t).unwrap().unwrap();
    assert_eq!(removed, source.faces().len() - expected_faces);
    let result = merged.to_brep(t).unwrap();
    assert_eq!(result.faces().len(), expected_faces);
    assert!((result.area(t).unwrap() - area).abs() < 1e-7);
    assert_eq!(result.is_solid(), source.is_solid());
    if let Some(loops) = loops {
        assert_eq!(result.faces()[0].loops().len(), loops);
    }
    assert_eq!(source, before);
    assert!(merged.merge_coplanar_faces(t).unwrap().is_none());
    result
}
#[test]
fn annulus_preserves_inner_hole_and_original_vertices() {
    let points = vec![
        p(0., 0., 0.),
        p(3., 0., 0.),
        p(3., 3., 0.),
        p(0., 3., 0.),
        p(1., 1., 0.),
        p(2., 1., 0.),
        p(2., 2., 0.),
        p(1., 2., 0.),
    ];
    let faces = vec![
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let b = mesh_brep(points, faces);
    let result = check_merge(b.clone(), 1, 8., Some(2));
    for vertex in b.vertices() {
        assert!(
            result
                .vertices()
                .iter()
                .any(|v| v.point() == vertex.point())
        );
    }
}
#[test]
fn triangulated_closed_box_merges_only_planar_interfaces() {
    let points = vec![
        p(0., 0., 0.),
        p(2., 0., 0.),
        p(2., 3., 0.),
        p(0., 3., 0.),
        p(0., 0., 4.),
        p(2., 0., 4.),
        p(2., 3., 4.),
        p(0., 3., 4.),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let result = check_merge(mesh_brep(points, faces), 6, 52., None);
    assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-8);
}
#[test]
fn disconnected_regions_merge_independently() {
    let points = vec![
        p(0., 0., 0.),
        p(1., 0., 0.),
        p(1., 1., 0.),
        p(0., 1., 0.),
        p(3., 0., 0.),
        p(4., 0., 0.),
        p(4., 1., 0.),
        p(3., 1., 0.),
    ];
    check_merge(
        mesh_brep(points, vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]]),
        2,
        2.,
        Some(1),
    );
}
#[test]
fn kinked_faces_and_cylindrical_seam_are_noops() {
    let t = Tolerance::DEFAULT;
    let b = mesh_brep(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.01)],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    assert!(
        Shape::from_brep(&b, t)
            .unwrap()
            .merge_coplanar_faces(t)
            .unwrap()
            .is_none()
    );
    let frame = viboceros_geometry::Frame3::try_from_normal(
        p(0., 0., 0.),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        t,
    )
    .unwrap();
    let b = Brep::try_cylinder(frame, 2., 0., 5., t).unwrap();
    assert!(
        Shape::from_brep(&b, t)
            .unwrap()
            .merge_coplanar_faces(t)
            .unwrap()
            .is_none()
    );
}

#[test]
fn rational_closed_interface_merges_without_rebuilding_outer_trims() {
    use viboceros_geometry::{Circle3, NurbsSurface, UnitVector3, join_breps};
    let t = Tolerance::DEFAULT;
    let surface = NurbsSurface::try_bilinear([
        p(-3., -3., 0.),
        p(3., -3., 0.),
        p(3., 3., 0.),
        p(-3., 3., 0.),
    ])
    .unwrap();
    let circle = Circle3::try_new(
        p(0., 0., 0.),
        1.,
        UnitVector3::try_new(0., 0., 1., t).unwrap(),
        t,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let pieces = Brep::try_split_rectangular_surface_face_with_closed_curve(
        surface,
        0. ..=1.,
        0. ..=1.,
        circle,
        false,
        t,
    )
    .unwrap();
    let joined = join_breps(&[&pieces[0], &pieces[1]], t.absolute(), t).unwrap();
    assert_eq!(joined.len(), 1);
    let b = joined[0].brep.clone();
    assert_eq!(b.faces().len(), 2);
    assert!(b.edges().iter().any(|e| e.curve().degree() == 2));
    check_merge(b, 1, 36., None);
}
#[test]
fn overlapping_sheet_interiors_are_rejected_before_merging() {
    let t = Tolerance::DEFAULT;
    let b = mesh_brep(
        vec![
            p(0., 0., 0.),
            p(4., 0., 0.),
            p(4., 3., 0.),
            p(0., 3., 0.),
            p(2., -1., -1.),
            p(2., 4., -1.),
            p(2., 4., 1.),
            p(2., -1., 1.),
        ],
        vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
    );
    let before = b.clone();
    let shape = Shape::from_brep(&b, t).unwrap();
    let error = shape
        .merge_coplanar_faces(t)
        .err()
        .expect("transverse interiors must fail");
    assert!(
        error.to_string().contains("non-topological face contacts")
            || error.to_string().contains("shared topological"),
        "{error}"
    );
    assert_eq!(b, before);
    assert_eq!(shape.to_brep(t).unwrap().faces().len(), 4);
}
