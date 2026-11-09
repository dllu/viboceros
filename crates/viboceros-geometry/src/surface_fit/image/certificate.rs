//! Retained witnesses for rechecking complete surface image accuracy.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceImageFitCertificate {
    /// Affine normalization of both complete U/V domains.
    NormalizedUv,
    /// Complete convex planar auxiliary quadrilaterals.
    PlanarQuad,
    /// Supplied independent piecewise affine normalized U/V correspondences.
    SeparableUv([Vec<[Real; 2]>; 2]),
}

impl SurfaceImageFitCertificate {
    /// Recheck the complete original tensors, including one-sided knot limits
    /// where supported by the indicated certificate. Invalid retained maps or
    /// tolerances return an error; outside, unsupported or exhausted is `None`.
    /// This does not certify trimming, topology, normals or optimality.
    pub fn verify(
        &self,
        source: &NurbsSurface,
        candidate: &NurbsSurface,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        match self {
            Self::NormalizedUv => source.parameter_surface_deviation_bound(candidate, limit),
            Self::PlanarQuad => source.planar_surface_image_deviation_bound(candidate, limit),
            Self::SeparableUv(maps) => source.mapped_surface_deviation_bound(
                candidate,
                maps.each_ref().map(Vec::as_slice),
                limit,
            ),
        }
    }
}
