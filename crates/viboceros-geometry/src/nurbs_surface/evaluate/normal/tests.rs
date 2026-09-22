use super::*;
use crate::WeightedPoint3;

#[test]
fn filtered_and_exact_normals_match_independent_fraction_bernstein_reference() {
    let (mut count, mut accepted, mut poles, mut degenerate) = (0, 0, 0, 0);
    for line in include_str!("reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        let du = fields.next().unwrap().parse::<usize>().unwrap();
        let dv = fields.next().unwrap().parse::<usize>().unwrap();
        let parse = |s: &str| Real::from_bits(u64::from_str_radix(s, 16).unwrap());
        let [u0, u1, v0, v1, u, v] = std::array::from_fn(|_| parse(fields.next().unwrap()));
        let controls = (0..(du + 1) * (dv + 1))
            .map(|_| {
                let [x, y, z, w] = std::array::from_fn(|_| parse(fields.next().unwrap()));
                WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), w).unwrap()
            })
            .collect();
        let surface = NurbsSurface::try_new_rational(
            du,
            dv,
            du + 1,
            dv + 1,
            controls,
            [vec![u0; du + 1], vec![u1; du + 1]].concat(),
            [vec![v0; dv + 1], vec![v1; dv + 1]].concat(),
        )
        .unwrap();
        let expected = fields.collect::<Vec<_>>();
        let filtered = filter::normal(&surface, [u, v], [du, dv]);
        let exact = exact::ExactJetNet::new(&surface, [du, dv]).normal([u, v]);
        let actual = surface.normal_at(u, v);
        if expected == ["pole"] {
            assert!(filtered.is_none());
            assert_eq!(exact, Err(GeometryError::ZeroWeightAtParameter));
            assert_eq!(actual, exact);
            poles += 1;
        } else if expected == ["degenerate"] {
            assert!(filtered.is_none());
            assert!(matches!(exact, Err(GeometryError::Degenerate { .. })));
            assert_eq!(actual, exact);
            degenerate += 1;
        } else {
            assert_eq!(expected.len(), 3);
            for (value, expected) in exact
                .unwrap()
                .as_vector()
                .to_array()
                .into_iter()
                .zip(&expected)
            {
                assert!(
                    (value - parse(expected)).abs() <= 1e-15,
                    "exact case {count}"
                );
            }
            let actual = actual.unwrap();
            for (value, expected) in actual.as_vector().to_array().into_iter().zip(&expected) {
                assert!(
                    (value - parse(expected)).abs() <= 1.001e-12,
                    "public case {count}"
                );
            }
            if let Some(filtered) = filtered {
                assert_eq!(actual, filtered);
                accepted += 1;
            }
        }
        count += 1;
    }
    eprintln!(
        "normal reference: {count} cases, {accepted} floating filters, {poles} poles, {degenerate} degeneracies"
    );
    assert_eq!((count, poles, degenerate), (95, 2, 2));
    assert!(
        accepted >= 20,
        "ordinary cases must use the bounded floating path"
    );
    assert!(
        accepted < count - poles - degenerate,
        "exact recovery must also be exercised"
    );
}

#[test]
#[ignore = "manual native normal-query microbenchmark, not a Rhino speed comparison"]
fn regular_normal_query_microbenchmark() {
    use std::{hint::black_box, time::Instant};
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [1., 0., 1.], [0., 1., 2.], [1., 1., 3.]]
            .map(|p| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), 1.).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(filter::normal(&surface, [0.25, 0.75], [1, 1]).is_some());
    let count = 10_000;
    for exact in [false, true] {
        let start = Instant::now();
        for _ in 0..count {
            black_box(if exact {
                exact::ExactJetNet::new(black_box(&surface), [1, 1]).normal([0.25, 0.75])
            } else {
                black_box(&surface).normal_at(0.25, 0.75)
            })
            .unwrap();
        }
        eprintln!("exact={exact}: {:?} per query", start.elapsed() / count);
    }
}
