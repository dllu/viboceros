//! Curvature analysis statistics, qualified against public Rhino SDK outputs.
use crate::{CurvatureStyle, Real};

impl CurvatureStyle {
    /// Finite values accepted by OpenNURBS mesh curvature statistics.
    pub fn statistics_value(self, principal: [Real; 2]) -> Option<Real> {
        let limit = if self == Self::Gaussian { 1e20 } else { 1e10 };
        self.value(principal)
            .filter(|v| v.is_finite() && v.abs() < limit)
    }

    /// Suggest a color interval from distinct analysis-mesh vertex samples.
    /// Empty input returns `None`; samples entirely beyond the statistics
    /// cutoff return `[0, 0]`. Constant data can also return a singleton.
    pub fn automatic_range(
        self,
        samples: impl IntoIterator<Item = [Real; 2]>,
    ) -> Option<[Real; 2]> {
        let mut count = 0;
        let mut values = samples
            .into_iter()
            .filter_map(|k| {
                count += 1;
                self.statistics_value(k)
            })
            .collect::<Vec<_>>();
        if count == 0 {
            return None;
        }
        if values.is_empty() {
            return Some([0., 0.]);
        }
        values.sort_unstable_by(Real::total_cmp);
        let mut lo = values[0];
        let mut hi = values[values.len() - 1];
        if lo == hi {
            return Some([lo, hi]);
        }
        let mean = values.iter().sum::<Real>() / values.len() as Real;
        let deviation =
            values.iter().map(|k| (k - mean).abs()).sum::<Real>() / values.len() as Real;
        if lo < mean - 3. * deviation {
            lo = mean - deviation;
        }
        if hi > mean + 3. * deviation {
            hi = mean + deviation;
        }
        if self == Self::Gaussian {
            if lo < 0. && hi > 0. {
                let half = 0.5 * (hi - lo);
                lo = -half;
                hi = half;
            } else if lo.abs().max(hi.abs()) >= 4. * lo.abs().min(hi.abs()) {
                let half = lo.abs().max(hi.abs());
                lo = -half;
                hi = half;
            }
        } else if lo > 0. {
            // A very small lower bound is treated as a sampled zero for the
            // span cap; preserve its measured value rather than rounding it.
            if lo >= 0.001 {
                hi = hi.min(20. * lo);
            }
            if hi - lo < 1e-4 * lo.abs().max(hi.abs()) {
                lo *= 0.9999;
                hi *= 1.0001;
            }
        }
        Some([lo, hi])
    }
}
