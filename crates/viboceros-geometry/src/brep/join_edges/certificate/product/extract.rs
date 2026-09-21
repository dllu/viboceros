//! Exact polar-form extraction on the union of normalized active knot spans.
use super::*;

pub(super) struct Spline<'a> {
    curve: &'a NurbsCurve,
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
        let domain = curve.domain();
        if interval
            .iter()
            .any(|t| !t.is_finite() || !domain.contains(t))
            || interval[0] == interval[1]
        {
            return Ok(None);
        }
        charge(
            curve
                .knots()
                .len()
                .saturating_add(curve.control_points().len()),
        )?;
        let negative = curve.control_points()[0].weight().is_sign_negative();
        if curve
            .control_points()
            .iter()
            .any(|c| c.weight() == 0. || c.weight().is_sign_negative() != negative)
        {
            return Ok(None);
        }
        let [start, end] = interval;
        let reversed = start > end;
        let start = rational(start);
        let length = rational(end) - &start;
        let knots = (0..curve.knots().len())
            .map(|i| {
                let i = if reversed {
                    curve.knots().len() - 1 - i
                } else {
                    i
                };
                (rational(curve.knots()[i]) - &start) / &length
            })
            .collect();
        Ok(Some(Self {
            curve,
            reversed,
            knots,
        }))
    }

    pub fn degree(&self) -> usize {
        self.curve.degree()
    }

    pub fn end_span(
        &self,
        end: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<(Vec<H>, Rational), GeometryError> {
        let mut spans = self.degree()..self.curve.control_points().len();
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
        let gauge = rational(self.curve.control_points()[0].weight().abs());
        let source = (span - p..=span)
            .map(|i| {
                let i = if self.reversed {
                    self.curve.control_points().len() - 1 - i
                } else {
                    i
                };
                let c = self.curve.control_points()[i];
                let weight = rational(c.weight().abs()) / &gauge;
                std::array::from_fn(|axis| {
                    if axis == 3 {
                        weight.clone()
                    } else {
                        rational(c.point().to_array()[axis]) * &weight
                    }
                })
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
