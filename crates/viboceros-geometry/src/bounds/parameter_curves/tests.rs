use super::*;
use crate::{Point2, WeightedPoint2};

fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-10, 1e-13, 1e-10).unwrap()
}
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn paraboloid() -> NurbsSurface {
    NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    point(
                        [-1., 0., 1.][u],
                        [-1., 0., 1.][v],
                        [1., -1., 1.][u] + [1., -1., 1.][v],
                    )
                })
            })
            .collect(),
        vec![-1., -1., -1., 1., 1., 1.],
        vec![-1., -1., -1., 1., 1., 1.],
    )
    .unwrap()
}
fn circle(radius: f64, gauge: f64) -> NurbsCurve2 {
    NurbsCurve2::try_new_rational(
        2,
        [
            [1., 0.],
            [1., 1.],
            [0., 1.],
            [-1., 1.],
            [-1., 0.],
            [-1., -1.],
            [0., -1.],
            [1., -1.],
            [1., 0.],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, [x, y])| {
            WeightedPoint2::try_new(
                Point2::try_new(x * radius, y * radius).unwrap(),
                gauge
                    * if i % 2 == 0 {
                        1.
                    } else {
                        std::f64::consts::FRAC_1_SQRT_2
                    },
            )
            .unwrap()
        })
        .collect(),
        vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.],
    )
    .unwrap()
}
fn near(b: BoundingBox3, min: [f64; 3], max: [f64; 3]) {
    for i in 0..3 {
        assert!(
            (b.min().to_array()[i] - min[i]).abs() < 2e-9,
            "{b:?} min {min:?}"
        );
        assert!(
            (b.max().to_array()[i] - max[i]).abs() < 2e-9,
            "{b:?} max {max:?}"
        );
    }
}

#[test]
fn rational_circles_lift_to_constant_height_on_a_paraboloid_without_fitting_edges() {
    for radius in [0.1, 0.8, 1.] {
        for gauge in [1., -1., 1e-200, 1e200] {
            near(
                paraboloid()
                    .parameter_curve_bounds(&circle(radius, gauge), tolerance())
                    .unwrap(),
                [-radius, -radius, radius * radius],
                [radius, radius, radius * radius],
            );
        }
    }
}

#[test]
fn diagonal_parameter_lines_find_interior_composed_curve_extrema() {
    let c = NurbsCurve2::try_line(
        Point2::try_new(-0.8, 0.8).unwrap(),
        Point2::try_new(0.8, -0.8).unwrap(),
    )
    .unwrap();
    near(
        paraboloid()
            .parameter_curve_bounds(&c, tolerance())
            .unwrap(),
        [-0.8, -0.8, 0.],
        [0.8, 0.8, 1.28],
    );
}

#[test]
fn crossing_non_dyadic_surface_knots_keeps_all_image_pieces() {
    let s = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                (0..4).map(move |u| point([0., 0.31, 0.73, 1.][u], v as f64, [0., 3., -2., 1.][u]))
            })
            .collect(),
        vec![0., 0., 0.31, 0.73, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let c = NurbsCurve2::try_line(
        Point2::try_new(0., 0.1).unwrap(),
        Point2::try_new(1., 0.9).unwrap(),
    )
    .unwrap();
    near(
        s.parameter_curve_bounds(&c, tolerance()).unwrap(),
        [0., 0.1, -2.],
        [1., 0.9, 3.],
    );
}

