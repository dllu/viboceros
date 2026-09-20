use super::*;
use crate::WeightedPoint3;

#[test]
fn exact_surface_jets_match_independent_fraction_bernstein_reference_bits() {
    let mut count = 0;
    let mut overflow_count = 0;
    for line in include_str!("reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        let du = fields.next().unwrap().parse::<usize>().unwrap();
        let dv = fields.next().unwrap().parse::<usize>().unwrap();
        let parse = |field: &str| Real::from_bits(u64::from_str_radix(field, 16).unwrap());
        let header: [Real; 6] = std::array::from_fn(|_| parse(fields.next().unwrap()));
        let controls = (0..(du + 1) * (dv + 1))
            .map(|_| {
                let [x, y, z, w] = std::array::from_fn(|_| parse(fields.next().unwrap()));
                WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), w).unwrap()
            })
            .collect();
        let knots = |degree, start, end| [vec![start; degree + 1], vec![end; degree + 1]].concat();
        let surface = NurbsSurface::try_new_rational(
            du,
            dv,
            du + 1,
            dv + 1,
            controls,
            knots(du, header[0], header[1]),
            knots(dv, header[2], header[3]),
        )
        .unwrap();
        assert!(
            surface
                .evaluation_controls([du, dv], true)
                .unwrap()
                .range_loss
        );
        let expected = fields.collect::<Vec<_>>();
        let (u, v) = (header[4], header[5]);
        let extended = !surface.domain_u().contains(&u) || !surface.domain_v().contains(&v);
        for order in 0..=2 {
            let result = surface.evaluate_jet([u, v], [ParameterSide::Right; 2], extended, order);
            if expected == ["pole"] {
                assert_eq!(result, Err(GeometryError::ZeroWeightAtParameter));
                continue;
            }
            assert_eq!(expected.len(), 18);
            let components = [3, 9, 18][usize::from(order)];
            let expected = expected[..components]
                .iter()
                .map(|value| parse(value))
                .collect::<Vec<_>>();
            if expected.iter().any(|value| !value.is_finite()) {
                assert!(
                    matches!(result, Err(GeometryError::NonFinite { .. })),
                    "case {count}, order {order}: {result:?}"
                );
                overflow_count += 1;
                continue;
            }
            let jet = result.unwrap();
            let actual = [
                jet.point.to_array(),
                jet.derivative_u.to_array(),
                jet.derivative_v.to_array(),
                jet.derivative_uu.to_array(),
                jet.derivative_uv.to_array(),
                jet.derivative_vv.to_array(),
            ]
            .concat();
            assert_eq!(
                actual[..components]
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "case {count}, order {order}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 70);
    assert!(overflow_count > 0);
}
