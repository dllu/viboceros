use super::*;

#[test]
fn exact_curve_jets_match_independent_fraction_basis_reference_bits() {
    let mut count = 0;
    let mut poles = 0;
    let mut overflows = 0;
    for line in include_str!("reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        let degree = fields.next().unwrap().parse::<usize>().unwrap();
        let control_count = fields.next().unwrap().parse::<usize>().unwrap();
        let side = match fields.next().unwrap() {
            "L" => ParameterSide::Left,
            "R" => ParameterSide::Right,
            _ => panic!("unexpected side"),
        };
        let parse = |field: &str| Real::from_bits(u64::from_str_radix(field, 16).unwrap());
        let knots = (0..control_count + degree + 1)
            .map(|_| parse(fields.next().unwrap()))
            .collect();
        let parameter = parse(fields.next().unwrap());
        let controls = (0..control_count)
            .map(|_| {
                let [x, y, z, w] = std::array::from_fn(|_| parse(fields.next().unwrap()));
                WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), w).unwrap()
            })
            .collect();
        let curve = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
        let span = curve.checked_span_on_side(parameter, side).unwrap();
        assert!(curve.homogeneous_controls(span, true).unwrap().range_loss);
        let expected = fields.collect::<Vec<_>>();
        for order in 0..=2 {
            let actual = match order {
                0 => curve
                    .evaluate_on_side(parameter, side)
                    .map(|p| p.to_array().to_vec()),
                1 => curve
                    .evaluate_with_derivative_on_side(parameter, side)
                    .map(|(p, d)| [p.to_array(), d.to_array()].concat()),
                _ => curve
                    .evaluate_with_second_derivative_on_side(parameter, side)
                    .map(|(p, d, dd)| [p.to_array(), d.to_array(), dd.to_array()].concat()),
            };
            if expected == ["pole"] {
                assert_eq!(actual, Err(GeometryError::ZeroWeightAtParameter));
                poles += 1;
                continue;
            }
            assert_eq!(expected.len(), 9);
            let expected = expected[..3 * (order + 1)]
                .iter()
                .map(|value| parse(value))
                .collect::<Vec<_>>();
            if expected.iter().any(|value| !value.is_finite()) {
                assert!(
                    matches!(actual, Err(GeometryError::NonFinite { .. })),
                    "case {count}, order {order}: {actual:?}"
                );
                overflows += 1;
            } else {
                assert_eq!(
                    actual
                        .unwrap()
                        .into_iter()
                        .map(Real::to_bits)
                        .collect::<Vec<_>>(),
                    expected.into_iter().map(Real::to_bits).collect::<Vec<_>>(),
                    "case {count}, order {order}, side {side:?}",
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 127);
    assert_eq!(poles, 3);
    assert!(overflows > 0);
}
