use super::*;
use crate::WeightedPoint3;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn saddle(scale: Real) -> NurbsSurface {
    NurbsSurface::try_clamped_uniform(
        1,
        1,
        2,
        2,
        vec![
            p(0., 0., 0.),
            p(scale, 0., 0.),
            p(0., scale, 0.),
            p(scale, scale, scale),
        ],
    )
    .unwrap()
}

#[test]
fn closest_point_exact_hits_include_corners_grid_interiors_and_constant_patches() {
    let source = saddle(4.);
    for (u, v) in [(0., 0.), (0.5, 0.5), (1., 1.), (1., 0.5)] {
        let target = p(4. * u, 4. * v, 4. * u * v);
        assert_eq!(
            source
                .closest_parameters(target, Tolerance::DEFAULT)
                .unwrap(),
            (u, v)
        );
    }
    let target = p(3., -4., 5.);
    let constant = NurbsSurface::try_clamped_uniform(2, 2, 3, 3, vec![target; 9]).unwrap();
    assert_eq!(
        constant
            .closest_parameters(target, Tolerance::DEFAULT)
            .unwrap(),
        (0., 0.)
    );
}

#[test]
fn closest_point_exact_hit_requires_coordinate_equality_not_model_tolerance() {
    let surface = saddle(4.);
    let target = p(0.2, 0.2, 0.01);
    let tolerance = Tolerance::try_new(1., 1e-12, 1e-10).unwrap();
    let (u, v) = surface.closest_parameters(target, tolerance).unwrap();
    let point = surface.evaluate(u, v).unwrap();
    assert!(point.distance_to(target).unwrap() < p(0., 0., 0.).distance_to(target).unwrap());
    assert_ne!((u, v), (0., 0.));
}

#[test]
fn closest_point_exact_hit_checks_denominator_before_constant_point_shortcut() {
    let target = p(3., -4., 5.);
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        [1., -1., 2., 1., -1., 2.]
            .into_iter()
            .map(|w| WeightedPoint3::try_new(target, w).unwrap())
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    // At the native start, W = (1 + -1)/2 = 0 despite a constant control net.
    assert_eq!(
        surface.evaluate(0., 0.),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    let (u, v) = surface
        .closest_parameters(target, Tolerance::DEFAULT)
        .unwrap();
    assert_ne!(u, 0.);
    assert_eq!(surface.evaluate(u, v).unwrap(), target);
}

#[test]
fn closest_point_exact_hit_does_not_confuse_subnormal_separation_with_zero() {
    let tiny = Real::from_bits(1);
    let surface = saddle(4. * tiny);
    let target = p(tiny, tiny, 0.);
    let (u, v) = surface
        .closest_parameters(target, Tolerance::DEFAULT)
        .unwrap();
    assert_ne!((u, v), (0., 0.));
    assert_eq!(surface.evaluate(u, v).unwrap(), target);
}
