use super::*;
#[test]
fn native_edit_point_solver_reuses_original_open_and_periodic_bases() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mut curves = vec![
        NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(p(0., 0., 2.), 1.).unwrap(),
                WeightedPoint3::try_new(p(1., 2., 3.), 0.5).unwrap(),
                WeightedPoint3::try_new(p(3., 0., 2.), 2.).unwrap(),
            ],
            vec![2., 2., 2., 7., 7., 7.],
        )
        .unwrap(),
        NurbsCurve::try_control_point_curve_with_closure(
            3,
            vec![
                p(2., 0., 2.),
                p(1., 2., 2.),
                p(-1., 2., 2.),
                p(-2., 0., 2.),
                p(-1., -2., 2.),
                p(1., -2., 2.),
            ],
            viboceros_geometry::ControlPointCurveClosure::Smooth,
        )
        .unwrap(),
    ];
    for degree in [2, 4] {
        curves.push(
            NurbsCurve::try_control_point_curve_with_closure(
                degree,
                vec![
                    p(2., 0., 2.),
                    p(1., 2., 2.),
                    p(-1., 2., 2.),
                    p(-2., 0., 2.),
                    p(-1., -2., 2.),
                    p(1., -2., 2.),
                ],
                viboceros_geometry::ControlPointCurveClosure::Smooth,
            )
            .unwrap(),
        );
    }
    for original in curves {
        for gauge in [1., -1.] {
            let curve = NurbsCurve::try_new_rational(
                original.degree(),
                original
                    .control_points()
                    .iter()
                    .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * gauge).unwrap())
                    .collect(),
                original.knots().to_vec(),
            )
            .unwrap();
            let points = curve
                .edit_points()
                .unwrap()
                .iter()
                .map(|e| p(e.point.x() + 4., e.point.y(), 0.))
                .collect::<Vec<_>>();
            let output = curve
                .try_with_edit_points_with_solver(
                    &points,
                    Tolerance::DEFAULT,
                    &mut curve_edit_point_interpolation,
                )
                .unwrap();
            assert_eq!(output.knots(), curve.knots());
            assert_eq!(output.is_periodic(), curve.is_periodic());
        }
    }
}
