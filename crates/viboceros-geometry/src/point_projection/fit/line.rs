//! Least-squares lines share the plane fitter's centering and thin SVD.
use super::*;
use crate::LineSegment;

pub const MAX_LINE_FIT_POINTS: usize = MAX_PLANE_FIT_POINTS;
#[cfg(test)]
mod tests;

impl PointProjection3 {
    /// Minimize the sum of squared orthogonal distances to an infinite line.
    /// Each point occurrence has equal weight. Exact collinear inputs preserve
    /// their exact supporting line; coincident inputs return `None`. Other
    /// inputs share the plane fitter's exactly centered, isotropically scaled
    /// thin SVD, choosing its largest singular direction. Repeated largest
    /// singular values admit multiple equally good lines.
    pub fn onto_best_fit_line(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        if points.len() < 2 {
            return Err(GeometryError::Degenerate {
                context: "line fitting needs at least two points",
            });
        }
        if points.len() > MAX_LINE_FIT_POINTS {
            return Err(GeometryError::LineFitResourceLimit {
                maximum: MAX_LINE_FIT_POINTS,
            });
        }
        let a = points[0];
        let Some(&b) = points.iter().find(|&&p| p != a) else {
            return Ok(None);
        };
        let direction = difference(b, a);
        let mut line = if points.iter().all(|&p| {
            cross(&direction, &difference(p, a))
                .iter()
                .all(Zero::is_zero)
        }) {
            Self::new(a, direction, true)?
        } else {
            principal_axis(points, true).map_err(|e| match e {
                GeometryError::PlaneFitDidNotConverge => GeometryError::LineFitDidNotConverge,
                GeometryError::PlaneFitNumericalRange => GeometryError::LineFitNumericalRange,
                other => other,
            })?
        };
        // A deterministic positive dominant component fixes the output segment
        // orientation without changing the represented infinite projection.
        let dominant = (0..3)
            .min_by(|&i, &j| line.axis[j].abs().cmp(&line.axis[i].abs()))
            .unwrap();
        if line.axis[dominant].is_negative() {
            line.axis = line.axis.map(|v| -v);
        }
        Ok(Some(line))
    }
}

impl LineSegment {
    /// Fit an orthogonal least-squares line and bound its complete projected
    /// point set. Extrema are selected by exact scalar products; only the two
    /// final projected endpoints are rounded. The domain measures the stored
    /// endpoint distance. Coincident stored endpoints return `None`.
    /// Numerical/resource failures remain errors. Model tolerance does not
    /// discard a short, representable fit.
    pub fn try_fit_to_points(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        let Some(projector) = PointProjection3::onto_best_fit_line(points)? else {
            return Ok(None);
        };
        let coordinate = |p: Point3| {
            projector
                .axis
                .iter()
                .zip(p.to_array())
                .map(|(a, p)| a * rational(p))
                .sum::<Rational>()
        };
        let mut lo = (points[0], coordinate(points[0]));
        let mut hi = lo.clone();
        for &p in &points[1..] {
            let value = coordinate(p);
            if value < lo.1 {
                lo = (p, value.clone());
            }
            if value > hi.1 {
                hi = (p, value);
            }
        }
        let a = projector.project(lo.0)?;
        let b = projector.project(hi.0)?;
        if a == b {
            return Ok(None);
        }
        let length = a.distance_to(b)?;
        if length <= 0. {
            return Err(GeometryError::LineFitNumericalRange);
        }
        Ok(Some(Self::from_validated(a, b, [0., length])))
    }
}
