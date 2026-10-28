//! Squared-distance Bernstein coefficients retain information lost by convex hulls.
use super::*;

struct Moments {
    weight: Vec<Rational>,
    norm: Vec<Rational>,
    weighted: [Vec<Rational>; 3],
}

impl Moments {
    fn new(net: &[H], budget: &mut Budget) -> Result<Self, GeometryError> {
        let weight = net.iter().map(|p| p[3].clone()).collect::<Vec<_>>();
        let mut norm = vec![Rational::zero(); 2 * net.len() - 1];
        let mut weighted = Vec::with_capacity(3);
        for axis in 0..3 {
            let coordinate = net.iter().map(|p| p[axis].clone()).collect::<Vec<_>>();
            for (sum, square) in
                norm.iter_mut()
                    .zip(algebra::multiply(&coordinate, &coordinate, budget)?)
            {
                *sum += square;
                budget.check(sum)?;
            }
            weighted.push(algebra::multiply(&coordinate, &weight, budget)?);
        }
        Ok(Self {
            weight: algebra::multiply(&weight, &weight, budget)?,
            norm,
            weighted: weighted.try_into().unwrap(),
        })
    }
}

pub(super) fn lower(hulls: &[Rc<Hull>; 2], budget: &mut Budget) -> Result<Rational, GeometryError> {
    let (Some(a), Some(b)) = (&hulls[0].net, &hulls[1].net) else {
        return Ok(Rational::zero());
    };
    let (a, b) = (Moments::new(a, budget)?, Moments::new(b, budget)?);
    let mut lower: Option<Rational> = None;
    for i in 0..a.weight.len() {
        for j in 0..b.weight.len() {
            // For independent curve parameters, the squared numerator is
            // |A|² Wb² + Wa² |B|² - 2 (A Wa) dot (B Wb). These products
            // are outer products of the existing exact univariate algebra.
            let cross: Rational = (0..3).map(|k| &a.weighted[k][i] * &b.weighted[k][j]).sum();
            let numerator =
                &a.norm[i] * &b.weight[j] + &a.weight[i] * &b.norm[j] - rational(2.) * cross;
            let denominator = &a.weight[i] * &b.weight[j];
            budget.check(&numerator)?;
            budget.check(&denominator)?;
            // Extracted common-sign weights give strictly positive squared
            // denominator coefficients. N - min(N_i/D_i) D has nonnegative
            // Bernstein coefficients on the complete parameter rectangle.
            let ratio = numerator / denominator;
            budget.check(&ratio)?;
            lower = Some(lower.map_or_else(|| ratio.clone(), |v| v.min(ratio.clone())));
        }
    }
    Ok(lower.unwrap().max(Rational::zero()))
}
