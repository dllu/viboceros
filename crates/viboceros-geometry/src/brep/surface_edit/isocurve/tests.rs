use super::*;
use crate::{NurbsCurve2, Point2, WeightedPoint2, WeightedPoint3};
fn surface() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        [
            ([0., 0., 0.], 1.),
            ([1., 0., 1.], 2.),
            ([2., 0., 0.], 1.1),
            ([0., 2., 0.], 1.5),
            ([1., 2., -0.5], 0.8),
            ([2., 2., 1.], 1.2),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
        .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn trim(points: Vec<WeightedPoint2>) -> BrepTrim {
    BrepTrim::try_new(
        [0, 0],
        Some(0),
        false,
        NurbsCurve2::try_new_rational(1, points, vec![2., 2., 3., 7., 9., 10., 10.]).unwrap(),
        BrepTrimType::Boundary,
        SurfaceIso::NotIso,
        [0.; 2],
    )
    .unwrap()
}
fn points() -> Vec<WeightedPoint2> {
    [[0.1, 0.2], [0.8, 0.2], [0.8, 0.9], [0.1, 0.9], [0.1, 0.2]]
        .map(|p| WeightedPoint2::try_new(Point2::try_from(p).unwrap(), 1.).unwrap())
        .to_vec()
}
#[test]
fn composite_isotrim_retains_clock_rational_geometry_and_corner_partitions() {
    let surface = surface();
    let before = surface.clone();
    let trim = trim(points());
    let image = image(&trim, &surface).unwrap().unwrap();
    assert_eq!(image.domain(), trim.curve().domain());
    assert_eq!(image.degree(), 2);
    assert!(
        surface
            .parameter_curve_deviation_bound(trim.curve(), &image, 1e-10)
            .unwrap()
            .is_some()
    );
    for i in 0..=64 {
        let t = 2. + 8. * i as f64 / 64.;
        let uv = trim.curve().evaluate(t).unwrap();
        assert!(
            image
                .evaluate(t)
                .unwrap()
                .distance_to(surface.evaluate(uv.x(), uv.y()).unwrap())
                .unwrap()
                < 1e-12
        );
    }
    assert_eq!(surface, before);
}
#[test]
fn diagonal_or_nonlinear_parameter_segments_use_general_fitting() {
    let s = surface();
    let mut p = points();
    p[1] = WeightedPoint2::try_new(Point2::try_new(0.8, 0.3).unwrap(), 1.).unwrap();
    assert!(image(&trim(p), &s).unwrap().is_none());
    let mut p = points();
    p[1] = WeightedPoint2::try_new(p[1].point(), 2.).unwrap();
    assert!(image(&trim(p), &s).unwrap().is_none());
}
