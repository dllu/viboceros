//! Polynomial tensor fitting in an explicit, clamped U/V basis.
use crate::{GeometryError, NurbsCurve, NurbsInterpolationSystem, NurbsSurface, Point3, Real};
mod adaptive;
mod budget;
mod image;
pub use adaptive::{
    SurfaceFitOptions, try_fit_surface_with_bound, try_fit_surface_with_bound_with_solver,
};
pub use budget::SurfaceFitRefinementBudget;
pub use image::{
    BoundedSurfaceImageFit, try_fit_surface_image_in_basis,
    try_fit_surface_image_in_basis_with_solver, try_fit_surface_image_with_bound,
    try_fit_surface_image_with_bound_with_solver,
};

/// Largest axis in one fitting job; the complete tensor has at most 4096 controls.
pub const MAX_SURFACE_FIT_AXIS: usize = 64;

/// A polynomial candidate admitted by a complete original-surface certificate.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedSurfaceFit {
    pub surface: NurbsSurface,
    pub maximum_deviation_bound: Real,
}

/// Interpolate Cartesian source points at the target basis's tensor Greville
/// stations. This produces a candidate, without a geometric accuracy claim.
/// Retains the source UV domains. Target degrees are 1..=11, with clamped ends,
/// interior multiplicities <= degree and at most 64 controls per direction.
pub fn try_interpolate_surface_in_basis(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
) -> Result<NurbsSurface, GeometryError> {
    try_interpolate_surface_in_basis_with_solver(
        source,
        degree,
        knots,
        &mut crate::surface_rebuild::portable_solve,
    )
}

/// Propose polynomial controls in the supplied basis and certify the entire
/// original active tensor, including one-sided source knot limits. A failed or
/// inconclusive certificate returns `None`; zero requires exact agreement.
/// This does not choose an adaptive basis or certify trimming or continuity.
pub fn try_fit_surface_in_basis(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
    limit: Real,
) -> Result<Option<BoundedSurfaceFit>, GeometryError> {
    try_fit_surface_in_basis_with_solver(
        source,
        degree,
        knots,
        limit,
        &mut crate::surface_rebuild::portable_solve,
    )
}

/// Share candidate preparation and exact admission with a collocation backend.
pub fn try_fit_surface_in_basis_with_solver<E: From<GeometryError>>(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
    limit: Real,
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<Option<BoundedSurfaceFit>, E> {
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance.into());
    }
    let surface = try_interpolate_surface_in_basis_with_solver(source, degree, knots, solve)?;
    Ok(source
        .parameter_surface_deviation_bound(&surface, limit)?
        .map(|maximum_deviation_bound| BoundedSurfaceFit {
            surface,
            maximum_deviation_bound,
        }))
}

/// Batch every XYZ right-hand side into one factorization per parameter axis.
/// The source is sampled once. Local coordinates reduce translation roundoff;
/// constant channels and all four corners are restored exactly after solving.
pub fn try_interpolate_surface_in_basis_with_solver<E: From<GeometryError>>(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    interpolate_with_sampler(source, degree, knots, solve, &mut |u, v| {
        source.evaluate(u, v)
    })
    .map_err(|error| match error {
        SampledFitError::Sample(error) => error.into(),
        SampledFitError::Backend(error) => error,
    })
}

enum SampledFitError<E> {
    Sample(GeometryError),
    Backend(E),
}
impl<E> From<GeometryError> for SampledFitError<E> {
    fn from(error: GeometryError) -> Self {
        Self::Sample(error)
    }
}

