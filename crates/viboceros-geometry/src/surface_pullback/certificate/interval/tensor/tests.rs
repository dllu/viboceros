use super::*;

fn net(degree: [usize; 2], shift: i64) -> Vec<H> {
    (0..(degree[0] + 1) * (degree[1] + 1))
        .map(|i| {
            let w = Rational::from_integer((1 + (i + shift as usize) % 3).into());
            [
                Rational::new((i as i64 - shift).into(), 3.into()) * &w,
                Rational::new((2 * i as i64 + shift).into(), 7.into()) * &w,
                Rational::new((i as i64 + 3 * shift).into(), 11.into()) * &w,
                w,
            ]
        })
        .collect()
}

#[test]
fn outward_tensor_products_enclose_exact_coefficients_for_unequal_degrees() {
    for degrees in [[[1, 1], [1, 1]], [[2, 1], [1, 3]], [[3, 2], [2, 3]]] {
        let (a, b) = (net(degrees[0], 1), net(degrees[1], 2));
        let mut work = Work {
            remaining: FLOAT_WORK,
            coefficients: BTreeMap::new(),
        };
        let enclosed = difference(
            &Patch::new(&a).unwrap().controls,
            &Patch::new(&b).unwrap().controls,
            degrees,
            &mut work,
        )
        .unwrap();
        // Different weights/degrees force the general exact tensor product.
        let (exact, _) = super::super::super::surface_pair::tensor::difference(
            &a,
            &b,
            degrees,
            &mut Budget(MAX_WORK),
        )
        .unwrap();
        assert_eq!(enclosed.len(), exact.len());
        for (a, b) in enclosed.iter().zip(exact) {
            for k in 0..4 {
                assert!(rational(a[k].lo) <= b[k] && rational(a[k].hi) >= b[k]);
            }
        }
    }
}

#[test]
fn interval_subdivision_certifies_a_tight_interior_bump_and_proves_an_outside_corner() {
    let a = vec![
        [
            Rational::zero(),
            Rational::zero(),
            Rational::zero(),
            Rational::one()
        ];
        9
    ];
    let mut b = a.clone();
    b[4][2] = Rational::one();
    let call = |a: &[H], b: &[H], limit| {
        surfaces(
            a,
            b,
            [[2; 2]; 2],
            limit,
            &mut Coefficients::default(),
            &mut Budget(MAX_WORK),
        )
        .unwrap()
    };
    assert!(matches!(call(&a, &b, 0.3), Outcome::Within(bound) if (0.25..=0.3).contains(&bound)));
    b[0][2] = Rational::one();
    assert!(matches!(call(&a, &b, 0.3), Outcome::Outside));
    for limit in [0., Real::from_bits(1)] {
        assert!(matches!(call(&a, &a, limit), Outcome::Inconclusive));
    }
    let mut budget = Budget(0);
    assert!(matches!(
        surfaces(
            &a,
            &a,
            [[2; 2]; 2],
            1.,
            &mut Coefficients::default(),
            &mut budget
        )
        .unwrap(),
        Outcome::Inconclusive
    ));
}
