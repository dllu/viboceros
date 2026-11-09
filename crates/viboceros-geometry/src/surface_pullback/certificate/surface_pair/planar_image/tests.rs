use super::*;
use crate::{Point3, WeightedPoint3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn knots(p: usize) -> Vec<Real> {
    [vec![0.; p + 1], vec![1.; p + 1]].concat()
}
fn sheet(x: &[Real], y: &[Real], rational_weights: bool) -> NurbsSurface {
    let nu = x.len();
    let nv = y.len();
    NurbsSurface::try_new_rational(
        nu - 1,
        nv - 1,
        nu,
        nv,
        y.iter()
            .enumerate()
            .flat_map(|(j, &y)| {
                x.iter().enumerate().map(move |(i, &x)| {
                    WeightedPoint3::try_new(
                        point(x, y, 0.),
                        if rational_weights {
                            1. + ((i + 2 * j) % 3) as Real / 2.
                        } else {
                            1.
                        },
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots(nu - 1),
        knots(nv - 1),
    )
    .unwrap()
}
fn rectangle() -> NurbsSurface {
    sheet(&[0., 1.], &[0., 1.], false)
}

#[test]
fn equal_images_ignore_nonlinear_rational_parameterization_and_knot_density() {
    let target = rectangle();
    let source = sheet(&[0., 0.1, 0.8, 1.], &[0., 0.2, 0.9, 1.], true)
        .try_insert_knot_u(0.25, 2)
        .unwrap()
        .try_insert_knot_v(0.5, 3)
        .unwrap()
        .try_reparameterized(2. ..=5., -7. ..=1.)
        .unwrap();
    let before = source.clone();
    assert!(
        source
            .parameter_surface_deviation_bound(&target, 1e-6)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        source
            .planar_surface_image_deviation_bound(&target, 0.)
            .unwrap(),
        Some(0.)
    );
    assert_eq!(
        target
            .planar_surface_image_deviation_bound(&source, 0.)
            .unwrap(),
        Some(0.)
    );
    assert_eq!(source, before);
}

#[test]
fn exact_half_spaces_allow_backtracking_with_controls_outside_the_image() {
    // This cubic backtracks around x=1/2 but remains in [0,1]. The first
    // subdivision has controls [0,.55,.525,.5] and [.5,.475,.45,1].
    let folded = sheet(&[0., 1.1, -0.1, 1.], &[0., 1.], false);
    assert_eq!(
        folded
            .planar_surface_image_deviation_bound(&rectangle(), 0.)
            .unwrap(),
        Some(0.)
    );
    // A similar control net actually exceeds x=1 in its interior.
    let outside = sheet(&[0., 2., 2., 1.], &[0., 1.], false);
    assert!(outside.evaluate(0.5, 0.5).unwrap().x() > 1.);
    assert_eq!(
        outside
            .planar_surface_image_deviation_bound(&rectangle(), 0.)
            .unwrap(),
        None
    );
}

#[test]
fn cyclic_reversed_and_swapped_parameter_frames_have_the_same_image() {
    let source = sheet(&[0., 0.25, 1.], &[0., 0.75, 1.], true);
    for changed in [
        source.try_reversed_u().unwrap(),
        source.try_reversed_v().unwrap(),
        source.try_swapped_uv().unwrap(),
    ] {
        assert_eq!(
            source
                .planar_surface_image_deviation_bound(&changed, 0.)
                .unwrap(),
            Some(0.)
        );
    }
}

#[test]
fn different_convex_quads_and_planes_have_complete_vertex_displacement_bounds() {
    let source = rectangle();
    for target in [
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                point(0., 0., 0.125),
                point(1., 0., 0.125),
                point(0., 1., 0.125),
                point(1., 1., 0.125),
            ],
            knots(1),
            knots(1),
        )
        .unwrap(),
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(0., 1., 0.),
                point(1.125, 1., 0.),
            ],
            knots(1),
            knots(1),
        )
        .unwrap(),
    ] {
        assert_eq!(
            source
                .planar_surface_image_deviation_bound(&target, 0.125)
                .unwrap(),
            Some(0.125)
        );
        assert_eq!(
            source
                .planar_surface_image_deviation_bound(&target, 0.125_f64.next_down())
                .unwrap(),
            None
        );
    }
    let tilted = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.125),
            point(1., 0., 0.125),
            point(0., 1., 0.25),
            point(1., 1., 0.25),
        ],
        knots(1),
        knots(1),
    )
    .unwrap();
    assert_eq!(
        source
            .planar_surface_image_deviation_bound(&tilted, 0.25)
            .unwrap(),
        Some(0.25)
    );
}

