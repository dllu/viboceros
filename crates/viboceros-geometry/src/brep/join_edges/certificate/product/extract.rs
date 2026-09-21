//! Exact polar-form extraction on the union of normalized active knot spans.
use super::*;

#[derive(Clone, Copy)]
enum Controls<'a> {
    Euclidean(&'a [WeightedPoint3]),
    Homogeneous(&'a [H]),
}

impl Controls<'_> {
    fn len(self) -> usize {
        match self {
            Self::Euclidean(c) => c.len(),
            Self::Homogeneous(c) => c.len(),
        }
    }

    fn positive_gauge(self) -> Option<Rational> {
        match self {
            Self::Euclidean(c) => {
                let sign = c[0].weight().is_sign_negative();
                c.iter()
                    .all(|p| p.weight() != 0. && p.weight().is_sign_negative() == sign)
                    .then(|| rational(c[0].weight()))
            }
            Self::Homogeneous(c) => {
                let zero = rational(0.);
                let sign = c[0][3] < zero;
                c.iter()
                    .all(|p| p[3] != zero && (p[3] < zero) == sign)
                    .then(|| c[0][3].clone())
            }
        }
    }

    fn normalized(self, i: usize, gauge: &Rational) -> H {
        match self {
            Self::Euclidean(c) => {
                let weight = rational(c[i].weight()) / gauge;
                std::array::from_fn(|axis| {
                    if axis == 3 {
                        weight.clone()
                    } else {
                        rational(c[i].point().to_array()[axis]) * &weight
                    }
                })
            }
            Self::Homogeneous(c) => std::array::from_fn(|axis| &c[i][axis] / gauge),
        }
    }
}

pub(super) struct Spline<'a> {
    controls: Controls<'a>,
    degree: usize,
    gauge: Rational,
    reversed: bool,
    pub knots: Vec<Rational>,
}

impl<'a> Spline<'a> {
    pub fn new(
        curve: &'a NurbsCurve,
        reversed: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Self>, GeometryError> {
        let domain = curve.domain();
        let mut interval = [*domain.start(), *domain.end()];
        if reversed {
            interval.reverse();
        }
        Self::restricted(curve, interval, charge)
    }

    /// Normalize an oriented subinterval exactly, retaining the original
    /// controls. Knots outside the restriction can lie outside [0,1].
    pub fn restricted(
        curve: &'a NurbsCurve,
        interval: [Real; 2],
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Self>, GeometryError> {
        Self::from_controls(
            Controls::Euclidean(curve.control_points()),
            curve.degree(),
            curve.knots(),
            interval,
            charge,
        )
    }

    pub fn homogeneous(
        controls: &'a [H],
        degree: usize,
        knots: &[Real],
        interval: [Real; 2],
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Self>, GeometryError> {
        Self::from_controls(
            Controls::Homogeneous(controls),
            degree,
            knots,
            interval,
            charge,
        )
    }

    fn from_controls(
        controls: Controls<'a>,
        degree: usize,
        knots: &[Real],
        interval: [Real; 2],
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Self>, GeometryError> {
        let domain = knots[degree]..=knots[controls.len()];
        if interval
            .iter()
            .any(|t| !t.is_finite() || !domain.contains(t))
            || interval[0] == interval[1]
        {
            return Ok(None);
        }
        charge(knots.len().saturating_add(controls.len()))?;
        let Some(gauge) = controls.positive_gauge() else {
            return Ok(None);
        };
        let [start, end] = interval;
        let reversed = start > end;
        let start = rational(start);
        let length = rational(end) - &start;
        let knots = (0..knots.len())
            .map(|i| {
                let i = if reversed { knots.len() - 1 - i } else { i };
                (rational(knots[i]) - &start) / &length
            })
            .collect();
        Ok(Some(Self {
            controls,
            degree,
            gauge,
            reversed,
            knots,
        }))
    }

    pub fn degree(&self) -> usize {
        self.degree
    }

    pub fn end_span(
        &self,
        end: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<(Vec<H>, Rational), GeometryError> {
        let mut spans = self.degree()..self.controls.len();
        let (zero, one) = (rational(0.), rational(1.));
        let nonempty = |&i: &usize| {
            self.knots[i] < self.knots[i + 1] && self.knots[i] < one && self.knots[i + 1] > zero
        };
        let span = if end {
            spans.rev().find(nonempty)
        } else {
            spans.find(nonempty)
        }
        .expect("validated nonempty active domain");
        let (left, right) = (
            std::cmp::max(&self.knots[span], &zero),
            std::cmp::min(&self.knots[span + 1], &one),
        );
        Ok((self.extract(span, left, right, charge)?, right - left))
    }

    pub fn extract(
        &self,
        span: usize,
        left: &Rational,
        right: &Rational,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Vec<H>, GeometryError> {
        let p = self.degree();
        let isolated = self.knots[span - p..=span].iter().all(|k| k == left)
            && self.knots[span + 1..=span + p + 1]
                .iter()
                .all(|k| k == right);
        charge(if isolated { p + 1 } else { 4 * (p + 1).pow(3) })?;
        // Exact normalization eliminates arbitrary common weight gauges,
        // including negative, subnormal and huge finite gauges.
        let source = (span - p..=span)
            .map(|i| {
                let i = if self.reversed {
                    self.controls.len() - 1 - i
                } else {
                    i
                };
                self.controls.normalized(i, &self.gauge)
            })
            .collect::<Vec<H>>();
        if isolated {
            return Ok(source);
        }
        let mut output = Vec::with_capacity(p + 1);
        for right_arguments in 0..=p {
            let mut work = source.clone();
            for level in 1..=p {
                let t = if level <= p - right_arguments {
                    left
                } else {
                    right
                };
                for j in (level..=p).rev() {
                    let k = span - p + j;
                    let alpha =
                        (t - &self.knots[k]) / (&self.knots[k + p - level + 1] - &self.knots[k]);
                    let beta = rational(1.) - &alpha;
                    work[j] = std::array::from_fn(|axis| {
                        &beta * &work[j - 1][axis] + &alpha * &work[j][axis]
                    });
                }
            }
            output.push(work[p].clone());
        }
        Ok(output)
    }
}
