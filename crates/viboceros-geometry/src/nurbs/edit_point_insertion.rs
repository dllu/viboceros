//! Invert the Greville mean, then use the existing exact knot refinement.
use super::*;
#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Knot which places a new Greville edit station at `parameter`. The
    /// surrounding existing Greville stations choose its control index. This
    /// differs from inserting a knot directly at the requested edit station.
    pub fn edit_point_insertion_knot(
        &self,
        parameter: Real,
    ) -> Result<Option<Real>, GeometryError> {
        self.validate_parameter(parameter)?;
        let domain = self.domain();
        if parameter == *domain.start() || parameter == *domain.end() || self.degree == 1 {
            return Ok(Some(parameter));
        }
        let (mut lower, mut upper) = (0, self.control_points.len());
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if self.control_greville_parameter(middle)? < parameter {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        let index = lower.min(self.control_points.len() - 1);
        for i in [index.saturating_sub(1), index] {
            let station = self.control_greville_parameter(i)?;
            let scale = parameter
                .abs()
                .max(station.abs())
                .max(domain.start().abs())
                .max(domain.end().abs());
            let previous = self.control_greville_parameter(i.saturating_sub(1))?;
            let next =
                self.control_greville_parameter((i + 1).min(self.control_points.len() - 1))?;
            // Scale before subtracting: a valid finite-knot chart can span
            // both signs near f64::MAX without a representable total width.
            let station_scaled = station * OPENNURBS_SQRT_EPSILON;
            let spacing_scaled = (station_scaled - previous * OPENNURBS_SQRT_EPSILON)
                .abs()
                .min((next * OPENNURBS_SQRT_EPSILON - station_scaled).abs());
            let station_tolerance = (2. * spacing_scaled).max(128. * Real::EPSILON * scale);
            if (station - parameter).abs() <= station_tolerance {
                return Ok(None);
            }
        }
        let mean = stable_knot_mean(&self.knots[index + 1..index + self.degree])?;
        // t = (k + sum(p-1 neighboring knots))/p. The shifted form avoids
        // overflowing p*t when the complete parameter chart is translated.
        let knot = parameter + (parameter - mean) * (self.degree - 1) as Real;
        require_finite([knot], "edit-point insertion knot")?;
        if !domain.contains(&knot) {
            return Ok(None);
        }
        let span = self.find_span(knot);
        let a = self.knots[span];
        let b = self.knots[span + 1];
        let tolerance = (a.abs() * OPENNURBS_SQRT_EPSILON
            + b.abs() * OPENNURBS_SQRT_EPSILON
            + (b * OPENNURBS_SQRT_EPSILON - a * OPENNURBS_SQRT_EPSILON).abs())
        .max(Real::EPSILON);
        if (span == self.degree && knot - *domain.start() <= tolerance)
            || (span == self.control_points.len() - 1 && *domain.end() - knot <= tolerance)
        {
            return Ok(None);
        }
        Ok(Some(knot))
    }

    /// Evaluate the Greville stations inside the active parameter domain for
    /// display. Unlike edit-grip interpolation, this also admits a curve with
    /// only one clamped end. Closed seams have one marker.
    pub fn greville_points_in_active_domain(&self) -> Result<Vec<Point3>, GeometryError> {
        let parameters = if self.is_periodic() {
            periodic_greville_parameters(
                self.degree,
                self.control_points.len(),
                self.control_points.len() - self.degree,
                &self.knots,
            )?
            .0
        } else {
            (0..self.control_points.len())
                .map(|i| self.control_greville_parameter(i))
                .collect::<Result<Vec<_>, _>>()?
        };
        let domain = self.domain();
        let mut points = parameters
            .into_iter()
            .filter(|t| domain.contains(t))
            .map(|t| self.evaluate(t))
            .collect::<Result<Vec<_>, _>>()?;
        if points.len() > 1
            && self.is_closed()?
            && crate::curve_points_coincident(points[0], *points.last().unwrap())
        {
            points.pop();
        }
        Ok(points)
    }

    /// Add a Greville edit station without changing the curve's locus.
    /// Refinement retains rational controls, domains and periodic topology.
    /// Active endpoint picks clamp only the corresponding end.
    pub fn try_insert_edit_point(&self, parameter: Real) -> Result<Self, GeometryError> {
        let Some(knot) = self.edit_point_insertion_knot(parameter)? else {
            return Ok(self.clone());
        };
        let target = (self.knot_multiplicity(knot)? + 1).min(self.degree);
        self.try_insert_knot(knot, target)
    }
}
