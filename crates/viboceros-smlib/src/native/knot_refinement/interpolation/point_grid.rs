//! Native open and periodic point-grid collocation.
use super::*;

pub fn interpolate_open_point_grid(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, Error> {
    interpolate_point_grid(points, count, degree, [false; 2])
}

/// Open or closed Rhino-style point-grid interpolation through native basis
/// continuation and batched LU; core knot/topology/residual policy stays shared.
pub fn interpolate_point_grid(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
) -> Result<NurbsSurface, Error> {
    NurbsSurface::try_through_point_grid_with_solver(
        points,
        count,
        degree,
        closed,
        &mut solve_point_grid,
    )
}

pub fn point_grid_brep(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
    tolerance: Tolerance,
) -> Result<viboceros_geometry::Brep, Error> {
    viboceros_geometry::Brep::try_through_point_grid_with_solver(
        points,
        count,
        degree,
        closed,
        tolerance,
        &mut solve_point_grid,
    )
}

pub(super) fn solve_point_grid(
    input: &viboceros_geometry::PointGridInterpolationSystem<'_>,
) -> Result<viboceros_geometry::PointGridInterpolationSolution, Error> {
    let mut amplification = vec![0.; input.parameters.len()];
    let view = InterpolationSystem {
        knots: input.knots.as_ptr(),
        parameters: input.parameters.as_ptr(),
        targets: input.targets.as_ptr(),
        knot_count: input.knots.len(),
        parameter_count: input.parameters.len(),
        columns: input.columns,
        target_count: input.targets.len(),
        degree: input.degree,
        closed: c_int::from(input.closed),
        handles: 0,
        uniform_periodic: 0,
        fixed_rows: std::ptr::null(),
        fixed_count: 0,
        point_grid: 1,
        amplification: amplification.as_mut_ptr(),
        amplification_count: amplification.len(),
        nonuniform_periodic: 0,
        periodic_control_shift: 0,
        derivative_orders: std::ptr::null(),
        derivative_count: 0,
    };
    let mut controls = vec![0.; input.targets.len()];
    call(|error, capacity| unsafe {
        vb_interpolation_solve(
            &view,
            controls.as_mut_ptr(),
            controls.len(),
            error,
            capacity,
        )
    })?;
    Ok(viboceros_geometry::PointGridInterpolationSolution {
        controls,
        amplification,
    })
}
