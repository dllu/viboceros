//! Explicit polynomial bases admitted through complete supplied UV mappings.
use super::*;

/// Fit a polynomial tensor in a supplied basis and certify its complete image
/// using independent piecewise affine U/V maps. Each map contains normalized
/// `[source, candidate]` pairs; see [`NurbsSurface::mapped_surface_deviation_bound`].
/// Maps and tolerance are validated before any solve. Input geometry is immutable.
///
/// Shares Greville interpolation and basis/source limits with
/// [`try_interpolate_surface_in_basis`]. Returns `None` for an outside or
/// inconclusive correspondence. Makes no normalized-UV, optimal-map, automatic
/// basis selection, trimming, topology or Rhino fitting-policy claim.
pub fn try_fit_surface_image_in_basis(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
    correspondence: [&[[Real; 2]]; 2],
    limit: Real,
) -> Result<Option<BoundedSurfaceImageFit>, GeometryError> {
    try_fit_surface_image_in_basis_with_solver(
        source,
        degree,
        knots,
        correspondence,
        limit,
        &mut crate::surface_rebuild::portable_solve,
    )
}

/// Retain complete original-surface image admission with an external batched
/// collocation solver. No backend result can bypass the exact certificate.
pub fn try_fit_surface_image_in_basis_with_solver<E: From<GeometryError>>(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
    correspondence: [&[[Real; 2]]; 2],
    limit: Real,
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<Option<BoundedSurfaceImageFit>, E> {
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance.into());
    }
    crate::surface_pullback::validate_surface_correspondence(correspondence)?;
    let surface = try_interpolate_surface_in_basis_with_solver(source, degree, knots, solve)?;
    Ok(source
        .mapped_surface_deviation_bound(&surface, correspondence, limit)?
        .map(|maximum_deviation_bound| BoundedSurfaceImageFit {
            surface,
            maximum_deviation_bound,
        }))
}

#[cfg(test)]
mod tests;
