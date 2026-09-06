use super::*;
use crate::{Point3, Vector3, WeightedPoint3};

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn accuracy() -> Tolerance {
    Tolerance::try_new(1e-10, 1e-14, 1e-10).unwrap()
}
fn near(a: Point3, b: [f64; 3]) {
    assert!(a.distance_to(p(b)).unwrap() < 2e-9, "{a:?} != {b:?}");
}
fn quadratic() -> NurbsSurface {
    NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    p([
                        u as f64 / 2.,
                        v as f64 / 2.,
                        [0., 2., 0.][u] + [0., 4., 0.][v],
                    ])
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn polynomial_tensor_bounds_find_interior_extrema_instead_of_control_net_extents() {
    let surface = quadratic();
    let bounds = surface.tight_bounds(accuracy()).unwrap();
    near(bounds.min(), [0.; 3]);
    near(bounds.max(), [1., 1., 3.]);
    assert_eq!(surface.control_point_bounds().max().z(), 6.);
}

#[test]
fn extruded_rational_ridges_retain_projective_controls_and_negative_common_gauges() {
    for gauge in [1., -1., 1e-200, 1e200] {
        let controls = (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    WeightedPoint3::try_new(
                        p([u as f64, v as f64 * 3., [0., 10., 0.][u]]),
                        [1., -1., 2.][u] * gauge,
                    )
                    .unwrap()
                })
            })
            .collect();
        let s = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let b = s.tight_bounds(accuracy()).unwrap();
        assert!((b.min().z() + 10. * (1. + 2f64.sqrt())).abs() < 1e-8);
        assert_eq!(b.min().y(), 0.);
        assert!((b.max().y() - 3.).abs() < 1e-12);
        for u in 0..=1000 {
            for v in [0., 0.19, 0.83, 1.] {
                let actual = s.evaluate(u as f64 / 1000., v).unwrap().to_array();
                for (i, a) in actual.into_iter().enumerate() {
                    assert!(
                        a >= b.min().to_array()[i] - 1e-10 && a <= b.max().to_array()[i] + 1e-10
                    );
                }
            }
        }
    }
}

#[test]
fn homogeneous_bounds_reject_true_surface_poles() {
    let controls = (0..2)
        .flat_map(|v| {
            (0..3).map(move |u| {
                WeightedPoint3::try_new(p([u as f64, v as f64, 0.]), [1., -1., 1.][u]).unwrap()
            })
        })
        .collect();
    let s = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(s.tight_bounds(accuracy()).is_err());
}

#[test]
fn unclamped_tensor_bounds_cover_active_spans_and_full_order_independent_limits() {
    let curve_controls = [
        [100., 10., 0.],
        [0., 1., 0.],
        [0., -1., 0.],
        [100., -10., 0.],
    ];
    let controls = (0..2)
        .flat_map(|v| curve_controls.map(move |[x, y, _]| p([x, y, v as f64 * 3.])))
        .collect();
    let s = NurbsSurface::try_new(
        2,
        1,
        4,
        2,
        controls,
        (0..7).map(f64::from).collect(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let b = s.tight_bounds(accuracy()).unwrap();
    near(b.min(), [0., -5.5, 0.]);
    near(b.max(), [50., 5.5, 3.]);
    let controls = (0..4)
        .flat_map(|v| {
            (0..4).map(move |u| p([[0., 1., 10., 11.][u], [0., 2., 20., 22.][v], (u + v) as f64]))
        })
        .collect();
    let s = NurbsSurface::try_new(
        1,
        1,
        4,
        4,
        controls,
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    let b = s.tight_bounds(accuracy()).unwrap();
    near(b.min(), [0.; 3]);
    near(b.max(), [11., 22., 6.]);
}

#[test]
fn large_translation_does_not_change_homogeneous_tensor_extrema() {
    let s = quadratic();
    let b = s.tight_bounds(accuracy()).unwrap();
    let offset = Vector3::try_new(1e12, -2e12, 3e12).unwrap();
    let shifted = s
        .transformed(crate::AffineTransform3::from_translation(offset))
        .unwrap()
        .tight_bounds(accuracy())
        .unwrap();
    assert_eq!(shifted.min(), b.min().translated(offset).unwrap());
    assert_eq!(shifted.max(), b.max().translated(offset).unwrap());
}

#[test]
fn closed_sphere_and_torus_cover_all_tensor_spans_and_poles_on_tilted_frames() {
    let frame = crate::Frame3::try_from_directions(
        p([3., 5., 7.]),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        accuracy(),
    )
    .unwrap();
    let sphere = NurbsSurface::try_sphere(frame, 4.).unwrap();
    let b = sphere.tight_bounds(accuracy()).unwrap();
    near(b.min(), [-1., 1., 3.]);
    near(b.max(), [7., 9., 11.]);
    let torus = NurbsSurface::try_torus(frame, 5., 2.).unwrap();
    let b = torus.tight_bounds(accuracy()).unwrap();
    let normal = frame.z_axis().as_vector().to_array();
    let extents = normal.map(|n| 5. * (1. - n * n).sqrt() + 2.);
    near(
        b.min(),
        std::array::from_fn(|i| frame.origin().to_array()[i] - extents[i]),
    );
    near(
        b.max(),
        std::array::from_fn(|i| frame.origin().to_array()[i] + extents[i]),
    );
}

#[test]
fn oblique_quadratic_boxes_match_closed_form_extrema_on_every_axis() {
    let frame = crate::Frame3::try_from_directions(
        p([7., -9., 11.]),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        accuracy(),
    )
    .unwrap();
    let world = crate::Frame3::try_from_normal(
        p([0.; 3]),
        Vector3::try_new(0., 0., 1.).unwrap(),
        accuracy(),
    )
    .unwrap();
    let surface = quadratic()
        .transformed(crate::AffineTransform3::try_frame_mapping(frame, world, [1.; 3]).unwrap())
        .unwrap();
    let b = surface.tight_bounds(accuracy()).unwrap();
    for (i, axis) in [frame.x_axis(), frame.y_axis(), frame.z_axis()]
        .into_iter()
        .enumerate()
    {
        let [a, basis_b, c] = axis.as_vector().to_array();
        let u = if c == 0. {
            0.
        } else {
            ((a + 4. * c) / (8. * c)).clamp(0., 1.)
        };
        let v = if c == 0. {
            0.
        } else {
            ((basis_b + 8. * c) / (16. * c)).clamp(0., 1.)
        };
        let shift = axis
            .as_vector()
            .dot(Vector3::try_from(frame.origin().to_array()).unwrap())
            .unwrap();
        let values = [0., u, 1.]
            .into_iter()
            .flat_map(|u| {
                [0., v, 1.].map(move |v| {
                    a * u + basis_b * v + c * (4. * u * (1. - u) + 8. * v * (1. - v)) - shift
                })
            })
            .collect::<Vec<_>>();
        assert!(
            (b.min().to_array()[i] - values.iter().copied().fold(f64::INFINITY, f64::min)).abs()
                < 2e-9
        );
        assert!(
            (b.max().to_array()[i] - values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
                .abs()
                < 2e-9
        );
    }
}
