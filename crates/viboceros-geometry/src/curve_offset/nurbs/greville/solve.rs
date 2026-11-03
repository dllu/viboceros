//! Shared homogeneous interpolation with optional native fixed-row solving.
use super::*;

pub(super) fn solve(
    axis: &Axis,
    rhs: Mat<Real>,
    solver: Option<crate::CurveEditPointSolver>,
) -> Result<Mat<Real>, GeometryError> {
    let count = axis.stations.len();
    let Some(solver) = solver else {
        let rows = axis
            .stations
            .iter()
            .enumerate()
            .map(|(i, station)| {
                if station.fixed {
                    Ok((i, vec![1.]))
                } else {
                    crate::nurbs::bspline_basis_nonzero(
                        &axis.knots,
                        axis.degree,
                        count,
                        station.parameter,
                    )
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        return crate::spline_collocation::general_banded::solve(axis.degree, &rows, rhs);
    };
    let parameters = axis
        .stations
        .iter()
        .map(|s| s.parameter)
        .collect::<Vec<_>>();
    let fixed = axis
        .stations
        .iter()
        .map(|s| u8::from(s.fixed))
        .collect::<Vec<_>>();
    let targets = (0..count * 4)
        .map(|i| rhs[(i / 4, i % 4)])
        .collect::<Vec<_>>();
    let solution = solver(&crate::CurveEditPointInterpolation {
        system: crate::FixedInterpolationSystem {
            system: crate::NurbsInterpolationSystem {
                degree: axis.degree,
                knots: &axis.knots,
                parameters: &parameters,
                periodic: false,
                columns: 4,
                targets: &targets,
            },
            fixed_rows: &fixed,
        },
        periodic_shift: None,
    })?;
    if solution.len() != targets.len() {
        return Err(GeometryError::CurveEditPointInterpolationFailed);
    }
    crate::require_finite(solution.iter().copied(), "offset interpolation solution")?;
    Ok(Mat::from_fn(count, 4, |i, c| solution[4 * i + c]))
}
