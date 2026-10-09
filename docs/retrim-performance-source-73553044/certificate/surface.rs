//! Exact tensor patches and images, including control hulls at knot crossings.
use super::*;
mod linear;

pub(super) struct Surface {
    degree: [usize; 2],
    counts: [usize; 2],
    knots: [Vec<Rational>; 2],
    spans: [Vec<usize>; 2],
    controls: Vec<H>,
    patches: BTreeMap<[usize; 2], Vec<H>>,
    interval_coefficients: interval::Coefficients,
}
impl Surface {
    pub fn new(surface: &NurbsSurface, budget: &mut Budget) -> Result<Option<Self>, GeometryError> {
        budget.charge(
            surface.control_points().len() + surface.knots_u().len() + surface.knots_v().len(),
        )?;
        let degree = [surface.degree_u(), surface.degree_v()];
        let counts = [
            surface.control_point_count_u(),
            surface.control_point_count_v(),
        ];
        let knots = [surface.knots_u(), surface.knots_v()];
        if (0..2).any(|axis| {
            knots[axis].chunk_by(|a, b| a == b).any(|group| {
                group[0] > knots[axis][degree[axis]]
                    && group[0] < knots[axis][counts[axis]]
                    && group.len() > degree[axis]
            })
        }) {
            return Ok(None);
        }
        let gauge = surface.control_points()[0].weight();
        if surface
            .control_points()
            .iter()
            .any(|c| c.weight() == 0. || c.weight().is_sign_negative() != gauge.is_sign_negative())
        {
            return Ok(None);
        }
        let spans = std::array::from_fn(|axis| {
            (degree[axis]..counts[axis])
                .filter(|&i| knots[axis][i] < knots[axis][i + 1])
                .collect()
        });
        let knots = knots.map(|knots| knots.iter().map(|k| rational(*k)).collect());
        let controls = surface
            .control_points()
            .iter()
            .map(|c| {
                let w = rational(c.weight()) / rational(gauge);
                std::array::from_fn(|i| {
                    if i == 3 {
                        w.clone()
                    } else {
                        rational(c.point().to_array()[i]) * &w
                    }
                })
            })
            .collect();
        Ok(Some(Self {
            degree,
            counts,
            knots,
            spans,
            controls,
            patches: BTreeMap::new(),
            interval_coefficients: interval::Coefficients::default(),
        }))
    }
    pub fn in_domain(&self, bounds: &[[Rational; 2]]) -> bool {
        (0..2).all(|axis| {
            bounds[axis][0] >= self.knots[axis][self.degree[axis]]
                && bounds[axis][1] <= self.knots[axis][self.counts[axis]]
        })
    }
    pub fn endpoints_in_domain(&self, uv: &[Uv]) -> bool {
        [uv.first().unwrap(), uv.last().unwrap()]
            .into_iter()
            .all(|h| {
                (0..2).all(|axis| {
                    let r = &h[axis] / &h[2];
                    r >= self.knots[axis][self.degree[axis]]
                        && r <= self.knots[axis][self.counts[axis]]
                })
            })
    }

