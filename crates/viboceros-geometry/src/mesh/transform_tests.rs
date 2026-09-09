use super::*;
use crate::Vector3;

#[test]
fn reflection_and_composition_retain_mesh_faces_and_signed_volume_parity() {
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let frame = Frame3::try_from_normal(
        origin,
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source =
        TriangleMesh::try_box_grid(frame, [[0., 1.]; 3], 1, 1, 1, Tolerance::DEFAULT).unwrap();
    let original = source.clone();
    let first = AffineTransform3::try_new(
        [[-2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(3., -4., 5.).unwrap(),
    )
    .unwrap();
    let reflected = source.transformed(first, Tolerance::DEFAULT).unwrap();
    assert_eq!(reflected.faces(), source.faces());
    assert!((reflected.signed_volume().unwrap() + 24.).abs() < 1e-12);
    let second = AffineTransform3::try_new(
        [[0., 1., 0.], [1., 0., 0.], [0., 0., 1.]],
        Vector3::try_new(-7., 8., -9.).unwrap(),
    )
    .unwrap();
    let sequential = reflected.transformed(second, Tolerance::DEFAULT).unwrap();
    let composed = source
        .transformed(first.then(second).unwrap(), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(composed, sequential);
    assert_eq!(composed.faces(), source.faces());
    assert!((composed.signed_volume().unwrap() - 24.).abs() < 1e-12);
    assert_eq!(source, original);
}
