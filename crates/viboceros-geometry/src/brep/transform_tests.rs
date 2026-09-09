//! End-to-end affine composition, topology, and material-side checks.
use super::*;

#[test]
fn composed_box_transforms_match_sequential_geometry_and_reflection_parity() {
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let frame = Frame3::try_from_normal(
        origin,
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = Brep::try_box(frame, [[0., 1.]; 3], Tolerance::DEFAULT).unwrap();
    let original = source.clone();
    let first = AffineTransform3::try_new(
        [[-2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(3., -4., 5.).unwrap(),
    )
    .unwrap();
    for swap in [false, true] {
        let rows = if swap {
            [[0., 1., 0.], [1., 0., 0.], [0., 0., 1.]]
        } else {
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
        };
        let second =
            AffineTransform3::try_new(rows, Vector3::try_new(-7., 8., -9.).unwrap()).unwrap();
        let sequential = source
            .transformed(first, Tolerance::DEFAULT)
            .unwrap()
            .transformed(second, Tolerance::DEFAULT)
            .unwrap();
        let composed = source
            .transformed(first.then(second).unwrap(), Tolerance::DEFAULT)
            .unwrap();
        // Integer matrices/coordinates and unit second-map scale avoid rounding
        // ambiguity, including propagated component tolerances.
        assert_eq!(composed, sequential);
        assert!(composed.is_solid());
        assert_eq!(composed.vertices().len(), source.vertices().len());
        assert_eq!(composed.edges().len(), source.edges().len());
        assert_eq!(composed.faces().len(), source.faces().len());
        assert!((composed.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-12);
        for (before, after) in source.faces().iter().zip(composed.faces()) {
            assert_eq!(after.is_reversed(), before.is_reversed() ^ !swap);
            assert_eq!(after.loops, before.loops);
        }
    }
    assert_eq!(source, original);
}
