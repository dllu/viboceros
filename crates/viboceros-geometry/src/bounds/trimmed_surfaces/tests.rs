use super::*;
use crate::{
    BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, Frame3, NurbsCurve2, NurbsSurface, Point2,
    SurfaceIso, Vector3, WeightedPoint2,
};

pub(in crate::bounds) fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-10, 1e-13, 1e-10).unwrap()
}
pub(in crate::bounds) fn paraboloid() -> NurbsSurface {
    NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    Point3::try_new(
                        [-1., 0., 1.][u],
                        [-1., 0., 1.][v],
                        [1., -1., 1.][u] + [1., -1., 1.][v],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![-1., -1., -1., 1., 1., 1.],
        vec![-1., -1., -1., 1., 1., 1.],
    )
    .unwrap()
}
pub(in crate::bounds) fn circle(radius: f64, gauge: f64) -> NurbsCurve2 {
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
        .map(|(i, [u, v])| {
            WeightedPoint2::try_new(
                Point2::try_new(u * radius, v * radius).unwrap(),
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
pub(in crate::bounds) fn face(surface: NurbsSurface, curves: Vec<Vec<NurbsCurve2>>) -> BrepFace {
    BrepFace::try_new(
        surface,
        false,
        curves
            .into_iter()
            .enumerate()
            .map(|(i, curves)| {
                BrepLoop::try_new(
                    if i == 0 {
                        BrepLoopType::Outer
                    } else {
                        BrepLoopType::Inner
                    },
                    curves
                        .into_iter()
                        .map(|c| {
                            BrepTrim::try_new(
                                [0, 0],
                                Some(0),
                                false,
                                c,
                                BrepTrimType::Boundary,
                                SurfaceIso::NotIso,
                                [0.; 2],
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
pub(in crate::bounds) fn polygon(points: &[[f64; 2]]) -> Vec<NurbsCurve2> {
    (0..points.len())
        .map(|i| {
            let [a, b] = [points[i], points[(i + 1) % points.len()]];
            NurbsCurve2::try_line(
                Point2::try_new(a[0], a[1]).unwrap(),
                Point2::try_new(b[0], b[1]).unwrap(),
            )
            .unwrap()
        })
        .collect()
}
fn near(bounds: BoundingBox3, low: [f64; 3], high: [f64; 3]) {
    for (p, expected) in [
        (bounds.min().to_array(), low),
        (bounds.max().to_array(), high),
    ] {
        for i in 0..3 {
            assert!(
                (p[i] - expected[i]).abs() <= 1.1e-10,
                "{bounds:?}; expected {low:?}..{high:?}"
            );
        }
    }
}

#[test]
fn round_trims_keep_interior_extrema_and_remove_hole_interiors() {
    for radii in [
        &[0.8][..],
        &[0.8, 0.35][..],
        &[0.8, 0.799][..],
        &[0.8, 1e-5][..],
    ] {
        for gauge in [1., -1., 1e-200, 1e200] {
            let f = face(
                paraboloid(),
                radii.iter().map(|r| vec![circle(*r, gauge)]).collect(),
            );
            let minimum = radii.get(1).map_or(0., |r| r * r);
            near(
                f.tight_bounds(tolerance()).unwrap(),
                [-0.8, -0.8, minimum],
                [0.8, 0.8, 0.64],
            );
        }
    }
}

#[test]
fn concave_outer_trim_excludes_the_untrimmed_stationary_point() {
    let f = face(
        paraboloid(),
        vec![polygon(&[
            [-0.8, -0.8],
            [0.8, -0.8],
            [0.8, -0.3],
            [-0.3, -0.3],
            [-0.3, 0.8],
            [-0.8, 0.8],
        ])],
    );
    near(
        f.tight_bounds(tolerance()).unwrap(),
        [-0.8, -0.8, 0.09],
        [0.8, 0.8, 1.28],
    );
}

#[test]
fn natural_sphere_includes_interior_extrema_missing_from_its_seam_box() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance(),
    )
    .unwrap();
    let b =
        Brep::try_surface_face(NurbsSurface::try_sphere(frame, 2.).unwrap(), tolerance()).unwrap();
    near(b.tight_bounds(tolerance()).unwrap(), [-2.; 3], [2.; 3]);
}

#[test]
fn c0_knot_ridge_extremum_is_not_discarded_by_one_sided_derivatives() {
    let s = NurbsSurface::try_new(
        1,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    Point3::try_new(
                        [0., 0.37, 1.][u],
                        [-1., 0., 1.][v],
                        [0., 2., 0.][u] - [1., -1., 1.][v],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0.37, 1., 1.],
        vec![-1., -1., -1., 1., 1., 1.],
    )
    .unwrap();
    let f = face(
        s,
        vec![polygon(&[[0.1, -0.7], [0.8, -0.7], [0.8, 0.7], [0.1, 0.7]])],
    );
    near(
        f.tight_bounds(tolerance()).unwrap(),
        [0.1, -0.7, 0.2 / 0.37 - 0.49],
        [0.8, 0.7, 2.],
    );
}

#[test]
fn actual_uv_gaps_are_errors_even_when_model_tolerance_would_close_them() {
    let mut curves = polygon(&[[-0.8, -0.8], [0.8, -0.8], [0.8, 0.8], [-0.8, 0.8]]);
    curves[0] = NurbsCurve2::try_line(
        Point2::try_new(-0.8 + 1e-12, -0.8).unwrap(),
        Point2::try_new(0.8, -0.8).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        face(paraboloid(), vec![curves]).tight_bounds(Tolerance::DEFAULT),
        Err(GeometryError::InvalidBrepTopology { .. })
    ));
}

#[test]
fn oblique_disk_and_annulus_extrema_match_closed_form_quadratics_on_every_axis() {
    let rows = [[1., 0., 3.], [-2., 1., 1.], [0.25, 0.5, -2.]];
    let offset = [2., -3., 4.];
    let transform =
        crate::AffineTransform3::try_new(rows, Vector3::try_from(offset).unwrap()).unwrap();
    for inner in [0., 0.1, 0.35, 0.799] {
        let curves = if inner == 0. {
            vec![vec![circle(0.8, 1.)]]
        } else {
            vec![vec![circle(0.8, 1.)], vec![circle(inner, 1.)]]
        };
        let f = face(paraboloid().transformed(transform).unwrap(), curves);
        let b = f.tight_bounds(tolerance()).unwrap();
        for (axis, [a, b_coefficient, c]) in rows.into_iter().enumerate() {
            let radial = a.hypot(b_coefficient);
            let candidates = [
                inner,
                0.8,
                (radial / (2. * c)).clamp(inner, 0.8),
                (-radial / (2. * c)).clamp(inner, 0.8),
            ];
            let values = candidates
                .into_iter()
                .flat_map(|r| {
                    [-1., 1.].map(move |sign| offset[axis] + c * r * r + sign * radial * r)
                })
                .collect::<Vec<_>>();
            assert!(
                (b.min().to_array()[axis] - values.iter().copied().fold(f64::INFINITY, f64::min))
                    .abs()
                    < 1.1e-10,
                "{inner}: {b:?}"
            );
            assert!(
                (b.max().to_array()[axis]
                    - values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
                .abs()
                    < 1.1e-10,
                "{inner}: {b:?}"
            );
        }
    }
}

#[test]
fn a_surface_pole_outside_the_trim_is_irrelevant_but_a_retained_pole_is_an_error() {
    let s = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    crate::WeightedPoint3::try_new(
                        Point3::try_new(u as f64, v as f64, 0.).unwrap(),
                        [1., -1., 1.][u],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let f = face(
        s.clone(),
        vec![polygon(&[[0., 0.2], [0.25, 0.2], [0.25, 0.8], [0., 0.8]])],
    );
    near(
        f.tight_bounds(tolerance()).unwrap(),
        [-1., 0.2, 0.],
        [0., 0.8, 0.],
    );
    let f = face(
        s,
        vec![polygon(&[
            [0.25, 0.2],
            [0.75, 0.2],
            [0.75, 0.8],
            [0.25, 0.8],
        ])],
    );
    assert!(f.tight_bounds(tolerance()).is_err());
}

#[test]
fn full_order_surface_knots_keep_only_the_branches_in_the_trimmed_region() {
    let s = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        (0..2)
            .flat_map(|v| {
                (0..4).map(move |u| {
                    Point3::try_new(
                        [0., 0.5, 0.5, 1.][u],
                        v as f64,
                        if u < 2 { 0. } else { 10. },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for (low, high, min_z, max_z) in [
        (0.1, 0.4, 0., 0.),
        (0.6, 0.9, 10., 10.),
        (0.1, 0.9, 0., 10.),
    ] {
        let f = face(
            s.clone(),
            vec![polygon(&[[low, 0.2], [high, 0.2], [high, 0.8], [low, 0.8]])],
        );
        near(
            f.tight_bounds(tolerance()).unwrap(),
            [low, 0.2, min_z],
            [high, 0.8, max_z],
        );
    }
    // A varying trim starting exactly on a jump still leaves an unresolved
    // branch in the guarded boundary-image query. Retain the limitation as an
    // error, not an invented left-hand face at Z=0 or a loose two-branch box.
    let touching = face(
        s,
        vec![polygon(&[[0.5, 0.2], [0.9, 0.2], [0.9, 0.8], [0.5, 0.8]])],
    );
    assert_eq!(
        touching.tight_bounds(tolerance()),
        Err(GeometryError::BoundingBoxDidNotConverge)
    );
}

#[test]
fn natural_trimmed_faces_retain_signed_rational_geometry_and_extreme_gauges() {
    for gauge in [1., -1., 1e-200, 1e200] {
        let s = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            (0..2)
                .flat_map(|v| {
                    (0..3).map(move |u| {
                        crate::WeightedPoint3::try_new(
                            Point3::try_new(u as f64, v as f64, [0., 1., 0.][u]).unwrap(),
                            [1., -1., 2.][u] * gauge,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let f = face(s, vec![polygon(&[[0., 0.], [1., 0.], [1., 1.], [0., 1.]])]);
        near(
            f.tight_bounds(tolerance()).unwrap(),
            [1. - 2_f64.sqrt(), 0., -1. - 2_f64.sqrt()],
            [1. + 2_f64.sqrt(), 1., 0.],
        );
    }
}

#[test]
fn neighboring_representable_native_uv_domains_preserve_continuous_face_extrema() {
    let s = paraboloid()
        .try_reparameterized(1e15..=1e15 + 0.125, 1e15..=1e15 + 0.25)
        .unwrap();
    let f = face(
        s,
        vec![polygon(&[
            [1e15, 1e15],
            [1e15 + 0.125, 1e15],
            [1e15 + 0.125, 1e15 + 0.25],
            [1e15, 1e15 + 0.25],
        ])],
    );
    near(
        f.tight_bounds(tolerance()).unwrap(),
        [-1., -1., 0.],
        [1., 1., 2.],
    );
}

#[test]
fn a_hole_can_remove_an_isolated_rational_surface_pole() {
    // W=(u-3/8)^2+(v-3/8)^2, S=(1,u,v)/W (to control rounding).
    // Dyadic denominator coefficients make the isolated zero exact. It is
    // inside the circular hole; the retained annulus is entirely regular.
    let squared = [9. / 64., -15. / 64., 25. / 64.];
    let s = NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    let w = squared[u] + squared[v];
                    crate::WeightedPoint3::try_new(
                        Point3::try_new(1. / w, (u as f64 / 2.) / w, (v as f64 / 2.) / w).unwrap(),
                        w,
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let shifted = |radius| {
        let c = circle(radius, 1.);
        NurbsCurve2::try_new_rational(
            c.degree(),
            c.control_points()
                .iter()
                .map(|p| {
                    WeightedPoint2::try_new(
                        Point2::try_new(p.point().x() + 0.375, p.point().y() + 0.375).unwrap(),
                        p.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap()
    };
    let f = face(s, vec![vec![shifted(0.25)], vec![shifted(0.125)]]);
    near(
        f.tight_bounds(tolerance()).unwrap(),
        [16., 2., 2.],
        [64., 32., 32.],
    );
}
