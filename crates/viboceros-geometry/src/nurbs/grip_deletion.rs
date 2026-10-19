//! Deletion acts on stable displayed indices, descending through the source net.
use super::*;
use std::collections::BTreeSet;

impl NurbsCurve {
    /// Delete displayed controls. None means that every grip was selected.
    /// A request leaving only one grip is ignored, preserving the source net.
    /// Closed/periodic layouts retain at least three unique controls.
    pub fn try_delete_grips(
        &self,
        indices: &BTreeSet<usize>,
    ) -> Result<Option<Self>, GeometryError> {
        self.delete_grips(indices, true)
    }

    pub(crate) fn delete_grips(
        &self,
        indices: &BTreeSet<usize>,
        normalize_weights: bool,
    ) -> Result<Option<Self>, GeometryError> {
        let count = self.grip_count()?;
        if let Some(&index) = indices.last().filter(|&&i| i >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if indices.len() == count {
            return Ok(None);
        }
        if indices.is_empty() || count - indices.len() < 2 {
            return Ok(Some(self.clone()));
        }
        let mut result = self.clone();
        let uniform = self.command_uniform_step();
        let source_domain = self.domain();
        // A seam conversion that retains the stored control count keeps the
        // reduced clamped basis's interior spans, then distributes the averaged
        // end span around the seam.
        // Separate Delete commands regenerate a uniform basis between edits.
        let seam_knots = if self.degree >= 3
            && !self.is_periodic()
            && self.is_closed()?
            && indices.len() == self.degree - 1
            && indices.contains(&0)
            && count >= 3 * (self.degree - 1)
            && uniform.is_some()
        {
            let degree = self.degree;
            let mut raw = self.clone();
            for &index in indices.iter().rev().filter(|&&i| i != 0) {
                raw = raw.remove_control_point(index, false)?;
            }
            let spans = raw.knots[degree..=raw.control_points.len()]
                .windows(2)
                .map(|k| k[1] - k[0])
                .collect::<Vec<_>>();
            let seam = 0.5 * (spans[0] + spans[spans.len() - 1]);
            let mut intervals = vec![seam; degree - 1];
            intervals.extend_from_slice(&spans[degree - 1..spans.len() - 1]);
            intervals.extend(std::iter::repeat_n(seam, degree - 1));
            let sum = intervals.iter().sum::<Real>();
            for span in &mut intervals {
                *span /= sum;
            }
            Some(periodic_knots_from_normalized_intervals(
                degree,
                count + 1,
                &intervals,
                *source_domain.start(),
                *source_domain.end(),
            )?)
        } else {
            None
        };
        for &index in indices.iter().rev() {
            let n = result.grip_count()?;
            if (result.is_closed()? || result.is_periodic()) && n <= 3 {
                continue;
            }
            if result.is_periodic() {
                result = result.try_remove_control_point_preserving_weights(index)?;
                if let Some(step) = uniform {
                    let end = result.grip_count()? as Real * step;
                    result = result.try_make_uniform()?.try_reparameterized(0.0..=end)?;
                }
            } else if result.is_closed()? && index == 0 {
                let mut controls = result.control_points[1..n].to_vec();
                controls.rotate_right(1);
                let degree = result.degree.min(controls.len());
                if uniform.is_some() {
                    result = Self::try_unit_periodic_from_unique_controls(degree, controls)?
                        .try_reparameterized(source_domain.clone())?;
                } else {
                    controls.extend_from_within(..degree);
                    let knots =
                        periodic_knots_preserving_active(degree, controls.len(), &result.knots)?;
                    result = Self::try_new_rational(degree, controls, knots)?;
                }
            } else {
                let endpoint_weights = normalize_weights && !result.is_closed()?;
                let knot_repair = if result.control_points.len() > result.degree + 1
                    && result.degree.is_multiple_of(2)
                {
                    let first = result.degree + 1;
                    let last = result.control_points.len() - 1;
                    let mut target = (index + result.degree.div_ceil(2)).clamp(first, last);
                    if result.knots[target] == result.knots[target - 1] {
                        let mut next = target;
                        while next < last && result.knots[next] == result.knots[target] {
                            next += 1;
                        }
                        if result.knots[next] != result.knots[next + 1] {
                            target = next;
                        }
                    }
                    let mut knots = result.knots.clone();
                    if target > first
                        && target < last
                        && knots[target - 1] != knots[target - 2]
                        && knots[target] != knots[target + 1]
                    {
                        knots[target - 1] = stable_knot_mean(&[knots[target - 1], knots[target]])?;
                    }
                    knots.remove(target);
                    Some(knots)
                } else {
                    None
                };
                result = result.remove_control_point(index, endpoint_weights)?;
                if let Some(knots) = knot_repair {
                    result.knots = knots;
                }
                if let Some(step) = uniform {
                    let end = (result.control_points.len() - result.degree) as Real * step;
                    result = result.try_make_uniform()?.try_reparameterized(0.0..=end)?;
                }
            }
        }
        if let Some(knots) = seam_knots {
            result = Self::try_new_rational(result.degree, result.control_points, knots)?;
        }
        if normalize_weights
            && result
                .control_points
                .iter()
                .all(|p| p.weight() == result.control_points[0].weight())
        {
            result.control_points = result
                .control_points
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), 1.))
                .collect::<Result<_, _>>()?;
        }
        Ok(Some(result))
    }

    /// Periodic edit stations use the domain-aligned Greville window. At an
    /// open boundary the new endpoint is the first remaining edit station.
    pub fn try_delete_edit_grips(
        &self,
        indices: &BTreeSet<usize>,
    ) -> Result<Option<Self>, GeometryError> {
        let count = self.grip_count()?;
        if let Some(&index) = indices.last().filter(|&&i| i >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if self.is_periodic() {
            let (_, start) = periodic_greville_parameters(
                self.degree,
                self.control_points.len(),
                count,
                &self.knots,
            )?;
            return self.try_delete_grips(&indices.iter().map(|i| (i + start) % count).collect());
        }
        let result = self.try_delete_grips(indices)?;
        let Some(mut result) = result else {
            return Ok(None);
        };
        if !indices.is_empty() && count - indices.len() >= 2 && !self.is_closed()? {
            let stations = self.edit_points()?;
            if indices.contains(&0) {
                let first = (0..count).find(|i| !indices.contains(i)).unwrap();
                let weight = result.control_points[0].weight();
                result.control_points[0] = WeightedPoint3::try_new(stations[first].point, weight)?;
            }
            if indices.contains(&(count - 1)) {
                let last = (0..count).rev().find(|i| !indices.contains(i)).unwrap();
                let n = result.control_points.len() - 1;
                let weight = result.control_points[n].weight();
                result.control_points[n] = WeightedPoint3::try_new(stations[last].point, weight)?;
            }
        }
        Self::try_new_rational(result.degree, result.control_points, result.knots).map(Some)
    }
}
