#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, Vector3};
use viboceros_smlib::KernelCurve;

fn curve(points: &[[f64; 3]]) -> NurbsCurve {
    NurbsCurve::try_clamped_uniform(
        1,
        points
            .iter()
            .map(|p| Point3::try_new(p[0], p[1], p[2]).unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn parallel_curve_plane_query_ignores_origins_and_preserves_handles() {
    let a = KernelCurve::from_nurbs(&curve(&[[0., 0., 0.], [2., 0., 3.], [4., 0., 0.]])).unwrap();
    let b = KernelCurve::from_nurbs(&curve(&[[0., 8., 0.], [2., 8., 3.], [4., 8., 0.]])).unwrap();
    let t = Tolerance::DEFAULT;
    let before = a.evaluate(0.5).unwrap();
    let n = KernelCurve::parallel_plane_normal(&[&a, &b], t)
        .unwrap()
        .unwrap();
    assert!(n.dot(Vector3::try_new(0., 1., 0.).unwrap()).unwrap().abs() > 1. - 1e-12);
    assert_eq!(a.evaluate(0.5).unwrap(), before);
}

#[test]
fn plane_query_distinguishes_parallel_lines_and_incompatible_directions() {
    let t = Tolerance::DEFAULT;
    let a = KernelCurve::from_nurbs(&curve(&[[0., 0., 0.], [4., 0., 0.]])).unwrap();
    assert!(
        KernelCurve::parallel_plane_normal(&[&a], t)
            .unwrap()
            .is_none()
    );
    let b = KernelCurve::from_nurbs(&curve(&[[0., 0., 3.], [4., 0., 3.]])).unwrap();
    assert!(
        KernelCurve::parallel_plane_normal(&[&a, &b], t)
            .unwrap()
            .is_some()
    );
    let vertical = KernelCurve::from_nurbs(&curve(&[[0., 0., 0.], [0., 0., 4.]])).unwrap();
    assert!(
        KernelCurve::parallel_plane_normal(&[&a, &vertical], t)
            .unwrap()
            .is_some()
    );
    let spatial = KernelCurve::from_nurbs(&curve(&[
        [0., 0., 0.],
        [1., 2., 1.],
        [2., 1., 3.],
        [4., 0., 0.],
    ]))
    .unwrap();
    assert!(
        KernelCurve::parallel_plane_normal(&[&a, &spatial], t)
            .unwrap()
            .is_none()
    );
}
