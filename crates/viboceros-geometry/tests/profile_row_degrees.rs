#[path = "support/profile_rows.rs"]
mod profile_rows;
use viboceros_geometry::{NurbsSurface, Point3, WeightedPoint3};

fn strip() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                WeightedPoint3::try_new(Point3::try_from(p).unwrap(), if i < 2 { 2. } else { 3. })
                    .unwrap()
            })
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn u_elevation_is_certified_in_both_orders_and_rail_directions() {
    let source = strip();
    let before = source.clone();
    let elevated = source.try_change_degree(3, 1, false).unwrap();
    for target in [elevated.clone(), elevated.try_reversed_u().unwrap()] {
        assert!(profile_rows::complete_bound(&source, &target, 1e-12).unwrap() <= 1e-12);
        assert!(profile_rows::complete_bound(&target, &source, 1e-12).unwrap() <= 1e-12);
        // Comparing an elevated copy to itself would silently discard the
        // stored-coordinate and weight rounding introduced by elevation.
        assert!(profile_rows::complete_bound(&source, &target, 1e-30).is_none());
        assert!(profile_rows::complete_bound(&target, &source, 1e-30).is_none());
    }
    assert_eq!(source, before);
}

#[test]
fn matching_end_rows_cannot_hide_a_changed_interior_u_row() {
    let source = strip();
    let raised = source.try_change_degree(3, 1, false).unwrap();
    let mut controls = raised.control_points().to_vec();
    let c = controls[1];
    controls[1] = WeightedPoint3::try_new(
        Point3::try_new(c.point().x(), c.point().y(), 0.1).unwrap(),
        c.weight(),
    )
    .unwrap();
    let bad = NurbsSurface::try_new_rational(
        raised.degree_u(),
        raised.degree_v(),
        raised.control_point_count_u(),
        raised.control_point_count_v(),
        controls,
        raised.knots_u().to_vec(),
        raised.knots_v().to_vec(),
    )
    .unwrap();
    assert!(profile_rows::complete_bound(&source, &bad, 1e-7).is_none());
    assert!(profile_rows::complete_bound(&bad, &source, 1e-7).is_none());
}

#[test]
fn coupled_uv_weights_decline_the_elevation_proposal() {
    let base = strip();
    let mut controls = base.control_points().to_vec();
    let c = controls[1];
    controls[1] = WeightedPoint3::try_new(c.point(), 1.).unwrap();
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        controls,
        base.knots_u().to_vec(),
        base.knots_v().to_vec(),
    )
    .unwrap();
    let raised = source.try_change_degree(3, 1, false).unwrap();
    assert!(profile_rows::complete_bound(&source, &raised, 1e-7).is_none());
}
