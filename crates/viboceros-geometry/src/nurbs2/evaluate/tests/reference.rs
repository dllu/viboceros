use super::*;

#[test]
fn exact_uv_jets_match_independent_fraction_basis_reference_bits() {
    // X/Y components of the existing independently generated curve reference
    // are also exact UV references. Z and second derivatives are not requested.
    let mut count = 0;
    let mut poles = 0;
    let mut overflows = 0;
    for line in include_str!("../../../nurbs/evaluate/exact/reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        let degree = fields.next().unwrap().parse::<usize>().unwrap();
        let control_count = fields.next().unwrap().parse::<usize>().unwrap();
        let side = match fields.next().unwrap() {
            "L" => Left,
            "R" => Right,
            _ => panic!("unexpected side"),
        };
        let parse = |field: &str| Real::from_bits(u64::from_str_radix(field, 16).unwrap());
        let knots = (0..control_count + degree + 1)
            .map(|_| parse(fields.next().unwrap()))
            .collect();
        let parameter = parse(fields.next().unwrap());
        let controls = (0..control_count)
            .map(|_| {
                let [x, y, _z, w] = std::array::from_fn(|_| parse(fields.next().unwrap()));
                WeightedPoint2::try_new(p(x, y), w).unwrap()
            })
            .collect();
        let curve = NurbsCurve2::try_new_rational(degree, controls, knots).unwrap();
        let span = curve.checked_span(parameter, side).unwrap();
        assert!(curve.controls(span, true).unwrap().range_loss);
        let expected = fields.collect::<Vec<_>>();
        for derivative in [false, true] {
            let actual = if derivative {
                curve
                    .evaluate_with_derivative_on_side(parameter, side)
                    .map(|(p, d)| [p.to_array(), d].concat())
            } else {
                curve
                    .evaluate_on_side(parameter, side)
                    .map(|p| p.to_array().to_vec())
            };
            if expected == ["pole"] {
                assert_eq!(actual, Err(GeometryError::ZeroWeightAtParameter));
                poles += 1;
                continue;
            }
            assert_eq!(expected.len(), 9);
            let indices = if derivative {
                &[0, 1, 3, 4][..]
            } else {
                &[0, 1][..]
            };
            let expected = indices
                .iter()
                .map(|&i| parse(expected[i]))
                .collect::<Vec<_>>();
            if expected.iter().any(|x| !x.is_finite()) {
                assert!(
                    matches!(actual, Err(GeometryError::NonFinite { .. })),
                    "case {count}: {actual:?}"
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
                    "case {count}, derivative {derivative}, side {side:?}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 127);
    assert_eq!(poles, 2);
    assert!(overflows > 0);
}
