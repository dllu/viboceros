use super::*;

fn integer(value: i64) -> Rational {
    Rational::from_integer(value.into())
}

// Independent point evaluation in the original B-spline basis. Unlike the
// extractor, this never forms a restricted control net or recognizes Beziers.
fn spline_value(
    knots: &[Rational],
    degree: usize,
    span: usize,
    controls: &[H],
    parameter: &Rational,
) -> H {
    let mut work = controls[span - degree..=span].to_vec();
    for level in 1..=degree {
        for j in (level..=degree).rev() {
            let i = span - degree + j;
            let alpha = (parameter - &knots[i]) / (&knots[i + degree + 1 - level] - &knots[i]);
            work[j] = std::array::from_fn(|axis| {
                (Rational::one() - &alpha) * &work[j - 1][axis] + &alpha * &work[j][axis]
            });
        }
    }
    work.pop().unwrap()
}

fn bezier_value(net: &[H], fraction: &Rational) -> H {
    let mut work = net.to_vec();
    for remaining in (1..work.len()).rev() {
        for j in 0..remaining {
            work[j] = std::array::from_fn(|axis| {
                (Rational::one() - fraction) * &work[j][axis] + fraction * &work[j + 1][axis]
            });
        }
    }
    work.remove(0)
}

#[test]
fn exact_extraction_preserves_smooth_c0_and_disconnected_rational_spans() {
    for degree in (1..=8).chain([12, MAX_DEGREE]) {
        let mut multiplicities = vec![1, degree.saturating_sub(1).max(1), degree, degree + 1];
        multiplicities.sort();
        multiplicities.dedup();
        for multiplicity in multiplicities {
            let knots = [
                vec![integer(-3); degree + 1],
                vec![integer(2); multiplicity],
                vec![integer(7); degree + 1],
            ]
            .concat();
            let controls = (0..degree + multiplicity + 1)
                .map(|i| {
                    [
                        integer(i as i64 - 4) / integer(3),
                        integer((i * i % 13) as i64 - 6) / integer(7),
                        integer((i % 5) as i64) / integer(11),
                        integer((i % 3 + 1) as i64) / integer(3),
                    ]
                })
                .collect::<Vec<H>>();
            for span in [degree, degree + multiplicity] {
                let a = knots[span].clone();
                let b = knots[span + 1].clone();
                let width = &b - &a;
                // Whole spans exercise the shared-endpoint shortcut. Partial
                // spans and lesser multiplicities must retain restriction.
                for (left, right) in [
                    (a.clone(), b.clone()),
                    (&a + &width / integer(8), &b - &width / integer(4)),
                ] {
                    let mut budget = Budget(MAX_WORK);
                    let net = extract(
                        &knots,
                        degree,
                        span,
                        &controls[span - degree..=span],
                        &left,
                        &right,
                        &mut budget,
                    )
                    .unwrap();
                    for fraction in [Rational::zero(), integer(2) / integer(7), Rational::one()] {
                        let parameter = &left + (&right - &left) * &fraction;
                        assert_eq!(
                            bezier_value(&net, &fraction),
                            spline_value(&knots, degree, span, &controls, &parameter),
                            "degree={degree}, multiplicity={multiplicity}, span={span}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn exact_degree_zero_extraction_keeps_the_constant_net() {
    let controls = vec![[integer(2), integer(-3), integer(4), integer(5)]];
    for (left, right) in [(integer(-3), integer(7)), (integer(1), integer(1))] {
        assert_eq!(
            extract(
                &[integer(-3), integer(7)],
                0,
                0,
                &controls,
                &left,
                &right,
                &mut Budget(MAX_WORK),
            )
            .unwrap(),
            controls
        );
    }
}
