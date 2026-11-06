use super::*;
use crate::{Circle3, Polyline3};
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_points(
        p(0., 0.),
        p(1., 0.),
        p(0., 1.),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
fn path(points: &[[Real; 2]]) -> NurbsCurve {
    Polyline3::try_new(
        points.iter().map(|p_| p(p_[0], p_[1])).collect(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
    .to_native_nurbs()
    .unwrap()
}
fn area(boundaries: &[NurbsCurve]) -> Real {
    boundaries
        .iter()
        .map(|c| {
            Brep::try_planar_face(c, Tolerance::default())
                .unwrap()
                .area(Tolerance::default())
                .unwrap()
        })
        .sum()
}
#[test]
fn filled_network_uses_unrestricted_spans_to_connect_tangent_regions() {
    let paths = vec![
        path(&[[0., 0.], [10., 0.], [10., 10.], [0., 10.], [0., 0.]]),
        path(&[[10., 2.], [14., 2.], [14., 6.], [10., 6.], [10. + 3e-6, 2.]]),
    ];
    let before = paths.clone();
    let boundaries =
        planar_curve_network_boundaries(&paths, frame(), Tolerance::default()).unwrap();
    assert_eq!(boundaries.len(), 1);
    assert!((area(&boundaries) - 116.).abs() < 1e-10);
    assert_eq!(paths, before);
}
#[test]
fn filled_network_orders_nearly_parallel_straight_rays_without_false_bend() {
    let paths = vec![
        path(&[[0., 0.], [10., 0.], [10., 10.], [0., 10.], [0., 0.]]),
        path(&[
            [10., 2.],
            [14., 2.],
            [14., 6.],
            [10., 6.],
            [10. + 3e-6, 2. + 1e-9],
        ]),
        path(&[
            [10. + 3e-6, 2. + 0.5e-9],
            [14., 2.],
            [14., 6.],
            [10., 6.],
            [10. + 3e-6, 2. + 0.5e-9],
        ]),
    ];
    let boundaries =
        planar_curve_network_boundaries(&paths, frame(), Tolerance::default()).unwrap();
    assert_eq!(boundaries.len(), 1);
    assert!((area(&boundaries) - 116.).abs() < 1e-10);
}
#[test]
fn filled_network_respects_rational_line_speed_at_interior_crossings() {
    let bottom = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(4., 0.), 4.).unwrap(),
        ],
        vec![12., 12., 20., 20.],
    )
    .unwrap();
    let paths = vec![
        bottom,
        path(&[[4., 0.], [4., 4.], [0., 4.], [0., 0.]]),
        path(&[[1., 0.], [1., -2.], [3., -2.], [3., 0.]]),
    ];
    let boundaries =
        planar_curve_network_boundaries(&paths, frame(), Tolerance::default()).unwrap();
    assert_eq!(boundaries.len(), 1);
    assert!((area(&boundaries) - 20.).abs() < 1e-10);
}
#[test]
fn filled_network_ignores_branches_and_keeps_point_contacts_separate() {
    let circle = Circle3::try_new(p(0., 0.), 4., frame().z_axis(), Tolerance::default())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let paths = vec![
        circle,
        path(&[[4., 0.], [8., 0.], [8., 4.], [4., 4.], [4., 0.]]),
        path(&[[-1., -1.], [1., -1.], [1., 1.], [-1., 1.], [-1., -1.]]),
        path(&[[4., 0.], [4., 2.]]),
    ];
    let boundaries =
        planar_curve_network_boundaries(&paths, frame(), Tolerance::default()).unwrap();
    assert_eq!(boundaries.len(), 2);
    assert!((area(&boundaries) - (16. * std::f64::consts::PI + 16.)).abs() < 1e-9);
}
