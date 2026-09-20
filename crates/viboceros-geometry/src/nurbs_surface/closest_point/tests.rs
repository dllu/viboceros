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

#[test]
fn closest_point_candidates_survive_overflowing_distances() {
    // S=(u,v,uv) on the unit square. All coordinates increase toward (1,1),
    // which is the unique nearest point to (MAX,MAX,MAX). Every distance
    // overflows, but candidate ordering and the minimizer remain defined.
    let surface = saddle(1.);
    let target = p(Real::MAX, Real::MAX, Real::MAX);
    assert!(target.distance_to(p(1., 1., 1.)).is_err());
    assert_eq!(
        surface
            .closest_parameters(target, Tolerance::DEFAULT)
            .unwrap(),
        (1., 1.)
    );
    // Also cover an overflowing coordinate subtraction, not just the norm.
    let surface = NurbsSurface::try_clamped_uniform(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|j| (0..3).map(move |i| p(-Real::MAX, i as Real / 2., j as Real / 2.)))
            .collect(),
    )
    .unwrap();
    let expected = p(-Real::MAX, 1., 1.);
    assert!(target.vector_to(expected).is_err());
    let (u, v) = surface
        .closest_parameters(target, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(surface.evaluate(u, v).unwrap(), expected);
}

#[test]
fn closest_point_seed_ranking_retains_a_distant_interior_peak() {
    // S=(u,v²,256u²(1-u)²v²(1-v)²). The highest point is uniquely
    // (.5,.25,1), directly below the target. The entire v=0 boundary is
    // singular, so starts restricted to that row cannot refine toward it.
    let surface = NurbsSurface::try_clamped_uniform(
        4,
        4,
        5,
        5,
        (0_usize..5)
            .flat_map(|j| {
                (0..5).map(move |i| {
                    p(
                        i as Real / 4.,
                        (j * j.saturating_sub(1)) as Real / 12.,
                        if i == 2 && j == 2 { 64. / 9. } else { 0. },
                    )
                })
            })
            .collect(),
    )
    .unwrap();
    let peak = p(0.5, 0.25, 1.);
    assert_eq!(surface.evaluate(0.5, 0.5).unwrap(), peak);
    for height in [1e20, 1e100, Real::MAX] {
        let target = p(0.5, 0.25, height);
        assert_eq!(
            target.distance_to(peak).unwrap(),
            target.distance_to(p(0., 0., 0.)).unwrap()
        );
        let (u, v) = surface
            .closest_parameters(target, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(
            surface.evaluate(u, v).unwrap(),
            peak,
            "height={height}, uv=({u},{v})"
        );
    }
}

#[test]
fn closest_point_boundary_candidates_use_the_original_surface_image() {
    let x = 1e16 + 2.;
    let w = 23. / 101.;
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    WeightedPoint3::try_new(
                        p(x + if i == 2 { 12800. } else { 0. }, j as Real * 64., 0.),
                        if j == 0 && i < 2 { w } else { 1. },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let target = p(x + 2., 0., 0.);
    // Column normalization during extraction changes the first Cartesian
    // control by one ulp. This is NOT the surface image at the same UV.
    let edge = surface.isocurve_u(0.).unwrap();
    assert_eq!(edge.evaluate(0.).unwrap(), target);
    assert_eq!(surface.evaluate(0., 0.).unwrap(), p(x, 0., 0.));
    // Independent rational quadratic formula supplies a genuine surface hit.
    let u = (2. * w / (12800. - 2. * (1. - w))).sqrt();
    assert_eq!(surface.evaluate(u, 0.).unwrap(), target);
    let (u, v) = surface
        .closest_parameters(target, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(surface.evaluate(u, v).unwrap(), target, "uv=({u},{v})");
}
