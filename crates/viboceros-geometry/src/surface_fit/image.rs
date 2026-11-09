//! Geometric image fitting with a certified corner-quad proposal.
use super::*;
mod basis;
mod certificate;
mod correspondence;
pub use basis::{try_fit_surface_image_in_basis, try_fit_surface_image_in_basis_with_solver};
pub use certificate::SurfaceImageFitCertificate;

/// A polynomial fit with a complete symmetric Hausdorff bound on its image.
/// This type makes no normalized-UV correspondence promise.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedSurfaceImageFit {
    pub surface: NurbsSurface,
    pub maximum_deviation_bound: Real,
    /// Retained complete correspondence, independently verifiable on the result.
    pub certificate: SurfaceImageFitCertificate,
}

/// Fit a complete untrimmed surface image, permitting UV speed changes.
/// First try the ordinary one-patch parameter fit. If it declines, propose a
/// bilinear corner patch elevated to the requested degrees and admit it only
/// through the exact planar image certificate. Each remaining candidate may
/// also receive an independent monotone U/V map proposed from mid-isocurves.
/// Samples reject poor proposals; only a complete mapped certificate admits
/// them. Otherwise continue ordinary parameter-guided basis refinement.
///
/// Preserves source domains, not parameter correspondence. Uses SurfaceFitOptions
/// and its degree/control limits; zero is strict. A declined image proposal is
/// inconclusive. No trimming, injectivity, topology or Rhino-policy claim.
pub fn try_fit_surface_image_with_bound(
    source: &NurbsSurface,
    options: SurfaceFitOptions,
    limit: Real,
) -> Result<Option<BoundedSurfaceImageFit>, GeometryError> {
    try_fit_surface_image_with_bound_with_solver(
        source,
        options,
        limit,
        &mut crate::surface_rebuild::portable_solve,
    )
}

/// Share proposals and complete admission with an external collocation backend.
/// The analytic bilinear control net needs no backend solve.
pub fn try_fit_surface_image_with_bound_with_solver<E: From<GeometryError>>(
    source: &NurbsSurface,
    options: SurfaceFitOptions,
    limit: Real,
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<Option<BoundedSurfaceImageFit>, E> {
    let mut certificate = SurfaceImageFitCertificate::NormalizedUv;
    Ok(
        adaptive::fit_with_fallback(source, options, limit, solve, |candidate, first| {
            if first {
                let surface = corner_patch(source, options.degree)?;
                if let Some(maximum_deviation_bound) =
                    source.planar_surface_image_deviation_bound(&surface, limit)?
                {
                    certificate = SurfaceImageFitCertificate::PlanarQuad;
                    return Ok(Some(BoundedSurfaceFit {
                        surface,
                        maximum_deviation_bound,
                    }));
                }
            }
            if let Some((maximum_deviation_bound, maps)) =
                correspondence::certify(source, candidate, limit)?
            {
                certificate = SurfaceImageFitCertificate::SeparableUv(maps);
                return Ok(Some(BoundedSurfaceFit {
                    surface: candidate.clone(),
                    maximum_deviation_bound,
                }));
            }
            Ok(None)
        })?
        .map(|fit| BoundedSurfaceImageFit {
            surface: fit.surface,
            maximum_deviation_bound: fit.maximum_deviation_bound,
            certificate,
        }),
    )
}

fn corner_patch(source: &NurbsSurface, degree: [usize; 2]) -> Result<NurbsSurface, GeometryError> {
    let [pu, pv] = degree;
    let nu = source.control_point_count_u();
    let n = source.control_points().len();
    // Stored corners need not be attained on an unclamped source. The image
    // certificate rejects such a source, so these controls cannot authorize it.
    let corners = [0, nu - 1, n - nu, n - 1].map(|i| source.control_points()[i].point().to_array());
    let mut points = Vec::with_capacity((pu + 1) * (pv + 1));
    let blend = |a: Real, b: Real, t: Real| {
        if t == 0. {
            a
        } else if t == 1. {
            b
        } else {
            (1. - t) * a + t * b
        }
    };
    for j in 0..=pv {
        for i in 0..=pu {
            let u = i as Real / pu as Real;
            let v = j as Real / pv as Real;
            points.push(Point3::try_from(std::array::from_fn(|k| {
                blend(
                    blend(corners[0][k], corners[1][k], u),
                    blend(corners[2][k], corners[3][k], u),
                    v,
                )
            }))?);
        }
    }
    let knots = |p: usize, d: std::ops::RangeInclusive<Real>| {
        [vec![*d.start(); p + 1], vec![*d.end(); p + 1]].concat()
    };
    NurbsSurface::try_new(
        pu,
        pv,
        pu + 1,
        pv + 1,
        points,
        knots(pu, source.domain_u()),
        knots(pv, source.domain_v()),
    )
}

#[cfg(test)]
mod tests;
