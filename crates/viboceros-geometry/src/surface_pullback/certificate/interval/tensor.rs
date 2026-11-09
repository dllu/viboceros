//! Outward tensor difference hulls; uncertain arithmetic keeps exact fallback.
use super::*;

pub(in crate::surface_pullback::certificate) fn surfaces(
    a: &[H],
    b: &[H],
    degrees: [[usize; 2]; 2],
    limit: Real,
    coefficients: &mut Coefficients,
    budget: &mut Budget,
) -> Result<Outcome, GeometryError> {
    let available = FLOAT_WORK.min(budget.0);
    let mut work = Work {
        remaining: available,
        coefficients: std::mem::take(&mut coefficients.0),
    };
    let result = (|| {
        if !limit.is_normal() || limit < 0. {
            return None;
        }
        let a = Patch::new(a)?;
        let b = Patch::new(b)?;
        let degree = std::array::from_fn(|axis| degrees[0][axis] + degrees[1][axis]);
        hull(
            difference(&a.controls, &b.controls, degrees, &mut work)?,
            degree,
            limit,
            &mut work,
        )
    })();
    coefficients.0 = work.coefficients;
    budget.charge(available - work.remaining)?;
    Ok(result.unwrap_or(Outcome::Inconclusive))
}

fn difference(a: &[F], b: &[F], degrees: [[usize; 2]; 2], work: &mut Work) -> Option<Vec<F>> {
    let [[pu, pv], [qu, qv]] = degrees;
    let u = work.coefficients(pu, qu)?;
    let v = work.coefficients(pv, qv)?;
    let mut result = vec![[zero(); 4]; (pu + qu + 1) * (pv + qv + 1)];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            work.charge(4)?;
            let c = u[(i % (pu + 1)) * (qu + 1) + j % (qu + 1)]
                .mul(v[(i / (pu + 1)) * (qv + 1) + j / (qu + 1)])?;
            let index = (i / (pu + 1) + j / (qu + 1)) * (pu + qu + 1) + i % (pu + 1) + j % (qu + 1);
            for axis in 0..3 {
                let d = a[axis].mul(b[3])?.sub(b[axis].mul(a[3])?)?;
                result[index][axis] = result[index][axis].add(c.mul(d)?)?;
            }
            result[index][3] = result[index][3].add(c.mul(a[3])?.mul(b[3])?)?;
        }
    }
    Some(result)
}

fn hull(net: Vec<F>, degree: [usize; 2], limit: Real, work: &mut Work) -> Option<Outcome> {
    let mut pending = vec![(net, 0)];
    let mut maximum = 0_f64;
    while let Some((net, depth)) = pending.pop() {
        work.charge(4 * net.len())?;
        for i in [0, degree[0], degree[1] * (degree[0] + 1), net.len() - 1] {
            if norm_squared(&net[i], limit)?.lo > 1. {
                return Some(Outcome::Outside);
            }
        }
        let mut bound = 0_f64;
        let mut within = true;
        for point in &net {
            let square = norm_squared(point, limit)?;
            if square.hi > 1. {
                within = false;
                break;
            }
            bound = bound.max(upper(square.hi, limit));
        }
        if within {
            maximum = maximum.max(bound);
            continue;
        }
        if depth == FLOAT_DEPTH {
            return None;
        }
        let children = split(&net, degree, depth % 2, work)?;
        pending.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    Some(Outcome::Within(maximum))
}

fn split(net: &[F], degree: [usize; 2], axis: usize, work: &mut Work) -> Option<[Vec<F>; 2]> {
    work.charge(4 * net.len() * (degree[axis] + 1))?;
    let mut children = [net.to_vec(), net.to_vec()];
    let half = I::point(0.5)?;
    for fixed in 0..=degree[1 - axis] {
        let index = |i| {
            if axis == 0 {
                fixed * (degree[0] + 1) + i
            } else {
                i * (degree[0] + 1) + fixed
            }
        };
        let mut values = (0..=degree[axis])
            .map(|i| net[index(i)])
            .collect::<Vec<_>>();
        children[0][index(0)] = values[0];
        children[1][index(degree[axis])] = values[degree[axis]];
        for remaining in (1..=degree[axis]).rev() {
            for i in 0..remaining {
                for k in 0..4 {
                    values[i][k] = values[i][k].add(values[i + 1][k])?.mul(half)?;
                }
            }
            children[0][index(degree[axis] + 1 - remaining)] = values[0];
            children[1][index(remaining - 1)] = values[remaining - 1];
        }
    }
    Some(children)
}

#[cfg(test)]
mod tests;
