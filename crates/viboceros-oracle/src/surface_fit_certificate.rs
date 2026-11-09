//! Serializable retained correspondences for complete source-image checks.
use super::*;
pub(super) fn value(certificate: &viboceros_geometry::SurfaceImageFitCertificate) -> Value {
    use viboceros_geometry::SurfaceImageFitCertificate::*;
    match certificate {
        NormalizedUv => json!({"type":"normalized_uv"}),
        PlanarQuad => json!({"type":"planar_quad"}),
        SeparableUv(maps) => json!({"type":"separable_uv","correspondence":maps}),
    }
}
