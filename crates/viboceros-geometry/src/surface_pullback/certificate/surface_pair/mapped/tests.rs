use super::*;
use crate::WeightedPoint3;

const IDENTITY: [[Real; 2]; 2] = [[0., 0.], [1., 1.]];
const U: [[Real; 2]; 3] = [[0., 0.], [0.25, 0.75], [1., 1.]];
const V: [[Real; 2]; 3] = [[0., 0.], [0.5, 0.125], [1., 1.]];

fn sheet(other: bool, dz: Real) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        3,
        3,
        (0..9)
            .map(|k| {
                let (i, j) = (k % 3, k / 3);
                WeightedPoint3::try_new(
                    Point3::try_new(i as Real, j as Real, (i * j) as Real / 4. + dz).unwrap(),
                    [1., 2., 4., 2., 1., 3., 4., 3., 2.][k] * if other { -8. } else { 2. },
                )
                .unwrap()
            })
            .collect(),
        if other {
            vec![-6., -6., 0., 2., 2.]
        } else {
            vec![2., 2., 4., 10., 10.]
        },
        if other {
            vec![8., 8., 9., 16., 16.]
        } else {
            vec![-2., -2., 2., 6., 6.]
        },
    )
    .unwrap()
}

#[test]
fn curved_rational_images_with_two_changed_clocks_have_an_exact_zero_bound() {
    let a = sheet(false, 0.);
    let b = sheet(true, 0.);
    let before = [a.clone(), b.clone()];
    assert_eq!(a.parameter_surface_deviation_bound(&b, 0.1).unwrap(), None);
    assert_eq!(
        a.planar_surface_image_deviation_bound(&b, 0.).unwrap(),
        None
    );
    assert_eq!(
        a.mapped_surface_deviation_bound(&b, [&U, &V], 0.).unwrap(),
        Some(0.)
    );
    let inverse = |pairs: &[[Real; 2]]| pairs.iter().map(|p| [p[1], p[0]]).collect::<Vec<_>>();
    assert_eq!(
        b.mapped_surface_deviation_bound(&a, [&inverse(&U), &inverse(&V)], 0.)
            .unwrap(),
        Some(0.)
    );
    assert_eq!(
        a.try_swapped_uv()
            .unwrap()
            .mapped_surface_deviation_bound(&b.try_swapped_uv().unwrap(), [&V, &U], 0.)
            .unwrap(),
        Some(0.)
    );
    assert_eq!(
        a.mapped_surface_deviation_bound(&a, [&U, &V], 0.).unwrap(),
        None
    );
    assert_eq!([a, b], before);
}

#[test]
fn displaced_rational_images_preserve_the_exact_threshold_and_subnormal_errors() {
    let a = sheet(false, 0.);
    for displacement in [0.125, Real::from_bits(1)] {
        let b = sheet(true, displacement);
        assert_eq!(
            a.mapped_surface_deviation_bound(&b, [&U, &V], displacement)
                .unwrap(),
            Some(displacement)
        );
        assert_eq!(
            a.mapped_surface_deviation_bound(&b, [&U, &V], displacement.next_down())
                .unwrap(),
            None
        );
    }
}

#[test]
fn map_intervals_are_partitioned_at_knots_of_both_original_tensors() {
    let a = sheet(false, 0.);
    let b = sheet(true, 0.)
        .try_insert_knot_u(-3., 1)
        .unwrap()
        .try_insert_knot_v(12., 1)
        .unwrap();
    // These inserted knots are not supplied as map stations. Rounded knot
    // insertion is compared against the original stored tensors too.
    let bound = a
        .mapped_surface_deviation_bound(&b, [&U, &V], 1e-12)
        .unwrap()
        .unwrap();
    assert!(bound <= 1e-12);
    assert_eq!(
        a.mapped_surface_deviation_bound(&b, [&IDENTITY; 2], 0.1)
            .unwrap(),
        None
    );
}

#[test]
fn both_limits_of_discontinuous_mapped_knots_are_checked() {
    let make = |other: bool| {
        NurbsSurface::try_new(
            1,
            1,
            4,
            4,
            (0..16)
                .map(|k| {
                    Point3::try_new(
                        (k % 4) as Real,
                        (k / 4) as Real,
                        if other && k == 5 { 0.25 } else { 0. },
                    )
                    .unwrap()
                })
                .collect(),
            if other {
                vec![0., 0., 0.75, 0.75, 1., 1.]
            } else {
                vec![0., 0., 0.25, 0.25, 1., 1.]
            },
            if other {
                vec![0., 0., 0.125, 0.125, 1., 1.]
            } else {
                vec![0., 0., 0.5, 0.5, 1., 1.]
            },
        )
        .unwrap()
    };
    let (a, b) = (make(false), make(true));
    assert_eq!(
        a.mapped_surface_deviation_bound(&b, [&U, &V], 0.1).unwrap(),
        None
    );
    assert_eq!(
        a.mapped_surface_deviation_bound(&b, [&U, &V], 0.25)
            .unwrap(),
        Some(0.25)
    );
}

