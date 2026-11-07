//! Bounded curve location in a caller-supplied two-dimensional projection.
use crate::{NurbsCurve, Point3, Real};
const MAX_SPANS: usize = 4096;
const SAMPLES_PER_SPAN: usize = 16;
const REFINE_INTERVALS: usize = 8;
#[derive(Clone, Copy)]
struct Interval {
    score: Real,
    start: Real,
    end: Real,
}
impl Interval {
    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .total_cmp(&other.score)
            .then(self.start.total_cmp(&other.start))
            .then(self.end.total_cmp(&other.end))
    }
}
impl NurbsCurve {
    /// Locate the closest projected point in the bounded native domain.
    /// The callback returns finite two-dimensional coordinates or `None` for an
    /// invisible point. Distances use that coordinate system, not world units.
    /// This numerical query uses 16 sampling intervals per span and refines the
    /// eight best neighborhoods; it is not a certified global minimum. More than
    /// 4096 spans, nonfinite targets and wholly invisible curves return `None`.
    pub fn closest_parameter_in_projection(
        &self,
        target: [Real; 2],
        mut project: impl FnMut(Point3) -> Option<[Real; 2]>,
    ) -> Option<Real> {
        if target.iter().any(|x| !x.is_finite())
            || self.spans().take(MAX_SPANS + 1).count() > MAX_SPANS
        {
            return None;
        }
        let mut score = |t| -> Option<Real> {
            let [x, y] = project(self.evaluate(t).ok()?)?;
            let d = (x - target[0]).hypot(y - target[1]);
            (x.is_finite() && y.is_finite() && d.is_finite()).then_some(d)
        };
        let mut intervals: Vec<Interval> = Vec::with_capacity(REFINE_INTERVALS + 1);
        let mut best: Option<(Real, Real)> = None;
        for (start, end) in self.spans() {
            let mut previous = None;
            for i in 0..=SAMPLES_PER_SPAN {
                let f = i as Real / SAMPLES_PER_SPAN as Real;
                let t = start * (1. - f) + end * f;
                if let Some(d) = score(t) {
                    if best.is_none_or(|(old, _)| d < old) {
                        best = Some((d, t));
                    }
                    if let Some((a, da)) = previous {
                        let interval = Interval {
                            score: Real::min(d, da),
                            start: a,
                            end: t,
                        };
                        let position =
                            intervals.partition_point(|old| old.compare(&interval).is_le());
                        if position < REFINE_INTERVALS {
                            intervals.insert(position, interval);
                            if intervals.len() > REFINE_INTERVALS {
                                intervals.pop();
                            }
                        }
                    }
                    previous = Some((t, d));
                } else {
                    previous = None;
                }
            }
        }
        for Interval {
            start: mut a,
            end: mut b,
            ..
        } in intervals
        {
            const F: Real = 0.381_966_011_250_105_1;
            let mut left = a * (1. - F) + b * F;
            let mut right = a * F + b * (1. - F);
            let mut dl = score(left).unwrap_or(Real::INFINITY);
            let mut dr = score(right).unwrap_or(Real::INFINITY);
            for _ in 0..48 {
                for (d, t) in [(dl, left), (dr, right)] {
                    if best.is_none_or(|(old, _)| d < old) {
                        best = Some((d, t));
                    }
                }
                if !(a < left && left < right && right < b) {
                    break;
                }
                if dl <= dr {
                    b = right;
                    right = left;
                    dr = dl;
                    left = a * (1. - F) + b * F;
                    dl = score(left).unwrap_or(Real::INFINITY);
                } else {
                    a = left;
                    left = right;
                    dl = dr;
                    right = a * F + b * (1. - F);
                    dr = score(right).unwrap_or(Real::INFINITY);
                }
            }
        }
        best.map(|(_, t)| t)
    }
}
