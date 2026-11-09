//! Polynomial surface fitting through the shared native collocation adapter.
use super::*;

/// Polynomial tensor candidate with shared Cartesian sampling and two batched
/// usd-brep collocation solves; basis selection remains the caller's policy.
pub fn interpolate_surface_in_basis(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[f64]; 2],
) -> Result<NurbsSurface, Error> {
    viboceros_geometry::try_interpolate_surface_in_basis_with_solver(
        source,
        degree,
        knots,
        &mut solve_cartesian_surface_basis,
    )
}

/// Retain the complete original-tensor displacement gate around native solves.
pub fn fit_surface_in_basis(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[f64]; 2],
    limit: f64,
) -> Result<Option<viboceros_geometry::BoundedSurfaceFit>, Error> {
    viboceros_geometry::try_fit_surface_in_basis_with_solver(
        source,
        degree,
        knots,
        limit,
        &mut solve_cartesian_surface_basis,
    )
}

/// Automatically choose a polynomial basis under the complete original-surface
/// displacement gate, using the shared refinement policy and native solving.
pub fn fit_surface_with_bound(
    source: &NurbsSurface,
    options: viboceros_geometry::SurfaceFitOptions,
    limit: f64,
) -> Result<Option<viboceros_geometry::BoundedSurfaceFit>, Error> {
    viboceros_geometry::try_fit_surface_with_bound_with_solver(
        source,
        options,
        limit,
        &mut solve_cartesian_surface_basis,
    )
}

/// Geometric image fitting with the same complete admission as the Rust API.
pub fn fit_surface_image_with_bound(
    source: &NurbsSurface,
    options: viboceros_geometry::SurfaceFitOptions,
    limit: f64,
) -> Result<Option<viboceros_geometry::BoundedSurfaceImageFit>, Error> {
    viboceros_geometry::try_fit_surface_image_with_bound_with_solver(
        source,
        options,
        limit,
        &mut solve_cartesian_surface_basis,
    )
}

fn solve_cartesian_surface_basis(
    input: &viboceros_geometry::NurbsInterpolationSystem<'_>,
) -> Result<Vec<f64>, Error> {
    // The native bridge batches homogeneous four-channel points. Keep the
    // polynomial fitter's Cartesian contract and pad each XYZ column locally.
    let targets = input
        .targets
        .chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 1.])
        .collect::<Vec<_>>();
    let solved = solve_uniform_system(&viboceros_geometry::NurbsInterpolationSystem {
        degree: input.degree,
        knots: input.knots,
        parameters: input.parameters,
        periodic: input.periodic,
        columns: input.columns / 3 * 4,
        targets: &targets,
    })?;
    Ok(solved
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect())
}

/// Polynomial image fitting in a supplied basis with the same complete mapped
/// certificate as the portable API and two batched usd-brep collocation solves.
pub fn fit_surface_image_in_basis(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[f64]; 2],
    correspondence: [&[[f64; 2]]; 2],
    limit: f64,
) -> Result<Option<viboceros_geometry::BoundedSurfaceImageFit>, Error> {
    viboceros_geometry::try_fit_surface_image_in_basis_with_solver(
        source,
        degree,
        knots,
        correspondence,
        limit,
        &mut solve_cartesian_surface_basis,
    )
}