#[test]
fn identical_boundaries_do_not_hide_a_nonplanar_interior_bump() {
    let flat = sheet(&[0., 0.5, 1.], &[0., 0.5, 1.], false);
    let mut controls = flat.control_points().to_vec();
    controls[4] = WeightedPoint3::try_new(point(0.5, 0.5, 1.), 1.).unwrap();
    let bump = NurbsSurface::try_new_rational(2, 2, 3, 3, controls, knots(2), knots(2)).unwrap();
    assert!(bump.evaluate(0.5, 0.5).unwrap().z() > 0.2);
    assert_eq!(
        flat.planar_surface_image_deviation_bound(&bump, 0.125)
            .unwrap(),
        None
    );
    assert_eq!(
        flat.planar_surface_image_deviation_bound(&bump, 1.)
            .unwrap(),
        Some(1.)
    );
}

#[test]
fn discontinuities_cannot_fake_boundary_coverage() {
    // The complete image omits 0.2<x<0.8 despite all controls lying inside
    // the rectangle and on the correct four boundary supporting lines.
    let source = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| [0., 0.2, 0.8, 1.].into_iter().map(move |x| point(x, y, 0.)))
            .collect(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
        knots(1),
    )
    .unwrap();
    assert_eq!(
        source
            .planar_surface_image_deviation_bound(&rectangle(), 0.)
            .unwrap(),
        None
    );
}

#[test]
fn concave_and_degenerate_quads_decline_while_bowed_edges_need_a_bound() {
    let target = rectangle();
    let concave = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(0., 1., 0.),
            point(0.25, 0.25, 0.),
        ],
        knots(1),
        knots(1),
    )
    .unwrap();
    let collapsed =
        NurbsSurface::try_new(1, 1, 2, 2, vec![point(0., 0., 0.); 4], knots(1), knots(1)).unwrap();
    let flat = sheet(&[0., 0.5, 1.], &[0., 0.5, 1.], false);
    let mut controls = flat.control_points().to_vec();
    controls[1] = WeightedPoint3::try_new(point(0.5, -0.25, 0.), 1.).unwrap();
    let bowed = NurbsSurface::try_new_rational(2, 2, 3, 3, controls, knots(2), knots(2)).unwrap();
    for source in [concave, collapsed] {
        assert_eq!(
            source
                .planar_surface_image_deviation_bound(&target, 2.)
                .unwrap(),
            None
        );
    }
    assert_eq!(
        bowed
            .planar_surface_image_deviation_bound(&target, 0.1)
            .unwrap(),
        None
    );
    assert_eq!(
        bowed
            .planar_surface_image_deviation_bound(&target, 0.25)
            .unwrap(),
        Some(0.25)
    );
}

#[test]
fn signed_gauges_validation_and_resource_limits_remain_explicit() {
    let source = rectangle();
    let weighted = |weights: [Real; 4]| {
        NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            source
                .control_points()
                .iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(p.point(), w).unwrap())
                .collect(),
            knots(1),
            knots(1),
        )
        .unwrap()
    };
    assert_eq!(
        source
            .planar_surface_image_deviation_bound(&weighted([-1., -2., -3., -4.]), 0.)
            .unwrap(),
        Some(0.)
    );
    assert_eq!(
        source
            .planar_surface_image_deviation_bound(&weighted([1., -1., 1., -1.]), 0.)
            .unwrap(),
        None
    );
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            source.planar_surface_image_deviation_bound(&source, limit),
            Err(GeometryError::InvalidTolerance)
        ));
    }
    let mut budget = Budget(1);
    assert!(matches!(
        bound(&source, &source, 0., &mut budget),
        Err(GeometryError::SurfaceCurveCertificateWorkLimit)
    ));
}

#[test]
fn active_endpoints_and_control_limits_cannot_be_replaced_by_stored_corners() {
    let unclamped = NurbsSurface::try_new(
        2,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                [0., 0.25, 0.75, 1.]
                    .into_iter()
                    .map(move |x| point(x, y, 0.))
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3., 4.],
        knots(1),
    )
    .unwrap();
    assert!(unclamped.evaluate(0., 0.).unwrap().x() > 0.1);
    assert_eq!(
        unclamped
            .planar_surface_image_deviation_bound(&rectangle(), 0.)
            .unwrap(),
        None
    );
    let axis = |n: usize| {
        [
            vec![0.],
            (0..n).map(|i| i as Real / (n - 1) as Real).collect(),
            vec![1.],
        ]
        .concat()
    };
    let large = NurbsSurface::try_new(
        1,
        1,
        65,
        64,
        (0..64)
            .flat_map(|j| (0..65).map(move |i| point(i as Real / 64., j as Real / 63., 0.)))
            .collect(),
        axis(65),
        axis(64),
    )
    .unwrap();
    assert_eq!(
        large
            .planar_surface_image_deviation_bound(&rectangle(), 0.)
            .unwrap(),
        None
    );
}
