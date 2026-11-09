//! Exact nonnegative scalar tensor hulls, including attained patch corners.
use super::*;

pub(super) fn nonnegative(
    net: Vec<Rational>,
    degree: [usize; 2],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    let mut pending = vec![(net, 0)];
    let [pu, pv] = degree;
    while let Some((net, depth)) = pending.pop() {
        budget.charge(net.len())?;
        if [0, pu, pv * (pu + 1), net.len() - 1]
            .into_iter()
            .any(|i| net[i] < Rational::zero())
        {
            return Ok(false);
        }
        if net.iter().all(|p| *p >= Rational::zero()) {
            continue;
        }
        if depth == MAX_DEPTH {
            return Ok(false);
        }
        let varies_u = net
            .chunks_exact(pu + 1)
            .any(|row| row.windows(2).any(|w| w[0] != w[1]));
        let varies_v = (0..pu + 1)
            .any(|i| (0..pv).any(|j| net[j * (pu + 1) + i] != net[(j + 1) * (pu + 1) + i]));
        let axis = if !varies_u {
            1
        } else if !varies_v {
            0
        } else {
            depth % 2
        };
        let mut children = [net.clone(), net.clone()];
        let lines = if axis == 0 { pv + 1 } else { pu + 1 };
        let count = degree[axis] + 1;
        for fixed in 0..lines {
            let index = |i| {
                if axis == 0 {
                    fixed * (pu + 1) + i
                } else {
                    i * (pu + 1) + fixed
                }
            };
            let values = (0..count)
                .map(|i| [net[index(i)].clone()])
                .collect::<Vec<_>>();
            let (a, b) = algebra::split(&values, budget)?;
            for (child, values) in children.iter_mut().zip([a, b]) {
                for (i, value) in values.into_iter().enumerate() {
                    child[index(i)] = value[0].clone();
                }
            }
        }
        pending.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    Ok(true)
}
