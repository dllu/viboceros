//! Bounded screen-space location query on the already-selected original edge.
use super::*;

impl Viewport {
    /// A distance constraint chooses the nearest projected cached candidate, even
    /// if the only reachable point lies away from the cursor. No integration or
    /// curve search runs on this per-frame path.
    pub(super) fn pick_edge_distance_parameter(
        &self,
        curve: &NurbsCurve,
        parameters: &[Real],
        pointer: Pos2,
        rect: Rect,
    ) -> Option<Real> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return None;
        }
        parameters
            .iter()
            .filter_map(|&t| {
                let [x, y] = self.project_precise(curve.evaluate(t).ok()?, rect)?;
                let d = (x - Real::from(pointer.x)).hypot(y - Real::from(pointer.y));
                d.is_finite().then_some((d, t))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))
            .map(|(_, t)| t)
    }

    pub(super) fn pick_edge_parameter(
        &self,
        curve: &NurbsCurve,
        pointer: Pos2,
        rect: Rect,
        osnap: bool,
    ) -> Option<Real> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return None;
        }
        let score = |parameter| -> Option<Real> {
            let [x, y] = self.project_precise(curve.evaluate(parameter).ok()?, rect)?;
            Some((x - Real::from(pointer.x)).hypot(y - Real::from(pointer.y)))
        };
        let domain = curve.domain();
        if osnap {
            let mut ends: Vec<_> = [*domain.start(), *domain.end()]
                .into_iter()
                .filter_map(|t| {
                    score(t)
                        .filter(|&d| d <= Real::from(PICK_CAPTURE_PIXELS))
                        .map(|d| (d, t))
                })
                .collect();
            ends.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            if let Some(&(_, t)) = ends.first() {
                return Some(t);
            }
        }
        // Use all span boundaries, then refine the best sampled neighborhoods.
        // This is a bounded numerical UI query, not a certified global minimum.
        let mut spans = Vec::new();
        for span in curve.spans() {
            if spans.len() == 4096 {
                return None;
            }
            spans.push(span);
        }
        let mut intervals = Vec::new();
        let mut best: Option<(Real, Real)> = None;
        for (start, end) in spans {
            let mut previous = None;
            for i in 0..=CURVE_SAMPLES_PER_SPAN {
                let f = i as Real / CURVE_SAMPLES_PER_SPAN as Real;
                let t = start * (1. - f) + end * f;
                let d = score(t);
                if let Some(d) = d {
                    if best.is_none_or(|(old, _)| d < old) {
                        best = Some((d, t));
                    }
                    if let Some((a, da)) = previous {
                        intervals.push((Real::min(d, da), a, t));
                    }
                    previous = Some((t, d));
                } else {
                    previous = None;
                }
            }
        }
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        for &(_, mut a, mut b) in intervals.iter().take(8) {
            // Golden section works in the native domain; endpoint seeds remain
            // candidates. No model-plane intersection or pixel-to-world rounding.
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
        best.filter(|&(d, _)| d <= Real::from(PICK_CAPTURE_PIXELS))
            .map(|(_, t)| t)
    }
}

#[cfg(test)]
mod tests;
