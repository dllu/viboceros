use super::*;

pub(super) fn surface(constant: bool) -> NurbsSurface {
    NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    Point3::try_new(
                        if constant { 2. } else { i as Real / 2. },
                        if constant { 3. } else { j as Real / 2. },
                        if constant { 4. } else { (i + j) as Real / 4. },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
    .try_insert_knot_u(0.5, 2)
    .unwrap()
    .try_insert_knot_v(0.5, 2)
    .unwrap()
}

#[test]
fn exact_zero_reduces_constant_tensors_and_preserves_source_and_other_axis() {
    let source = surface(true);
    let before = source.clone();
    let result = source
        .try_reduce_knots_in_range(SurfaceKnotDirection::Both, 1, 3, Some(0.))
        .unwrap();
    assert_eq!(result.removed, 2);
    assert_eq!(result.maximum_deviation_bound, Some(0.));
    assert_eq!(
        (
            result.surface.control_point_count_u(),
            result.surface.control_point_count_v()
        ),
        (4, 4)
    );
    let u = source
        .try_reduce_knot_multiplicity_with_bound(SurfaceKnotDirection::U, 0.5, 0, 0.)
        .unwrap()
        .unwrap();
    assert_eq!(u.removed, 2);
    assert_eq!(u.surface.knots_v(), source.knots_v());
    assert_eq!(u.surface.control_point_count_u(), 3);
    let unchanged = source
        .try_reduce_knot_multiplicity_with_bound(SurfaceKnotDirection::V, 0.5, 2, 0.)
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.surface, source);
    assert_eq!(unchanged.removed, 0);
    assert_eq!(source, before);
}

#[test]
fn rational_reductions_return_complete_original_surface_bounds() {
    let source = surface(false);
    for gauge in [1., -2.] {
        let rational = NurbsSurface::try_new_rational(
            source.degree_u,
            source.degree_v,
            source.control_point_count_u,
            source.control_point_count_v,
            source
                .control_points
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    WeightedPoint3::try_new(c.point(), gauge * (1. + (i % 3) as Real / 8.)).unwrap()
                })
                .collect(),
            source.knots_u.clone(),
            source.knots_v.clone(),
        )
        .unwrap();
        for axis in [
            SurfaceKnotDirection::U,
            SurfaceKnotDirection::V,
            SurfaceKnotDirection::Both,
        ] {
            let before = rational.clone();
            let result = rational
                .try_reduce_knots_in_range(axis, 1, 3, Some(1.))
                .unwrap();
            assert!(result.removed > 0);
            assert!(result.maximum_deviation_bound.unwrap() <= 1.);
            assert!(
                rational
                    .parameter_surface_deviation_bound(&result.surface, 1.)
                    .unwrap()
                    .is_some()
            );
            assert_eq!(rational, before);
        }
    }
}

