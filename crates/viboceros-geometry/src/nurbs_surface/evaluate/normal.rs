//! Scale-free regular normals: outward-rounded filter, exact rational recovery.
use super::*;
use crate::UnitVector3;
mod filter;
mod support;

#[cfg(test)]
mod tests;

impl NurbsSurface {
    pub(crate) fn minimum_x_support_sense_at(
        &self,
        u: Real,
        v: Real,
        bound: Real,
    ) -> Result<Option<bool>, GeometryError> {
        require_finite([bound], "surface support bound")?;
        let spans = [
            checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?,
            checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?,
        ];
        if support::strictly_above(self, spans, [u, v], bound) {
            return Ok(None);
        }
        exact::ExactJetNet::new(self, spans).minimum_x_support_sense([u, v], bound)
    }

    /// Unit normal in the natural U×V sense at a regular parameter station.
    ///
    /// This derived direction has no model-length cutoff and does not require
    /// either first derivative, its cross product, or the point to fit in f64.
    /// Floating-point filtering bounds every returned component's absolute error
    /// by 1e-12; uncertain cases use the original exact binary64 coefficients.
    /// Internal knots use their right-hand span, endpoints the interior span.
    /// Exact zero denominators and zero first-order crosses are errors; limiting
    /// normals at singular parameterizations are not supplied by this method.
    pub fn normal_at(&self, u: Real, v: Real) -> Result<UnitVector3, GeometryError> {
        self.normal_at_on_sides(u, v, ParameterSide::Right, ParameterSide::Right)
    }

    /// One-sided regular unit normal. At internal knots the requested side
    /// selects the incident span; outer endpoints use their interior span.
    pub fn normal_at_on_sides(
        &self,
        u: Real,
        v: Real,
        side_u: ParameterSide,
        side_v: ParameterSide,
    ) -> Result<UnitVector3, GeometryError> {
        let mut spans = [
            checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?,
            checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?,
        ];
        for (axis, (t, side, degree, count, knots)) in [
            (
                u,
                side_u,
                self.degree_u,
                self.control_point_count_u,
                &self.knots_u,
            ),
            (
                v,
                side_v,
                self.degree_v,
                self.control_point_count_v,
                &self.knots_v,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            if side == ParameterSide::Left && t > knots[degree] && t < knots[count] {
                spans[axis] = knots.partition_point(|value| *value < t) - 1;
            }
        }
        if let Some(normal) = filter::normal(self, [u, v], spans) {
            return Ok(normal);
        }
        exact::ExactJetNet::new(self, spans).normal([u, v])
    }
}
