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
}
