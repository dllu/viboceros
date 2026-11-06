//! Exact Bernstein exclusion for the independently known polynomial cubic.
use num_rational::BigRational as R;
use num_traits::{Signed, Zero};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct Certificate {
    pub excluded_radius: f64,
    pub leaves: usize,
    pub maximum_depth: usize,
    pub work: usize,
}
pub fn outside_cubic_ball(
    controls: [[f64; 3]; 4],
    point: [f64; 3],
    radius: f64,
) -> Option<Certificate> {
    if !radius.is_finite()
        || radius <= 0.
        || point
            .iter()
            .chain(controls.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return None;
    }
    let p = point.map(|x| R::from_float(x).unwrap());
    let q =
        controls.map(|c| std::array::from_fn::<_, 3, _>(|i| R::from_float(c[i]).unwrap() - &p[i]));
    let r = R::from_float(radius).unwrap();
    let squared = &r * &r;
    let bin3 = [1u32, 3, 3, 1];
    let bin6 = [1u32, 6, 15, 20, 15, 6, 1];
    let coefficients = (0..7)
        .map(|k| {
            let mut value = -squared.clone();
            for i in 0..4 {
                if k >= i && k - i < 4 {
                    let j = k - i;
                    let factor = R::new((bin3[i] * bin3[j]).into(), bin6[k].into());
                    let dot = (0..3).fold(R::zero(), |s, d| s + &q[i][d] * &q[j][d]);
                    value += factor * dot;
                }
            }
            value
        })
        .collect::<Vec<_>>();
    let mut stack = vec![(coefficients, 0usize)];
    let mut certificate = Certificate {
        excluded_radius: radius,
        leaves: 0,
        maximum_depth: 0,
        work: 0,
    };
    let two = R::from_integer(2.into());
    while let Some((mut row, depth)) = stack.pop() {
        certificate.work += 1;
        if certificate.work > 4096 || depth > 32 {
            return None;
        }
        if row.iter().all(Signed::is_positive) {
            certificate.leaves += 1;
            certificate.maximum_depth = certificate.maximum_depth.max(depth);
            continue;
        }
        let mut left = vec![row[0].clone()];
        let mut right = vec![row.last().unwrap().clone()];
        while row.len() > 1 {
            row = row.windows(2).map(|p| (&p[0] + &p[1]) / &two).collect();
            left.push(row[0].clone());
            right.push(row.last().unwrap().clone());
        }
        right.reverse();
        stack.push((left, depth + 1));
        stack.push((right, depth + 1));
    }
    Some(certificate)
}
