//! Positive homogeneous tensor hulls and exact de Casteljau subdivision.
use super::*;

pub(super) struct Patch {
    net: Vec<H>,
    degree: [usize; 2],
    pub(super) domain: [[Rational; 2]; 2],
    points: Vec<P>,
    bounds: [P; 2],
    variation: [Rational; 2],
    depth: usize,
}
impl Patch {
    pub(super) fn new(
        net: Vec<H>,
        degree: [usize; 2],
        domain: [[Rational; 2]; 2],
        depth: usize,
        budget: &mut Budget,
    ) -> Result<Self, GeometryError> {
        let mut points = Vec::with_capacity(net.len());
        for h in &net {
            if h[3] <= Rational::zero() {
                return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
            }
            let p: P = std::array::from_fn(|i| &h[i] / &h[3]);
            for x in &p {
                budget.check(x)?;
            }
            points.push(p);
        }
        let mut bounds = [points[0].clone(), points[0].clone()];
        for p in &points {
            for i in 0..3 {
                bounds[0][i] = bounds[0][i].clone().min(p[i].clone());
                bounds[1][i] = bounds[1][i].clone().max(p[i].clone());
            }
        }
        let mut variation = [Rational::zero(), Rational::zero()];
        let [pu, pv] = degree;
        for j in 0..=pv {
            for i in 0..=pu {
                for axis in 0..2 {
                    if (axis == 0 && i == pu) || (axis == 1 && j == pv) {
                        continue;
                    }
                    let a = &points[j * (pu + 1) + i];
                    let b = &points[j * (pu + 1) + i + if axis == 0 { 1 } else { pu + 1 }];
                    let width: Rational = (0..3)
                        .map(|k| {
                            let d = &a[k] - &b[k];
                            &d * &d
                        })
                        .sum();
                    budget.check(&width)?;
                    variation[axis] = variation[axis].clone().max(width);
                }
            }
        }
        Ok(Self {
            net,
            degree,
            domain,
            points,
            bounds,
            variation,
            depth,
        })
    }
    pub(super) fn constant(&self) -> bool {
        self.variation.iter().all(Zero::is_zero)
    }
    pub(super) fn lower(
        &self,
        target: &P,
        direction: Option<&P>,
        budget: &mut Budget,
    ) -> Result<Rational, GeometryError> {
        let mut lower: Rational = (0..3)
            .map(|i| {
                let gap = (&self.bounds[0][i] - &target[i])
                    .max(&target[i] - &self.bounds[1][i])
                    .max(Rational::zero());
                &gap * &gap
            })
            .sum();
        // A second basin can have a different closest-point normal from the
        // current best witness. A local box-center direction remains a valid
        // separating axis and sharpens that basin without a local-minimum claim.
        let local: P = std::array::from_fn(|i| {
            (&self.bounds[0][i] + &self.bounds[1][i]) / rational(2.) - &target[i]
        });
        for axis in direction.into_iter().chain([&local]) {
            let square: Rational = axis.iter().map(|x| x * x).sum();
            budget.check(&square)?;
            if !square.is_zero() {
                let mut extent: Option<[Rational; 2]> = None;
                for p in &self.points {
                    let value: Rational = (0..3).map(|i| (&p[i] - &target[i]) * &axis[i]).sum();
                    budget.check(&value)?;
                    extent = Some(match extent {
                        None => [value.clone(), value],
                        Some([lo, hi]) => [lo.min(value.clone()), hi.max(value)],
                    });
                }
                let [lo, hi] = extent.unwrap();
                let gap = lo.max(-hi).max(Rational::zero());
                lower = lower.max(&gap * &gap / square);
            }
        }
        budget.check(&lower)?;
        Ok(lower)
    }
    pub(super) fn children(&self, budget: &mut Budget) -> Result<[Self; 2], GeometryError> {
        if self.depth >= MAX_DEPTH {
            return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
        }
        let axis = usize::from(self.variation[1] > self.variation[0]);
        let [pu, pv] = self.degree;
        let mut nets = [self.net.clone(), self.net.clone()];
        if axis == 0 {
            for j in 0..=pv {
                let (a, b) = algebra::split(&self.net[j * (pu + 1)..(j + 1) * (pu + 1)], budget)?;
                nets[0][j * (pu + 1)..(j + 1) * (pu + 1)].clone_from_slice(&a);
                nets[1][j * (pu + 1)..(j + 1) * (pu + 1)].clone_from_slice(&b);
            }
        } else {
            for i in 0..=pu {
                let column = (0..=pv)
                    .map(|j| self.net[j * (pu + 1) + i].clone())
                    .collect::<Vec<_>>();
                let (a, b) = algebra::split(&column, budget)?;
                for j in 0..=pv {
                    nets[0][j * (pu + 1) + i] = a[j].clone();
                    nets[1][j * (pu + 1) + i] = b[j].clone();
                }
            }
        }
        let mut domains = [self.domain.clone(), self.domain.clone()];
        let middle = (&self.domain[axis][0] + &self.domain[axis][1]) / rational(2.);
        domains[0][axis][1] = middle.clone();
        domains[1][axis][0] = middle;
        let [a, b] = nets;
        let [da, db] = domains;
        Ok([
            Self::new(a, self.degree, da, self.depth + 1, budget)?,
            Self::new(b, self.degree, db, self.depth + 1, budget)?,
        ])
    }
}
