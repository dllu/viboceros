//! Bounded screen-space location query on the already-selected original edge.
use super::*;
use viboceros_drafting::ObjectSnap;

#[derive(Clone, Copy)]
pub(super) struct EdgePointCursor {
    pub(super) parameter: Real,
    snap: Option<ObjectSnap>,
}

pub(super) struct EdgeSnapCache {
    curve: NurbsCurve,
    point: Point3,
    tolerance: Tolerance,
    parameter: Option<Real>,
}

impl Viewport {
    pub(super) fn edge_point_cursor(
        &self,
        curve: &NurbsCurve,
        distance_parameters: Option<&[Real]>,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        osnap: bool,
    ) -> Option<EdgePointCursor> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return None;
        }
        let snap = osnap
            .then(|| self.object_snap(pointer, rect, document))
            .flatten();
        let parameter = match (snap, distance_parameters) {
            (Some(snap), Some(parameters)) => {
                // A feature snap chooses in model space, even when its screen
                // projection lies closer to the other distance candidate.
                parameters
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &t)| {
                        Some((
                            curve.evaluate(t).ok()?.distance_to(snap.point()).ok()?,
                            index,
                            t,
                        ))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?
                    .2
            }
            (Some(snap), None) => {
                self.edge_snap_parameter(curve, snap.point(), document.tolerance())?
            }
            (None, Some(parameters)) => {
                self.pick_edge_distance_parameter(curve, parameters, pointer, rect)?
            }
            (None, None) => self.pick_edge_parameter(curve, pointer, rect, osnap)?,
        };
        Some(EdgePointCursor { parameter, snap })
    }

    fn edge_snap_parameter(
        &self,
        curve: &NurbsCurve,
        point: Point3,
        tolerance: Tolerance,
    ) -> Option<Real> {
        let mut cache = self.edge_snap_cache.borrow_mut();
        if let Some(old) = cache.as_ref()
            && old.point == point
            && old.tolerance == tolerance
            && old.curve == *curve
        {
            return old.parameter;
        }
        #[cfg(test)]
        self.edge_snap_queries.set(self.edge_snap_queries.get() + 1);
        let parameter = curve.closest_parameter(point, tolerance).ok();
        *cache = Some(EdgeSnapCache {
            curve: curve.clone(),
            point,
            tolerance,
            parameter,
        });
        parameter
    }

    pub(super) fn paint_edge_point_cursor(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        curve: &NurbsCurve,
        cursor: EdgePointCursor,
    ) {
        let Ok(point) = curve.evaluate(cursor.parameter) else {
            return;
        };
        let Some(pixel) = self.project(point, rect) else {
            return;
        };
        if let Some(snap) = cursor.snap
            && let Some(source) = self.project(snap.point(), rect)
        {
            // Show the actual feature separately from its edge-constrained point.
            let color = SNAP_COLOR;
            painter.circle_stroke(source, 4., Stroke::new(1.25, color));
            if source.distance(pixel) > 1. {
                painter.line_segment([source, pixel], Stroke::new(0.75, color));
            }
            painter.text(
                source + Vec2::new(8., -8.),
                Align2::LEFT_BOTTOM,
                snap.kind().label(),
                FontId::proportional(12.),
                color,
            );
        }
        painter.circle_filled(pixel, 4., SELECTED_COLOR);
    }

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
                        .filter(|&d| d <= Real::from(OSNAP_CAPTURE_PIXELS))
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
        // Once selected, the edge constrains every point pick in this viewport;
        // unlike component selection this is not an eight-pixel hit test.
        best.map(|(_, t)| t)
    }
}

#[cfg(test)]
mod tests;