    /// Exact parameter fractions where a rational linear UV segment crosses
    /// an internal tensor knot. Homogeneous coordinates make each equation
    /// linear even when the two endpoint weights differ.
    pub fn linear_crossings(
        &self,
        uv: &[Uv],
        budget: &mut Budget,
    ) -> Result<Vec<Rational>, GeometryError> {
        if uv.len() != 2 {
            return Ok(Vec::new());
        }
        let mut cuts = Vec::new();
        for axis in 0..2 {
            for &span in self.spans[axis].iter().skip(1) {
                budget.charge(1)?;
                let knot = &self.knots[axis][span];
                let a = &uv[0][axis] - knot * &uv[0][2];
                let b = &uv[1][axis] - knot * &uv[1][2];
                let difference = &a - &b;
                budget.check(&a)?;
                budget.check(&b)?;
                budget.check(&difference)?;
                if difference.is_zero() {
                    continue;
                }
                let t = a / difference;
                budget.check(&t)?;
                if t > Rational::zero() && t < Rational::one() {
                    cuts.push(t);
                }
            }
        }
        Ok(cuts)
    }
    pub fn containing_patch(
        &self,
        bounds: &[[Rational; 2]],
        budget: &mut Budget,
    ) -> Result<Option<[usize; 2]>, GeometryError> {
        budget.charge(self.spans[0].len() + self.spans[1].len())?;
        let mut patch = [0; 2];
        for axis in 0..2 {
            let Some(span) = self.spans[axis].iter().copied().find(|&span| {
                bounds[axis][0] >= self.knots[axis][span]
                    && bounds[axis][1] <= self.knots[axis][span + 1]
            }) else {
                return Ok(None);
            };
            patch[axis] = span;
        }
        Ok(Some(patch))
    }
    fn patch(&mut self, span: [usize; 2], budget: &mut Budget) -> Result<Vec<H>, GeometryError> {
        if let Some(net) = self.patches.get(&span) {
            budget.charge(4 * net.len())?;
            return Ok(net.clone());
        }
        let [pu, pv] = self.degree;
        let [su, sv] = span;
        let mut rows = Vec::with_capacity(pv + 1);
        for j in sv - pv..=sv {
            let row = (su - pu..=su)
                .map(|i| self.controls[j * self.counts[0] + i].clone())
                .collect::<Vec<_>>();
            rows.push(curve::extract(
                &self.knots[0],
                pu,
                su,
                &row,
                &self.knots[0][su],
                &self.knots[0][su + 1],
                budget,
            )?);
        }
        let mut net = vec![std::array::from_fn(|_| Rational::zero()); (pu + 1) * (pv + 1)];
        for i in 0..=pu {
            let column = rows.iter().map(|row| row[i].clone()).collect::<Vec<_>>();
            let column = curve::extract(
                &self.knots[1],
                pv,
                sv,
                &column,
                &self.knots[1][sv],
                &self.knots[1][sv + 1],
                budget,
            )?;
            for (j, h) in column.into_iter().enumerate() {
                net[j * (pu + 1) + i] = h;
            }
        }
        self.patches.insert(span, net.clone());
        Ok(net)
    }
    pub fn compose(
        &mut self,
        span: [usize; 2],
        uv: &[Uv],
        budget: &mut Budget,
    ) -> Result<Vec<H>, GeometryError> {
        if let Some(image) = self.linear_image(span, uv, budget)? {
            return Ok(image);
        }
        let net = self.patch(span, budget)?;
        let mut bases = Vec::new();
        for axis in 0..2 {
            let low = &self.knots[axis][span[axis]];
            let width = &self.knots[axis][span[axis] + 1] - low;
            let a = uv
                .iter()
                .map(|h| (&h[axis] - low * &h[2]) / &width)
                .collect::<Vec<_>>();
            let b = uv
                .iter()
                .zip(&a)
                .map(|(h, a)| &h[2] - a)
                .collect::<Vec<_>>();
            bases.push(algebra::basis(&a, &b, self.degree[axis], budget)?);
        }
        let count = (uv.len() - 1) * (self.degree[0] + self.degree[1]) + 1;
        let mut image = vec![std::array::from_fn(|_| Rational::zero()); count];
        for (j, v) in bases[1].iter().enumerate() {
            for (i, u) in bases[0].iter().enumerate() {
                let basis = algebra::multiply(u, v, budget)?;
                let h = &net[j * (self.degree[0] + 1) + i];
                budget.charge(4 * count)?;
                for (k, b) in basis.iter().enumerate() {
                    for axis in 0..4 {
                        image[k][axis] += &h[axis] * b;
                        budget.check(&image[k][axis])?;
                    }
                }
            }
        }
        Ok(image)
    }
    pub fn interval_bound(
        &mut self,
        span: [usize; 2],
        uv: &[Uv],
        spatial: &[H],
        limit: Real,
        budget: &mut Budget,
    ) -> Result<interval::Outcome, GeometryError> {
        let net = self.patch(span, budget)?;
        let bounds = std::array::from_fn(|axis| {
            [
                self.knots[axis][span[axis]].clone(),
                &self.knots[axis][span[axis] + 1] - &self.knots[axis][span[axis]],
            ]
        });
        Ok(interval::bound_with_cache(
            &net,
            self.degree,
            &bounds,
            uv,
            spatial,
            limit,
            &mut self.interval_coefficients,
        ))
    }
    pub fn crossing_bound(
        &mut self,
        bounds: &[[Rational; 2]],
        spatial: &[H],
        limit: Real,
        budget: &mut Budget,
    ) -> Result<Option<Real>, GeometryError> {
        budget.charge(self.spans[0].len() + self.spans[1].len())?;
        let spans: [Vec<usize>; 2] = std::array::from_fn(|axis| {
            self.spans[axis]
                .iter()
                .copied()
                .filter(|&span| {
                    bounds[axis][0] <= self.knots[axis][span + 1]
                        && bounds[axis][1] >= self.knots[axis][span]
                })
                .collect()
        });
        if spans[0].len().saturating_mul(spans[1].len()) > 64 {
            return Ok(None);
        }
        let mut image_bounds: Option<Vec<[Rational; 2]>> = None;
        for &sv in &spans[1] {
            for &su in &spans[0] {
                let span = [su, sv];
                let mut intervals = Vec::new();
                for axis in 0..2 {
                    let low = &self.knots[axis][span[axis]];
                    let high = &self.knots[axis][span[axis] + 1];
                    let a = bounds[axis][0].clone().max(low.clone());
                    let b = bounds[axis][1].clone().min(high.clone());
                    intervals.push([(&a - low) / (high - low), (&b - low) / (high - low)]);
                }
                let net = self.patch(span, budget)?;
                let net = restrict(&net, self.degree, &intervals, budget)?;
                let b = curve::bounds(&net, budget)?;
                if let Some(current) = &mut image_bounds {
                    for axis in 0..3 {
                        current[axis][0] = current[axis][0].clone().min(b[axis][0].clone());
                        current[axis][1] = current[axis][1].clone().max(b[axis][1].clone());
                    }
                } else {
                    image_bounds = Some(b);
                }
            }
        }
        let Some(image) = image_bounds else {
            return Ok(None);
        };
        let spatial = curve::bounds(spatial, budget)?;
        let difference: H = std::array::from_fn(|axis| {
            if axis == 3 {
                return Rational::one();
            }
            let a = &image[axis][1] - &spatial[axis][0];
            let b = &spatial[axis][1] - &image[axis][0];
            a.max(b)
        });
        Ok(algebra::norm_bound(&difference, limit))
    }
    pub fn interval_crossing_bound(
        &mut self,
        bounds: &[[Rational; 2]],
        spatial: &[H],
        limit: Real,
        budget: &mut Budget,
    ) -> Result<interval::Crossing, GeometryError> {
        let spans: [Vec<usize>; 2] = std::array::from_fn(|axis| {
            self.spans[axis]
                .iter()
                .copied()
                .filter(|&span| {
                    bounds[axis][0] <= self.knots[axis][span + 1]
                        && bounds[axis][1] >= self.knots[axis][span]
                })
                .collect()
        });
        if spans[0].len().saturating_mul(spans[1].len()) > 64 {
            return Ok(interval::Crossing::Refine);
        }
        let mut boxes = interval::Boxes::new();
        for &sv in &spans[1] {
            for &su in &spans[0] {
                let span = [su, sv];
                let mut intervals = Vec::new();
                for axis in 0..2 {
                    let low = &self.knots[axis][span[axis]];
                    let high = &self.knots[axis][span[axis] + 1];
                    let a = bounds[axis][0].clone().max(low.clone());
                    let b = bounds[axis][1].clone().min(high.clone());
                    intervals.push([(&a - low) / (high - low), (&b - low) / (high - low)]);
                }
                let net = self.patch(span, budget)?;
                if boxes.add(&net, self.degree, &intervals).is_none() {
                    return Ok(interval::Crossing::Inconclusive);
                }
            }
        }
        Ok(boxes.finish(spatial, limit))
    }
}

fn restrict(
    net: &[H],
    degree: [usize; 2],
    intervals: &[[Rational; 2]],
    budget: &mut Budget,
) -> Result<Vec<H>, GeometryError> {
    let [pu, pv] = degree;
    let uknots = [
        vec![Rational::zero(); pu + 1],
        vec![Rational::one(); pu + 1],
    ]
    .concat();
    let vknots = [
        vec![Rational::zero(); pv + 1],
        vec![Rational::one(); pv + 1],
    ]
    .concat();
    let mut rows = Vec::new();
    for row in net.chunks(pu + 1) {
        rows.push(curve::extract(
            &uknots,
            pu,
            pu,
            row,
            &intervals[0][0],
            &intervals[0][1],
            budget,
        )?);
    }
    let mut output = net.to_vec();
    for i in 0..=pu {
        let column = rows.iter().map(|row| row[i].clone()).collect::<Vec<_>>();
        let column = curve::extract(
            &vknots,
            pv,
            pv,
            &column,
            &intervals[1][0],
            &intervals[1][1],
            budget,
        )?;
        for (j, h) in column.into_iter().enumerate() {
            output[j * (pu + 1) + i] = h;
        }
    }
    Ok(output)
}
