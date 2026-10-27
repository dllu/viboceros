//! A bounded nonlinear correspondence from a spatial coordinate projection.
use super::*;
use crate::{NurbsCurve2, Point2, WeightedPoint2};

impl NurbsCurve {
    /// Certify complete curve images using a nonlinear parameter proposal.
    /// A coordinate along the target's dominant endpoint displacement supplies
    /// a rational spline parameter map. All proposed map controls must stay
    /// inside the target domain, and the clamped map joins its two endpoints.
    /// Continuity therefore makes the map onto that complete domain, even if
    /// it backtracks. A complete surface-image proof then certifies the rounded
    /// map's Euclidean error and the symmetric Hausdorff distance.
    ///
    /// Both curves must be clamped, with common-sign weights and degrees at
    /// most 16. Interior full-order source knots and a degenerate target chord
    /// decline this proposal. Surface-image degree, work, arithmetic and depth
    /// limits apply. `None` is inconclusive, never proof of different images.
    /// This does not certify tangent continuity, injectivity or topology.
    pub fn projected_curve_deviation_bound(
        &self,
        other: &NurbsCurve,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        bound(self, other, limit, false).map_err(map_work_error)
    }

    /// Certify a proposed continuous, strictly increasing spline map over both
    /// complete active domains. Its unused second coordinate is zero. This is
    /// an internal fitting proposal; the exact image certificate remains the
    /// only acceptance criterion.
    pub(crate) fn monotone_spline_curve_deviation_bound(
        &self,
        other: &NurbsCurve,
        map: &NurbsCurve2,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        (|| {
            if !limit.is_finite() || limit < 0. {
                return Err(GeometryError::InvalidTolerance);
            }
            let controls = map.control_points();
            let degree = map.degree();
            let domain = map.domain();
            let target = other.domain();
            if self.degree() > MAX_DEGREE
                || other.degree() > MAX_DEGREE
                || degree > MAX_DEGREE
                || controls.len() > MAX_WORK / 16
                || !map.knots()[..=degree].iter().all(|t| t == domain.start())
                || !map.knots()[controls.len()..]
                    .iter()
                    .all(|t| t == domain.end())
                || controls[0].point().x() != *target.start()
                || controls.last().unwrap().point().x() != *target.end()
                || controls.iter().any(|p| {
                    p.point().y() != 0.
                        || !target.contains(&p.point().x())
                        || p.weight().is_sign_negative() != controls[0].weight().is_sign_negative()
                })
                || map
                    .knots()
                    .chunk_by(|a, b| a == b)
                    .any(|g| g[0] > *domain.start() && g[0] < *domain.end() && g.len() > degree)
                || !order::certify(map)?
            {
                return Ok(None);
            }
            image_bound(self, other, map, limit)
        })()
        .map_err(map_work_error)
    }

    /// Retain a strictly increasing parameter correspondence for G1 fitting.
    pub(crate) fn monotone_projected_curve_deviation_bound(
        &self,
        other: &NurbsCurve,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        bound(self, other, limit, true).map_err(map_work_error)
    }
}

fn bound(
    source: &NurbsCurve,
    other: &NurbsCurve,
    limit: Real,
    monotone: bool,
) -> Result<Option<Real>, GeometryError> {
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    for curve in [source, other] {
        let degree = curve.degree();
        let controls = curve.control_points();
        let domain = curve.domain();
        if degree > MAX_DEGREE
            || controls.len() > MAX_WORK / 16
            || !curve.knots()[..=degree].iter().all(|k| k == domain.start())
            || !curve.knots()[controls.len()..]
                .iter()
                .all(|k| k == domain.end())
            || controls
                .iter()
                .any(|c| c.weight().is_sign_negative() != controls[0].weight().is_sign_negative())
        {
            return Ok(None);
        }
    }
    if source.knots().chunk_by(|a, b| a == b).any(|g| {
        g[0] > *source.domain().start()
            && g[0] < *source.domain().end()
            && g.len() > source.degree()
    }) {
        return Ok(None);
    }
    let target = other.control_points();
    let start = target[0].point().to_array();
    let end = target.last().unwrap().point().to_array();
    let axis = (0..3)
        .max_by(|&a, &b| {
            (end[a] - start[a])
                .abs()
                .total_cmp(&(end[b] - start[b]).abs())
        })
        .unwrap();
    if start[axis] == end[axis] {
        return Ok(None);
    }
    let (lo, hi) = (start[axis].min(end[axis]), start[axis].max(end[axis]));
    let mut controls = Vec::with_capacity(source.control_points().len());
    for (i, control) in source.control_points().iter().enumerate() {
        let coordinate = control.point().to_array()[axis];
        if coordinate < lo || coordinate > hi {
            return Ok(None);
        }
        let mut fraction = crate::nurbs::interval_fraction(coordinate, lo, hi)?;
        if start[axis] > end[axis] {
            fraction = 1. - fraction;
        }
        let parameter = if i == 0 {
            *other.domain().start()
        } else if i + 1 == source.control_points().len() {
            *other.domain().end()
        } else {
            other
                .parameter_at(fraction)?
                .clamp(*other.domain().start(), *other.domain().end())
        };
        controls.push(WeightedPoint2::try_new(
            Point2::try_new(parameter, 0.)?,
            control.weight(),
        )?);
    }
    let map = NurbsCurve2::try_new_rational(source.degree(), controls, source.knots().to_vec())?;
    // A duplicate row makes the tensor image exactly the target curve;
    // its second parameter is constant and carries no geometric change.
    if monotone && !order::certify(&map)? {
        return Ok(None);
    }
    image_bound(source, other, &map, limit)
}

fn image_bound(
    source: &NurbsCurve,
    other: &NurbsCurve,
    map: &NurbsCurve2,
    limit: Real,
) -> Result<Option<Real>, GeometryError> {
    let target = other.control_points();
    let surface = NurbsSurface::try_new_rational(
        other.degree(),
        1,
        target.len(),
        2,
        target
            .iter()
            .copied()
            .chain(target.iter().copied())
            .collect(),
        other.knots().to_vec(),
        vec![0., 0., 1., 1.],
    )?;
    surface.parameter_curve_deviation_bound(map, source, limit)
}

mod order;

#[cfg(test)]
mod tests;