#[test]
fn invalid_parameter_images_and_true_rational_poles_do_not_return_boxes() {
    let c = NurbsCurve2::try_line(
        Point2::try_new(-2., 0.).unwrap(),
        Point2::try_new(0., 0.).unwrap(),
    )
    .unwrap();
    assert!(
        paraboloid()
            .parameter_curve_bounds(&c, tolerance())
            .is_err()
    );
    let c = NurbsCurve2::try_new_rational(
        2,
        [(-0.5, 1.), (0.5, -1.), (-0.5, 1.)]
            .map(|(u, w)| WeightedPoint2::try_new(Point2::try_new(u, 0.).unwrap(), w).unwrap())
            .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(
        paraboloid()
            .parameter_curve_bounds(&c, tolerance())
            .is_err()
    );
}

#[test]
fn tiny_tangential_uv_overshoots_are_not_silently_clamped_to_a_different_curve() {
    // A negative middle control tilts the endpoint tangent outward. The
    // rational curve leaves U>=-1 by roughly 1e-32 near its endpoint.
    let c = NurbsCurve2::try_new_rational(
        2,
        [[-1., 0.], [-1. - f64::EPSILON, -0.5], [0., -0.5]]
            .into_iter()
            .enumerate()
            .map(|(i, [u, v])| {
                WeightedPoint2::try_new(
                    Point2::try_new(u, v).unwrap(),
                    if i == 1 {
                        std::f64::consts::FRAC_1_SQRT_2
                    } else {
                        1.
                    },
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        paraboloid().parameter_curve_bounds(&c, tolerance()),
        Err(GeometryError::ParameterCurveOutsideSurfaceDomain)
    );
}

#[test]
fn continuous_parameter_images_survive_adjacent_representable_uv_domains() {
    let s = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.),
            point(1., 0., 1.),
            point(0., 1., -1.),
            point(1., 1., 0.),
        ],
        vec![1e15, 1e15, 1e15 + 0.125, 1e15 + 0.125],
        vec![1e15, 1e15, 1e15 + 0.25, 1e15 + 0.25],
    )
    .unwrap();
    let c = NurbsCurve2::try_line(
        Point2::try_new(1e15, 1e15).unwrap(),
        Point2::try_new(1e15 + 0.125, 1e15 + 0.25).unwrap(),
    )
    .unwrap();
    near(
        s.parameter_curve_bounds(&c, tolerance()).unwrap(),
        [0.; 3],
        [1., 1., 0.],
    );
}

#[test]
fn huge_off_domain_uv_controls_can_still_have_a_finite_in_domain_curve_image() {
    let s = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(0., 1., 0.),
            point(1., 1., 0.),
        ],
        vec![-1e308, -1e308, 0., 0.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let c = NurbsCurve2::try_new(
        2,
        vec![
            Point2::try_new(-1e308, 0.5).unwrap(),
            Point2::try_new(1e308, 0.5).unwrap(),
            Point2::try_new(-1e308, 0.5).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    near(
        s.parameter_curve_bounds(&c, tolerance()).unwrap(),
        [0., 0.5, 0.],
        [1., 0.5, 0.],
    );
}

#[test]
fn an_image_on_a_full_order_surface_knot_uses_the_native_side() {
    let s = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                (0..4).map(move |u| point([0., 0.5, 0.5, 1.][u], v as f64, [0., 0., 10., 10.][u]))
            })
            .collect(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let c = NurbsCurve2::try_line(
        Point2::try_new(0.5, 0.).unwrap(),
        Point2::try_new(0.5, 1.).unwrap(),
    )
    .unwrap();
    near(
        s.parameter_curve_bounds(&c, tolerance()).unwrap(),
        [0.5, 0., 10.],
        [0.5, 1., 10.],
    );
    let c = NurbsCurve2::try_line(
        Point2::try_new(0., 0.).unwrap(),
        Point2::try_new(1., 1.).unwrap(),
    )
    .unwrap();
    near(
        s.parameter_curve_bounds(&c, tolerance()).unwrap(),
        [0.; 3],
        [1., 1., 10.],
    );
    for ulps in 1..=16 {
        let u = f64::from_bits(0.5_f64.to_bits() - ulps);
        for weight in [std::f64::consts::FRAC_1_SQRT_2, 0.1234567, 1e-100, 1e100] {
            let c = NurbsCurve2::try_new_rational(
                1,
                vec![
                    WeightedPoint2::try_new(Point2::try_new(u, 0.).unwrap(), 1.).unwrap(),
                    WeightedPoint2::try_new(Point2::try_new(u, 1.).unwrap(), weight).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            near(
                s.parameter_curve_bounds(&c, tolerance()).unwrap(),
                [u, 0., 0.],
                [u, 1., 0.],
            );
        }
    }
}

#[test]
fn surface_poles_outside_the_parameter_curve_do_not_invalidate_its_regular_image() {
    let s = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    WeightedPoint3::try_new(point(u as f64, v as f64, 0.), [1., -1., 1.][u])
                        .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(s.tight_bounds(tolerance()).is_err());
    let c = NurbsCurve2::try_line(
        Point2::try_new(0.1, 0.).unwrap(),
        Point2::try_new(0.1, 1.).unwrap(),
    )
    .unwrap();
    near(
        s.parameter_curve_bounds(&c, tolerance()).unwrap(),
        [-0.25, 0., 0.],
        [-0.25, 1., 0.],
    );
}

#[test]
fn roundoff_near_a_discontinuous_surface_knot_cannot_invent_a_faraway_image_branch() {
    let s = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                (0..4).map(move |u| point([0., 0.5, 0.5, 1.][u], v as f64, [0., 0., 10., 10.][u]))
            })
            .collect(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for b in [0.5625, 0.625, 0.75, 1.] {
        for critical in [0.25_f64, 0.5, 0.75, 1., 1.25, 1.5, 2., 3., 4.] {
            let a = 0.5 - critical * (b - 0.5);
            if a < 0. {
                continue;
            }
            for weight in [
                f64::from_bits(critical.to_bits() - 1),
                f64::from_bits(critical.to_bits() + 1),
            ] {
                // u(1/2)=(a+b*w)/(1+w). The dyadic controls make the
                // critical weight exact: crossing occurs iff w>=critical.
                let c = NurbsCurve2::try_new_rational(
                    2,
                    [[a, 0.1], [b, 0.5], [a, 0.9]]
                        .into_iter()
                        .enumerate()
                        .map(|(i, [u, v])| {
                            WeightedPoint2::try_new(
                                Point2::try_new(u, v).unwrap(),
                                if i == 1 { weight } else { 1. },
                            )
                            .unwrap()
                        })
                        .collect(),
                    vec![0., 0., 0., 1., 1., 1.],
                )
                .unwrap();
                if let Ok(bounds) = s.parameter_curve_bounds(&c, tolerance()) {
                    assert!(
                        (bounds.max().z() - if weight < critical { 0. } else { 10. }).abs() < 1e-9,
                        "a {a}, b {b}, critical {critical}, weight {weight}, {bounds:?}"
                    );
                }
            }
        }
    }
}
