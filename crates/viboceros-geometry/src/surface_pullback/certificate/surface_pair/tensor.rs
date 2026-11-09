//! Rational tensor products and exact half subdivision of their positive hulls.
use super::*;

pub(super) fn difference(
    a: &[H],
    b: &[H],
    degrees: [[usize; 2]; 2],
    budget: &mut Budget,
) -> Result<(Vec<H>, [usize; 2]), GeometryError> {
    // An identical positive denominator needs only coefficient subtraction.
    // This includes polynomial tensors and avoids a quadratic product for the
    // common constant-weight reductions without changing the proof.
    if degrees[0] == degrees[1] && a.iter().zip(b).all(|(a, b)| a[3] == b[3]) {
        budget.charge(4 * a.len())?;
        let net = a
            .iter()
            .zip(b)
            .map(|(a, b)| {
                let h: H =
                    std::array::from_fn(|i| if i == 3 { a[3].clone() } else { &a[i] - &b[i] });
                for r in &h {
                    budget.check(r)?;
                }
                Ok(h)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        return Ok((net, degrees[0]));
    }
    budget.charge(4 * a.len() * b.len())?;
    let [[pu, pv], [qu, qv]] = degrees;
    let degree = [pu + qu, pv + qv];
    let coefficients = [
        (0..=pu)
            .map(|i| {
                (0..=qu)
                    .map(|j| {
                        Rational::new(
                            algebra::binomial(pu, i) * algebra::binomial(qu, j),
                            algebra::binomial(pu + qu, i + j),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        (0..=pv)
            .map(|i| {
                (0..=qv)
                    .map(|j| {
                        Rational::new(
                            algebra::binomial(pv, i) * algebra::binomial(qv, j),
                            algebra::binomial(pv + qv, i + j),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
    ];
    let mut output =
        vec![std::array::from_fn(|_| Rational::zero()); (degree[0] + 1) * (degree[1] + 1)];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            let u = i % (pu + 1) + j % (qu + 1);
            let v = i / (pu + 1) + j / (qu + 1);
            let c = &coefficients[0][i % (pu + 1)][j % (qu + 1)]
                * &coefficients[1][i / (pu + 1)][j / (qu + 1)];
            let h = &mut output[v * (degree[0] + 1) + u];
            for axis in 0..3 {
                h[axis] += &c * (&a[axis] * &b[3] - &b[axis] * &a[3]);
            }
            h[3] += c * &a[3] * &b[3];
            for r in h {
                budget.check(r)?;
            }
        }
    }
    Ok((output, degree))
}

pub(super) fn hull_bound(
    net: Vec<H>,
    degree: [usize; 2],
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    let mut pending = vec![(net, 0)];
    let mut maximum = 0_f64;
    while let Some((net, depth)) = pending.pop() {
        budget.charge(4 * net.len())?;
        // The four tensor corners are attained; an outside corner rejects this
        // correspondence without relying on a control-hull overestimate.
        for i in [0, degree[0], degree[1] * (degree[0] + 1), net.len() - 1] {
            if algebra::norm_bound(&net[i], limit).is_none() {
                return Ok(None);
            }
        }
        if let Some(upper) = net.iter().try_fold(0_f64, |upper, h| {
            Some(upper.max(algebra::norm_bound(h, limit)?))
        }) {
            maximum = maximum.max(upper);
            continue;
        }
        if depth == MAX_DEPTH {
            return Ok(None);
        }
        let children = split(&net, degree, depth % 2, budget)?;
        pending.extend(children.into_iter().map(|net| (net, depth + 1)));
    }
    Ok(Some(maximum))
}

fn split(
    net: &[H],
    degree: [usize; 2],
    axis: usize,
    budget: &mut Budget,
) -> Result<[Vec<H>; 2], GeometryError> {
    let [pu, pv] = degree;
    let mut children = [net.to_vec(), net.to_vec()];
    if axis == 0 {
        for j in 0..=pv {
            let range = j * (pu + 1)..(j + 1) * (pu + 1);
            let (a, b) = algebra::split(&net[range.clone()], budget)?;
            children[0][range.clone()].clone_from_slice(&a);
            children[1][range].clone_from_slice(&b);
        }
    } else {
        for i in 0..=pu {
            let column = (0..=pv)
                .map(|j| net[j * (pu + 1) + i].clone())
                .collect::<Vec<_>>();
            let (a, b) = algebra::split(&column, budget)?;
            for j in 0..=pv {
                children[0][j * (pu + 1) + i] = a[j].clone();
                children[1][j * (pu + 1) + i] = b[j].clone();
            }
        }
    }
    Ok(children)
}