fn interpolate_with_sampler<E: From<GeometryError>>(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[Real]; 2],
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
    sample: &mut impl FnMut(Real, Real) -> Result<Point3, GeometryError>,
) -> Result<NurbsSurface, SampledFitError<E>> {
    validate_source(source)?;
    let u = stations(degree[0], knots[0], source.domain_u())?;
    let v = stations(degree[1], knots[1], source.domain_v())?;
    let (nu, nv) = (u.len(), v.len());
    let mut samples = Vec::with_capacity(nu * nv);
    for &y in &v {
        for &x in &u {
            samples.push(sample(x, y)?.to_array());
        }
    }
    let origin = samples[0];
    let constant: [bool; 3] = std::array::from_fn(|k| samples.iter().all(|p| p[k] == origin[k]));
    let mut targets = Vec::with_capacity(nu * nv * 3);
    for i in 0..nu {
        for j in 0..nv {
            for (k, &o) in origin.iter().enumerate() {
                let value = samples[j * nu + i][k] - o;
                if !value.is_finite() {
                    return Err(GeometryError::SurfaceApproximationLimit.into());
                }
                targets.push(value);
            }
        }
    }
    let along_u = checked_solve(solve, degree[0], knots[0], &u, nv * 3, &targets)
        .map_err(SampledFitError::Backend)?;
    targets.clear();
    for j in 0..nv {
        for i in 0..nu {
            targets.extend_from_slice(&along_u[(i * nv + j) * 3..(i * nv + j + 1) * 3]);
        }
    }
    let along_v = checked_solve(solve, degree[1], knots[1], &v, nu * 3, &targets)
        .map_err(SampledFitError::Backend)?;
    let controls = (0..nu * nv)
        .map(|i| {
            let corner = (i % nu == 0 || i % nu + 1 == nu) && (i / nu == 0 || i / nu + 1 == nv);
            Point3::try_from(if corner {
                samples[i]
            } else {
                std::array::from_fn(|k| {
                    if constant[k] {
                        origin[k]
                    } else {
                        along_v[i * 3 + k] + origin[k]
                    }
                })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NurbsSurface::try_new(
        degree[0],
        degree[1],
        nu,
        nv,
        controls,
        knots[0].to_vec(),
        knots[1].to_vec(),
    )?)
}

fn checked_solve<E: From<GeometryError>>(
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
    degree: usize,
    knots: &[Real],
    parameters: &[Real],
    columns: usize,
    targets: &[Real],
) -> Result<Vec<Real>, E> {
    let result = solve(&NurbsInterpolationSystem {
        degree,
        knots,
        parameters,
        periodic: false,
        columns,
        targets,
    })?;
    if result.len() != targets.len() || result.iter().any(|x| !x.is_finite()) {
        return Err(GeometryError::InterpolationBackendFailed {
            message: "surface fitting solver returned malformed controls".into(),
        }
        .into());
    }
    Ok(result)
}

fn validate_source(source: &NurbsSurface) -> Result<(), GeometryError> {
    if source.control_points().len() > 4096 {
        return Err(GeometryError::SurfaceApproximationLimit);
    }
    let sign = source.control_points()[0].weight().signum();
    if source.degree_u() > 16
        || source.degree_v() > 16
        || source
            .control_points()
            .iter()
            .any(|p| p.weight() == 0. || p.weight().signum() != sign)
    {
        return Err(unsupported(
            "requires source degrees <=16 and nonzero common-sign weights",
        ));
    }
    Ok(())
}

fn stations(
    degree: usize,
    knots: &[Real],
    domain: std::ops::RangeInclusive<Real>,
) -> Result<Vec<Real>, GeometryError> {
    if !(1..=11).contains(&degree) || knots.len() < 2 * (degree + 1) {
        return Err(unsupported(
            "requires degrees 1..=11 and at least degree+1 controls",
        ));
    }
    let count = knots.len() - degree - 1;
    if count > MAX_SURFACE_FIT_AXIS {
        return Err(GeometryError::SurfaceApproximationLimit);
    }
    // Reuse the curve constructor's finite, sorted and multiplicity validation.
    let basis = NurbsCurve::try_new(
        degree,
        vec![Point3::try_new(0., 0., 0.)?; count],
        knots.to_vec(),
    )?;
    if basis.domain() != domain
        || knots[..=degree].iter().any(|t| *t != *domain.start())
        || knots[count..].iter().any(|t| *t != *domain.end())
        || knots[degree + 1..count]
            .chunk_by(|a, b| a == b)
            .any(|g| g.len() > degree)
    {
        return Err(unsupported(
            "requires clamped target ends on the source domain and interior multiplicities <= degree",
        ));
    }
    let parameters = (0..count)
        .map(|i| crate::nurbs::stable_knot_mean(&knots[i + 1..=i + degree]))
        .collect::<Result<Vec<_>, _>>()?;
    if parameters.windows(2).any(|w| w[0] >= w[1]) {
        return Err(unsupported(
            "target Greville stations must be strictly increasing",
        ));
    }
    Ok(parameters)
}

fn unsupported(context: &'static str) -> GeometryError {
    GeometryError::UnsupportedSurfaceApproximation { context }
}

#[cfg(test)]
mod tests;