#[test]
fn ranges_are_exclusive_and_select_directions_independently() {
    let source = NurbsSurface::try_new(
        3,
        3,
        4,
        4,
        (0..4)
            .flat_map(|j| {
                (0..4).map(move |i| Point3::try_new(i as Real, j as Real, (i + j) as Real).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
    .try_insert_knot_u(0.25, 2)
    .unwrap()
    .try_insert_knot_u(0.75, 3)
    .unwrap()
    .try_insert_knot_v(0.5, 2)
    .unwrap();
    for (minimum, maximum, removed) in [(0, 3, 2), (1, 3, 2), (2, 4, 2), (3, 4, 0), (1, 4, 4)] {
        let result = source
            .try_reduce_knots_in_range(SurfaceKnotDirection::Both, minimum, maximum, None)
            .unwrap();
        assert_eq!(result.removed, removed);
        assert_eq!(result.maximum_deviation_bound, None);
    }
    let u = source
        .try_reduce_knots_in_range(SurfaceKnotDirection::U, 1, 4, None)
        .unwrap();
    assert_eq!(u.removed, 3);
    assert_eq!(u.surface.knots_v(), source.knots_v());
    let v = source
        .try_reduce_knots_in_range(SurfaceKnotDirection::V, 1, 4, None)
        .unwrap();
    assert_eq!(v.removed, 1);
    assert_eq!(v.surface.knots_u(), source.knots_u());
}

#[test]
fn a_linear_interior_peak_is_declined_when_it_exceeds_the_bound() {
    let source = NurbsSurface::try_new(
        1,
        1,
        3,
        3,
        (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    Point3::try_new(i as Real, j as Real, if i == 1 && j == 1 { 1. } else { 0. })
                        .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0.5, 1., 1.],
        vec![0., 0., 0.5, 1., 1.],
    )
    .unwrap();
    let bounded = source
        .try_reduce_knots_in_range(SurfaceKnotDirection::Both, 1, 2, Some(0.001))
        .unwrap();
    assert_eq!(bounded.removed, 0);
    assert_eq!(bounded.surface, source);
    assert_eq!(
        source
            .try_reduce_knot_multiplicity_with_bound(SurfaceKnotDirection::U, 0.5, 0, 0.001)
            .unwrap(),
        None
    );
    let unrestricted = source
        .try_reduce_knots_in_range(SurfaceKnotDirection::Both, 1, 2, None)
        .unwrap();
    assert_eq!(unrestricted.removed, 2);
    assert_eq!(
        source
            .parameter_surface_deviation_bound(&unrestricted.surface, 0.001)
            .unwrap(),
        None
    );
}

#[test]
fn invalid_and_unsupported_sources_fail_before_backend_work() {
    let source = surface(false);
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            source.try_reduce_knots_in_range(SurfaceKnotDirection::U, 1, 3, Some(limit)),
            Err(GeometryError::InvalidTolerance)
        ));
    }
    assert!(matches!(
        source.try_reduce_knots_in_range(SurfaceKnotDirection::U, 2, 2, None),
        Err(GeometryError::InvalidKnotReductionRange { .. })
    ));
    assert!(matches!(
        source.try_reduce_knot_multiplicity_with_bound(SurfaceKnotDirection::Both, 0.5, 1, 0.),
        Err(GeometryError::InvalidControlNet { .. })
    ));
    assert!(matches!(
        source.try_reduce_knot_multiplicity_with_bound(SurfaceKnotDirection::U, 0.25, 1, 0.),
        Err(GeometryError::NoRemovableKnot { .. })
    ));
    let periodic = NurbsSurface::try_new(
        2,
        2,
        5,
        5,
        [(0., 0.), (2., -1.), (3., 4.), (0., 0.), (2., -1.)]
            .into_iter()
            .flat_map(|(y, z)| (0..5).map(move |i| Point3::try_new(i as Real, y, z).unwrap()))
            .collect(),
        source.knots_u.clone(),
        vec![-2., -1., 0., 1., 2., 3., 4., 5.],
    )
    .unwrap();
    assert!(periodic.is_periodic_v());
    let mut called = false;
    let result = periodic.try_reduce_knots_in_range_with_remover(
        SurfaceKnotDirection::Both,
        1,
        3,
        None,
        &mut |s, _, _| {
            called = true;
            Ok::<_, GeometryError>(s.clone())
        },
    );
    assert!(matches!(
        result,
        Err(GeometryError::PeriodicKnotRemovalUnsupported { .. })
    ));
    assert!(!called);
    assert!(
        periodic
            .try_reduce_knots_in_range(SurfaceKnotDirection::U, 1, 3, None)
            .unwrap()
            .surface
            .is_periodic_v()
    );
    let mut mixed = source.clone();
    mixed.control_points[0] =
        WeightedPoint3::try_new(mixed.control_points[0].point(), -1.).unwrap();
    assert!(matches!(
        mixed.try_reduce_knots_in_range(SurfaceKnotDirection::U, 1, 3, Some(1.)),
        Err(GeometryError::UnsupportedSurfaceApproximation { .. })
    ));
}

#[test]
fn unclamped_selected_axis_is_bounded_against_the_complete_stored_tensor() {
    let source = NurbsSurface::try_new(
        2,
        1,
        6,
        2,
        (0..2)
            .flat_map(|j| {
                (0..6).map(move |i| {
                    Point3::try_new(i as Real, j as Real, (i * i + j) as Real / 8.).unwrap()
                })
            })
            .collect(),
        vec![-2., -1., 0., 0.5, 0.5, 1., 2., 3., 4.],
        vec![-1., -1., 2., 2.],
    )
    .unwrap();
    let result = source
        .try_reduce_knots_in_range(SurfaceKnotDirection::U, 1, 3, Some(10.))
        .unwrap();
    assert_eq!(result.removed, 1);
    assert_eq!(result.surface.domain_u(), source.domain_u());
    assert_eq!(result.surface.knots_v(), source.knots_v());
    assert!(
        source
            .parameter_surface_deviation_bound(&result.surface, 10.)
            .unwrap()
            .is_some()
    );
}
