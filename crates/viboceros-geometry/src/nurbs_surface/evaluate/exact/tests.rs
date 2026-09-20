use super::*;
use crate::WeightedPoint3;

#[test]
fn exact_surface_query_prepares_derivative_nets_lazily_and_reuses_their_storage() {
    let surface = NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        (0..9)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new((i % 3) as Real, (i / 3) as Real, (i * i) as Real).unwrap(),
                    if i == 4 { -0.25 } else { 1. },
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut net = ExactJetNet::new(&surface, [2, 2]);
    let controls = net.controls.as_ptr();
    net.evaluate([0.1, 0.2], 0).unwrap();
    assert!(net.first.is_none() && net.second.is_none());
    net.evaluate([0.3, 0.4], 1).unwrap();
    let first = net.first.as_ref().unwrap();
    let first_ptrs = [first.u.as_ptr(), first.v.as_ptr()];
    assert!(net.second.is_none());
    net.evaluate([0.5, 0.6], 2).unwrap();
    let second = net.second.as_ref().unwrap();
    let second_ptrs = [
        second.uu.as_ref().unwrap().as_ptr(),
        second.uv.as_ptr(),
        second.vv.as_ref().unwrap().as_ptr(),
    ];
    for order in [0, 1, 2, 2] {
        net.evaluate([0.7, 0.8], order).unwrap();
        assert_eq!(net.controls.as_ptr(), controls);
        let first = net.first.as_ref().unwrap();
        assert_eq!([first.u.as_ptr(), first.v.as_ptr()], first_ptrs);
        let second = net.second.as_ref().unwrap();
        assert_eq!(
            [
                second.uu.as_ref().unwrap().as_ptr(),
                second.uv.as_ptr(),
                second.vv.as_ref().unwrap().as_ptr()
            ],
            second_ptrs
        );
    }
}

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
        let expected = fields.collect::<Vec<_>>();
        let (u, v) = (header[4], header[5]);
        let extended = !surface.domain_u().contains(&u) || !surface.domain_v().contains(&v);
        if !extended {
            let mut grid_point = None;
            surface.for_each_grid_point(&[u], &[v], |_, _, point| grid_point = Some(point));
            let point = grid_point.unwrap();
            if expected == ["pole"] {
                assert_eq!(point, Err(GeometryError::ZeroWeightAtParameter));
            } else {
                let coordinates = expected[..3]
                    .iter()
                    .map(|value| parse(value))
                    .collect::<Vec<_>>();
                if coordinates.iter().any(|value| !value.is_finite()) {
                    assert!(matches!(point, Err(GeometryError::NonFinite { .. })));
                } else {
                    assert_eq!(
                        point.unwrap().to_array().map(Real::to_bits).as_slice(),
                        coordinates.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        "grid case {count}"
                    );
                }
            }
        }
        let mut prepared = ExactJetNet::new(&surface, [du, dv]);
        for order in [2, 0, 1, 2] {
            let result = prepared.evaluate([u, v], order);
            assert_eq!(
                result,
                surface.evaluate_jet([u, v], [ParameterSide::Right; 2], extended, order),
                "prepared dispatch case {count}, order {order}"
            );
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
    assert_eq!(count, 150);
    assert!(overflow_count > 0);
}
