use super::*;

#[test]
fn excessive_group_count_is_rejected_before_any_basis_fit() {
    let count = 3 + 2 * 257;
    let mut knots = vec![0.; 3];
    for i in 1..=257 {
        knots.extend(std::iter::repeat_n(i as Real / 258., 2));
    }
    knots.extend([1.; 3]);
    let source = NurbsSurface::try_new(
        2,
        1,
        count,
        2,
        (0..2)
            .flat_map(|j| {
                (0..count).map(move |i| Point3::try_new(i as Real, j as Real, 0.).unwrap())
            })
            .collect(),
        knots,
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let mut called = false;
    let result = source.try_reduce_knots_in_range_with_remover(
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
        Err(GeometryError::SurfaceApproximationLimit)
    ));
    assert!(!called);
}

#[test]
fn independent_direction_errors_cannot_accumulate_past_the_original_bound() {
    let source = tests::surface(false);
    let before = source.clone();
    let mut calls = 0;
    let mut shifted =
        |s: &NurbsSurface, axis, knots: &[Real]| -> Result<NurbsSurface, GeometryError> {
            calls += 1;
            let mut result = basis::fit(s, axis, knots)?;
            for p in &mut result.control_points {
                let [x, y, z] = p.point().to_array();
                *p = WeightedPoint3::try_new(Point3::try_new(x, y, z + 0.06)?, p.weight())?;
            }
            Ok(result)
        };
    let result = source
        .try_reduce_knots_in_range_with_remover(
            SurfaceKnotDirection::Both,
            1,
            3,
            Some(0.1),
            &mut shifted,
        )
        .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(result.removed, 1);
    assert!(result.maximum_deviation_bound.unwrap() <= 0.1);
    assert_eq!(result.surface.knots_v(), source.knots_v());
    assert_eq!(source, before);
}

#[test]
fn a_backend_cannot_replace_the_requested_basis_or_the_other_axis() {
    let source = tests::surface(false);
    for alter_other in [false, true] {
        let result = source.try_reduce_knots_in_range_with_remover(
            SurfaceKnotDirection::U,
            1,
            3,
            None,
            &mut |s, axis, k| {
                if alter_other {
                    basis::fit(s, axis, k)?.try_reparameterized(s.domain_u(), 3. ..=9.)
                } else {
                    Ok(s.clone())
                }
            },
        );
        assert!(matches!(result, Err(GeometryError::KnotRemovalSolveFailed)));
    }
}

#[test]
fn a_second_direction_failure_never_exposes_a_partial_result_or_changes_inputs() {
    let source = tests::surface(false);
    let before = source.clone();
    let mut calls = 0;
    let result = source.try_reduce_knots_in_range_with_remover(
        SurfaceKnotDirection::Both,
        1,
        3,
        Some(1e-10),
        &mut |s, axis, k| {
            calls += 1;
            if calls == 2 {
                Err(GeometryError::KnotRemovalSolveFailed)
            } else {
                basis::fit(s, axis, k)
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert_eq!(source, before);
}
