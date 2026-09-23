//! Isolate surface-knot crossings of one rational UV Bézier span.
use super::StepError;
use num_rational::BigRational;
use num_traits::ToPrimitive;
use viboceros_geometry::NurbsCurve;

#[derive(Clone, Copy, Debug)]
pub(super) struct Crossing {
    pub parameter: f64,
    pub u: Option<f64>,
    pub v: Option<f64>,
}

pub(super) fn isolate(
    span: &NurbsCurve,
    u_knots: &[(f64, f64)],
    v_knots: &[(f64, f64)],
    id: u64,
) -> Result<Vec<Crossing>, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let exact = |value: f64| BigRational::from_float(value).unwrap();
    let zero = exact(0.);
    let one = exact(1.);
    let mut found = Vec::new();
    for (axis, rectangles) in [(0, u_knots), (1, v_knots)] {
        let mut knots = rectangles
            .iter()
            .flat_map(|(a, b)| [*a, *b])
            .collect::<Vec<_>>();
        knots.sort_by(f64::total_cmp);
        knots.dedup();
        for knot in knots.into_iter().skip(1).take(rectangles.len() - 1) {
            let controls = span.control_points();
            let low = controls
                .iter()
                .map(|control| coordinate(control, axis))
                .fold(f64::INFINITY, f64::min);
            let high = controls
                .iter()
                .map(|control| coordinate(control, axis))
                .fold(f64::NEG_INFINITY, f64::max);
            if knot <= low || knot >= high {
                continue;
            }
            let knot_exact = exact(knot);
            let coefficients = controls
                .iter()
                .map(|control| {
                    (exact(coordinate(control, axis)) - &knot_exact) * exact(control.weight())
                })
                .collect::<Vec<_>>();
            let mut roots = Vec::new();
            let mut nodes = 0;
            collect(&coefficients, &zero, &one, 0, &mut nodes, &mut roots)
                .map_err(|_| unsupported("curved p-curve knot crossing cannot be isolated"))?;
            roots.sort();
            roots.dedup();
            for fraction in roots {
                if fraction > zero && fraction < one {
                    found.push((fraction, axis, knot));
                }
            }
        }
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    let domain = span.domain();
    let start = exact(*domain.start());
    let end = exact(*domain.end());
    let mut result: Vec<Crossing> = Vec::new();
    let mut previous_fraction: Option<BigRational> = None;
    for (fraction, axis, knot) in found {
        if previous_fraction.as_ref() == Some(&fraction) {
            let last = result.last_mut().unwrap();
            if axis == 0 {
                last.u = Some(knot);
            } else {
                last.v = Some(knot);
            }
            continue;
        }
        let parameter = (&start + (&end - &start) * &fraction)
            .to_f64()
            .ok_or_else(|| unsupported("curved p-curve crossing is not representable"))?;
        if parameter <= result.last().map_or(*domain.start(), |last| last.parameter)
            || parameter >= *domain.end()
        {
            return Err(unsupported(
                "curved p-curve knot crossings are too close to compose",
            ));
        }
        result.push(Crossing {
            parameter,
            u: (axis == 0).then_some(knot),
            v: (axis == 1).then_some(knot),
        });
        previous_fraction = Some(fraction);
    }
    Ok(result)
}

fn coordinate(control: &viboceros_geometry::WeightedPoint3, axis: usize) -> f64 {
    if axis == 0 {
        control.point().x()
    } else {
        control.point().y()
    }
}

fn variations(coefficients: &[BigRational]) -> usize {
    let zero = BigRational::from_float(0.).unwrap();
    let mut previous = 0_i8;
    let mut count = 0;
    for coefficient in coefficients {
        let sign = if coefficient > &zero {
            1
        } else if coefficient < &zero {
            -1
        } else {
            0
        };
        if sign != 0 {
            if previous != 0 && sign != previous {
                count += 1;
            }
            previous = sign;
        }
    }
    count
}

fn collect(
    coefficients: &[BigRational],
    low: &BigRational,
    high: &BigRational,
    depth: usize,
    nodes: &mut usize,
    roots: &mut Vec<BigRational>,
) -> Result<(), ()> {
    *nodes += 1;
    if *nodes > 4096 {
        return Err(());
    }
    let zero = BigRational::from_float(0.).unwrap();
    let one = BigRational::from_float(1.).unwrap();
    if coefficients.iter().all(|coefficient| coefficient == &zero) {
        return Ok(());
    }
    if coefficients[0] == zero && low > &zero && low < &one {
        roots.push(low.clone());
    }
    if coefficients[coefficients.len() - 1] == zero && high > &zero && high < &one {
        roots.push(high.clone());
    }
    let changes = variations(coefficients);
    if changes == 0 {
        return Ok(());
    }
    let half = BigRational::from_float(0.5).unwrap();
    let middle = (low + high) * &half;
    if depth >= 80 {
        if changes == 1 {
            roots.push(middle);
            return Ok(());
        }
        return Err(());
    }
    let degree = coefficients.len() - 1;
    let mut work = coefficients.to_vec();
    let mut left = Vec::with_capacity(degree + 1);
    let mut right = vec![zero; degree + 1];
    for level in 0..=degree {
        left.push(work[0].clone());
        right[degree - level] = work[degree - level].clone();
        for index in 0..degree - level {
            work[index] = (&work[index] + &work[index + 1]) * &half;
        }
    }
    collect(&left, low, &middle, depth + 1, nodes, roots)?;
    collect(&right, &middle, high, depth + 1, nodes, roots)
}