#[test]
fn equal_map_stations_cannot_hide_an_interior_polynomial_excursion() {
    let make = |bump: bool| {
        let z = [0., 6., -13., 13., -6., 0.];
        NurbsSurface::try_new(
            5,
            5,
            6,
            6,
            (0..36)
                .map(|k| {
                    Point3::try_new(
                        (k % 6) as Real,
                        (k / 6) as Real,
                        if bump { z[k % 6] * z[k / 6] } else { 0. },
                    )
                    .unwrap()
                })
                .collect(),
            [vec![0.; 6], vec![1.; 6]].concat(),
            [vec![0.; 6], vec![1.; 6]].concat(),
        )
        .unwrap()
    };
    let (a, b) = (make(false), make(true));
    let pairs = [0., 0.25, 0.5, 0.75, 1.].map(|t| [t, t]);
    for u in pairs {
        for v in pairs {
            assert_eq!(
                a.evaluate(u[0], v[0]).unwrap(),
                b.evaluate(u[1], v[1]).unwrap()
            );
        }
    }
    assert_eq!(
        a.mapped_surface_deviation_bound(&b, [&pairs; 2], 0.01)
            .unwrap(),
        None
    );
}

#[test]
fn invalid_partial_folded_or_oversized_maps_fail_before_identical_geometry_shortcuts() {
    let a = sheet(false, 0.);
    for pairs in [
        vec![],
        vec![[0., 0.]],
        vec![[0., 0.], [0.5, 1.]],
        vec![[0., 0.], [1., 0.]],
        vec![[0., 0.], [0.5, 0.5], [0.5, 0.75], [1., 1.]],
        vec![[0., 0.], [0.5, 0.75], [0.75, 0.5], [1., 1.]],
        vec![[0., 0.], [Real::NAN, 0.5], [1., 1.]],
        vec![[0., 0.], [Real::INFINITY, 0.5], [1., 1.]],
        (0..4098).map(|i| [i as Real / 4097.; 2]).collect(),
    ] {
        for maps in [[pairs.as_slice(), &IDENTITY], [&IDENTITY, pairs.as_slice()]] {
            assert_eq!(
                a.mapped_surface_deviation_bound(&a, maps, 0.),
                Err(GeometryError::InvalidSurfaceCorrespondence)
            );
        }
    }
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            a.mapped_surface_deviation_bound(&a, [&IDENTITY; 2], limit),
            Err(GeometryError::InvalidTolerance)
        );
    }
}

#[test]
fn excessive_cartesian_map_work_declines_without_visiting_every_rectangle() {
    let a = sheet(false, 0.);
    let pairs = (0..4097)
        .map(|i| [i as Real / 4096.; 2])
        .collect::<Vec<_>>();
    assert_eq!(
        a.mapped_surface_deviation_bound(&a, [&pairs; 2], 0.)
            .unwrap(),
        None
    );
    let mut budget = Budget(100);
    assert!(matches!(
        bound(&a, &a, [&IDENTITY; 2], 1., &mut budget),
        Err(GeometryError::SurfaceCurveCertificateWorkLimit)
    ));
}

#[test]
fn unclamped_sources_are_restricted_exactly_and_unsupported_weights_or_sizes_decline() {
    let make = |p: usize, nu: usize, nv: usize, weights: bool| {
        let knots = |n: usize, degree: usize| (0..n + degree + 1).map(|i| i as Real).collect();
        NurbsSurface::try_new_rational(
            p,
            1,
            nu,
            nv,
            (0..nu * nv)
                .map(|k| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            (k % nu) as Real,
                            (k / nu) as Real,
                            ((k % nu) * (k / nu)) as Real / 8.,
                        )
                        .unwrap(),
                        if weights && k == 1 { -1. } else { 1. },
                    )
                    .unwrap()
                })
                .collect(),
            knots(nu, p),
            knots(nv, 1),
        )
        .unwrap()
    };
    let a = make(2, 4, 3, false);
    let b = a.try_clamped_to_active_domain().unwrap();
    assert!(
        a.mapped_surface_deviation_bound(&b, [&IDENTITY; 2], 1e-12)
            .unwrap()
            .is_some()
    );
    for unsupported in [
        make(2, 4, 3, true),
        make(17, 18, 2, false),
        make(1, 65, 64, false),
    ] {
        assert_eq!(
            unsupported
                .mapped_surface_deviation_bound(&unsupported, [&IDENTITY; 2], 0.)
                .unwrap(),
            None
        );
    }
}
