//! Exact one-dimensional restrictions and rational linear parameter changes.
use super::*;

fn at(net: Vec<H>, t: &Rational, budget: &mut Budget) -> Result<H, GeometryError> {
    if t.is_zero() || *t == Rational::one() {
        budget.charge(4 * net.len())?;
        return Ok(if t.is_zero() {
            net[0].clone()
        } else {
            net.last().unwrap().clone()
        });
    }
    budget.charge(4 * net.len().pow(2))?;
    let mut work = net;
    let complement = Rational::one() - t;
    for remaining in (1..work.len()).rev() {
        for i in 0..remaining {
            work[i] =
                std::array::from_fn(|axis| &complement * &work[i][axis] + t * &work[i + 1][axis]);
            for r in &work[i] {
                budget.check(r)?;
            }
        }
    }
    Ok(work[0].clone())
}

impl Surface {
    pub(super) fn linear_image(
        &mut self,
        span: [usize; 2],
        uv: &[Uv],
        budget: &mut Budget,
    ) -> Result<Option<Vec<H>>, GeometryError> {
        if uv.len() != 2 {
            return Ok(None);
        }
        let Some(fixed) = (0..2).find(|&axis| &uv[0][axis] * &uv[1][2] == &uv[1][axis] * &uv[0][2])
        else {
            return Ok(None);
        };
        let variable = 1 - fixed;
        let low = &self.knots[fixed][span[fixed]];
        let width = &self.knots[fixed][span[fixed] + 1] - low;
        let station = (&uv[0][fixed] / &uv[0][2] - low) / width;
        budget.check(&station)?;
        let net = self.patch(span, budget)?;
        let columns = self.degree[0] + 1;
        let mut iso = Vec::with_capacity(self.degree[variable] + 1);
        for i in 0..=self.degree[variable] {
            let section = (0..=self.degree[fixed])
                .map(|j| {
                    net[if fixed == 1 {
                        j * columns + i
                    } else {
                        i * columns + j
                    }]
                    .clone()
                })
                .collect();
            iso.push(at(section, &station, budget)?);
        }
        let low = &self.knots[variable][span[variable]];
        let width = &self.knots[variable][span[variable] + 1] - low;
        let start = (&uv[0][variable] / &uv[0][2] - low) / &width;
        let end = (&uv[1][variable] / &uv[1][2] - low) / &width;
        budget.check(&start)?;
        budget.check(&end)?;
        if start == end {
            return Ok(Some(vec![at(iso, &start, budget)?]));
        }
        let degree = self.degree[variable];
        let knots = [
            vec![Rational::zero(); degree + 1],
            vec![Rational::one(); degree + 1],
        ]
        .concat();
        let mut image = curve::extract(
            &knots,
            degree,
            degree,
            &iso,
            &start.clone().min(end.clone()),
            &start.clone().max(end.clone()),
            budget,
        )?;
        if start > end {
            image.reverse();
        }
        // Mapping the restricted Bernstein curve through
        // s(t)=w1*t/(w0*(1-t)+w1*t) scales control i by (w1/w0)^i.
        // The discarded common homogeneous factor is nonzero on the segment.
        // Equal weights need no multiplication or elevated tensor products.
        let ratio = &uv[1][2] / &uv[0][2];
        budget.check(&ratio)?;
        if ratio != Rational::one() {
            let mut factor = Rational::one();
            for control in &mut image {
                for r in control {
                    *r *= &factor;
                    budget.check(r)?;
                }
                factor *= &ratio;
                budget.check(&factor)?;
            }
        }
        Ok(Some(image))
    }
}
