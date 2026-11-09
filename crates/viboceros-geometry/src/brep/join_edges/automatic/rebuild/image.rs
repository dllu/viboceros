//! Join work accounting for the shared exact surface-boundary certificate.
use super::*;
use crate::brep::trim_image::isocurve;
#[cfg(test)]
mod tests;

pub(in crate::brep::join_edges) struct BoundaryImage(isocurve::BoundaryImage);
impl BoundaryImage {
    pub fn new(
        surface: &NurbsSurface,
        trim: &BrepTrim,
        budget: &mut Budget,
    ) -> Result<Option<Self>, GeometryError> {
        Ok(isocurve::BoundaryImage::new(surface, trim, &mut |n| budget.charge(n))?.map(Self))
    }
    pub fn bound(
        &self,
        edge: &NurbsCurve,
        reversed_3d: bool,
        tighten: bool,
        budget: &mut Budget,
    ) -> Result<Option<Real>, GeometryError> {
        self.0
            .bound(edge, reversed_3d, tighten, &mut |n| budget.charge(n))
    }
    pub fn endpoint_bound(
        &self,
        point: Point3,
        end: bool,
        budget: &mut Budget,
    ) -> Result<Option<Real>, GeometryError> {
        self.0.endpoint_bound(point, end, &mut |n| budget.charge(n))
    }

    pub fn bound_with_witnesses(
        &self,
        edge: &NurbsCurve,
        reversed_3d: bool,
        limit: Real,
        witnesses: &[&NurbsCurve],
        budget: &mut Budget,
    ) -> Result<Option<Real>, GeometryError> {
        for witness in witnesses {
            for reversed in [false, true] {
                let Some(a) =
                    certificate::whole_curve_bound(edge, witness, reversed, limit * 0.5, |n| {
                        budget.charge(n)
                    })?
                else {
                    continue;
                };
                // A sufficient complete bound avoids searching alternate
                // projective maps for a smaller roundoff-level residual.
                let Some(b) = self.bound(witness, reversed_3d ^ reversed, false, budget)? else {
                    continue;
                };
                let sum = certificate::add_bound(a, b)?;
                if sum <= limit {
                    return Ok(Some(sum));
                }
            }
        }
        self.bound(edge, reversed_3d, true, budget)
    }
}
