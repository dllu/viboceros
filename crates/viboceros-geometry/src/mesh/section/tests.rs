use super::*;
use crate::Vector3;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> Frame3 {
    Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn shared_diagonal_intersections_join_once_and_follow_face_orientation() {
    let mesh = TriangleMesh::try_new_faces(
        vec![
            p(-2., -2., 0.),
            p(2., -2., 0.),
            p(2., 2., 0.),
            p(-2., 2., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curves = mesh
        .section_with_plane(plane(), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(curves.len(), 1);
    assert_eq!(curves[0].vertices().len(), 3);
    assert_eq!(curves[0].vertices()[0], p(2., 0., 0.));
    assert_eq!(curves[0].vertices()[2], p(-2., 0., 0.));
}
#[test]
fn coplanar_face_perimeter_drops_internal_triangulation_edges() {
    let mesh = TriangleMesh::try_new_faces(
        vec![
            p(-2., 0., -2.),
            p(2., 0., -2.),
            p(2., 0., 2.),
            p(-2., 0., 2.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curves = mesh
        .section_with_plane(plane(), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(curves.len(), 1);
    assert_eq!(curves[0].vertices().len(), 5);
    assert_eq!(curves[0].vertices().first(), curves[0].vertices().last());
}
#[test]
fn distant_tiny_mesh_sections_do_not_lose_the_shared_exact_intersection() {
    let base = 2_f64.powi(40);
    let step = 2_f64.powi(-10);
    let mesh = TriangleMesh::try_new(
        vec![
            p(base, -step, 0.),
            p(base + step, step, 0.),
            p(base - step, step, 0.),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curves = mesh
        .section_with_plane(plane(), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(curves.len(), 1);
    for point in curves[0].vertices() {
        assert_eq!(point.y(), 0.);
        assert!(point.x() >= base - step && point.x() <= base + step);
    }
}

#[test]
fn rounded_near_vertex_hits_do_not_create_a_zero_length_output_chord() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(-4., -4., 0.),
            p(4., -4., 0.),
            p(4., 4., 0.),
            p(-4., 4., 0.),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let index = crate::MeshProjectionIndex::new(&mesh).unwrap();
    let a = index.closest_point(p(1., 3., 0.)).unwrap().unwrap();
    let b = index.closest_point(p(-2., 2., 0.)).unwrap().unwrap();
    let direction = a.vector_to(b).unwrap();
    let normal = direction
        .cross(Vector3::try_new(0., 0., 1.).unwrap())
        .unwrap();
    let frame =
        Frame3::try_from_x_and_normal(a, direction, normal, Tolerance::NUMERICAL_VALIDATION)
            .unwrap();
    let sections = mesh
        .section_with_plane(frame, Tolerance::NUMERICAL_VALIDATION)
        .unwrap();
    assert_eq!(sections.len(), 1);
    assert!(
        sections[0]
            .vertices()
            .windows(2)
            .all(|pair| pair[0] != pair[1])
    );
    assert!(
        sections[0]
            .closest_point(a, Tolerance::NUMERICAL_VALIDATION)
            .unwrap()
            .distance_to(a)
            .unwrap()
            < 1e-12
    );
}
