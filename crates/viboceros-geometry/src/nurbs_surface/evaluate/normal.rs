//! Scale-free regular normals: outward-rounded filter, exact rational recovery.
use super::*;
use crate::UnitVector3;
mod filter;

#[cfg(test)]
mod tests;

impl NurbsSurface {
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
        let spans = [
            checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?,
            checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?,
        ];
        if let Some(normal) = filter::normal(self, [u, v], spans) {
            return Ok(normal);
        }
        exact::ExactJetNet::new(self, spans).normal([u, v])
    }
}
