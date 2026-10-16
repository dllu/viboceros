//! Exact isocurve restrictions and rational parameter changes.
use super::*;

/// Keep only the most recently requested coordinate restriction. Arbitrary
/// repeated queries cannot accumulate one cache per parameter value.
pub(super) struct Isocurve {
    fixed: usize,
    coordinate: Rational,
    span: usize,
    controls: BTreeMap<usize, H>,
    patches: BTreeMap<usize, Vec<H>>,
}

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
    /// Collapse a coordinate only when every homogeneous UV control proves it
    /// constant. The reduced net is exact data from the stored tensor, not a
    /// rounded public isocurve. Its discarded weight factor is positive.
    pub(super) fn isocurve_patch(
        &mut self,
        span: [usize; 2],
        uv: &[Uv],
        budget: &mut Budget,
    ) -> Result<Option<(Vec<H>, usize)>, GeometryError> {
        let mut fixed = None;
        for axis in 0..2 {
            budget.charge(uv.len())?;
            let station = &uv[0][axis] / &uv[0][2];
            let mut constant = true;
            for h in uv {
                let coordinate = &h[axis] / &h[2];
                budget.check(&coordinate)?;
                constant &= coordinate == station;
            }
            if constant {
                fixed = Some(axis);
                break;
            }
        }
        let Some(fixed) = fixed else {
            return Ok(None);
        };
        let coordinate = &uv[0][fixed] / &uv[0][2];
        Ok(Some((
            self.restricted_isocurve(span, fixed, &coordinate, budget)?,
            1 - fixed,
        )))
    }

    /// Evaluate only the needed original B-spline columns in the fixed
    /// direction, then extract the variable-direction Bernstein span. This
    /// avoids building a two-dimensional patch before reducing it again.
    fn restricted_isocurve(
        &mut self,
        span: [usize; 2],
        fixed: usize,
        coordinate: &Rational,
        budget: &mut Budget,
    ) -> Result<Vec<H>, GeometryError> {
        if !self
            .isocurve
            .as_ref()
            .is_some_and(|c| c.fixed == fixed && c.coordinate == *coordinate)
        {
            budget.charge(self.spans[fixed].len())?;
            // Admitted surfaces have no full-order internal discontinuities;
            // either incident span has the same exact value at a shared knot.
            let fixed_span = self.spans[fixed]
                .iter()
                .copied()
                .find(|&s| {
                    *coordinate >= self.knots[fixed][s] && *coordinate <= self.knots[fixed][s + 1]
                })
                .ok_or(GeometryError::SurfaceCurveCertificateWorkLimit)?;
            self.isocurve = Some(Isocurve {
                fixed,
                coordinate: coordinate.clone(),
                span: fixed_span,
                controls: BTreeMap::new(),
                patches: BTreeMap::new(),
            });
        }
        let iso = self.isocurve.as_mut().unwrap();
        let variable = 1 - fixed;
        let variable_span = span[variable];
        if let Some(net) = iso.patches.get(&variable_span) {
            budget.charge(4 * net.len())?;
            return Ok(net.clone());
        }
        let mut controls = Vec::with_capacity(self.degree[variable] + 1);
        for i in variable_span - self.degree[variable]..=variable_span {
            if let Some(h) = iso.controls.get(&i) {
                budget.charge(4)?;
                controls.push(h.clone());
                continue;
            }
            let section = (iso.span - self.degree[fixed]..=iso.span)
                .map(|j| {
                    self.controls[if fixed == 1 {
                        j * self.counts[0] + i
                    } else {
                        i * self.counts[0] + j
                    }]
                    .clone()
                })
                .collect::<Vec<_>>();
            let h = curve::extract(
                &self.knots[fixed],
                self.degree[fixed],
                iso.span,
                &section,
                coordinate,
                coordinate,
                budget,
            )?[0]
                .clone();
            iso.controls.insert(i, h.clone());
            controls.push(h);
        }
        let net = curve::extract(
            &self.knots[variable],
            self.degree[variable],
            variable_span,
            &controls,
            &self.knots[variable][variable_span],
            &self.knots[variable][variable_span + 1],
            budget,
        )?;
        iso.patches.insert(variable_span, net.clone());
        Ok(net)
    }

    pub(super) fn box_patch(
        &mut self,
        span: [usize; 2],
        bounds: &[[Rational; 2]],
        budget: &mut Budget,
    ) -> Result<(Vec<H>, [usize; 2]), GeometryError> {
        let mut degree = self.degree;
        if let Some(fixed) = (0..2).find(|&axis| bounds[axis][0] == bounds[axis][1]) {
            degree[fixed] = 0;
            Ok((
                self.restricted_isocurve(span, fixed, &bounds[fixed][0], budget)?,
                degree,
            ))
        } else {
            Ok((self.patch(span, budget)?, degree))
        }
    }

    pub(super) fn isocurve_image(
        &mut self,
        span: [usize; 2],
        uv: &[Uv],
        budget: &mut Budget,
    ) -> Result<Option<Vec<H>>, GeometryError> {
        let Some((iso, variable)) = self.isocurve_patch(span, uv, budget)? else {
            return Ok(None);
        };
        let low = &self.knots[variable][span[variable]];
        let width = &self.knots[variable][span[variable] + 1] - low;
        if uv.len() > 2 {
            let a = uv
                .iter()
                .map(|h| (&h[variable] - low * &h[2]) / &width)
                .collect::<Vec<_>>();
            let b = uv
                .iter()
                .zip(&a)
                .map(|(h, a)| &h[2] - a)
                .collect::<Vec<_>>();
            let bases = algebra::basis(&a, &b, self.degree[variable], budget)?;
            let count = (uv.len() - 1) * self.degree[variable] + 1;
            let mut image = vec![std::array::from_fn(|_| Rational::zero()); count];
            for (h, basis) in iso.iter().zip(bases) {
                budget.charge(4 * count)?;
                for (control, coefficient) in image.iter_mut().zip(basis) {
                    for axis in 0..4 {
                        control[axis] += &h[axis] * &coefficient;
                        budget.check(&control[axis])?;
                    }
                }
            }
            return Ok(Some(image));
        }
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
