//! Shared bounded radial least squares for planar circles and spatial spheres.
use crate::{FiniteSum, GeometryError, Real};
use nalgebra::{SMatrix, SVector};
type Statistics<const D: usize> = (Real, Real, SVector<Real, D>, SMatrix<Real, D, D>);

fn statistics<const D: usize>(
    points: &[[Real; 3]],
    center: SVector<Real, D>,
) -> Result<Statistics<D>, GeometryError> {
    let displacement = |p: &[Real; 3]| {
        let d = SVector::<Real, D>::from_fn(|i, _| center[i] - p[i]);
        let distance = d[0].hypot(d[1]).hypot(if D == 2 { p[2] } else { d[2] });
        (d, distance)
    };
    let mut sum = FiniteSum::default();
    let mut derivative = SVector::<Real, D>::zeros();
    for p in points {
        let (d, distance) = displacement(p);
        sum.add(distance)?;
        if distance > 0. {
            derivative += d / distance;
        }
    }
    let radius = sum.mean()?;
    derivative /= points.len() as Real;
    let mut cost = FiniteSum::default();
    let mut gradient = SVector::<Real, D>::zeros();
    let mut hessian = SMatrix::<Real, D, D>::zeros();
    for p in points {
        let (d, distance) = displacement(p);
        let residual = distance - radius;
        let j = if distance > 0. {
            d / distance
        } else {
            SVector::<Real, D>::zeros()
        } - derivative;
        cost.add(residual * residual)?;
        gradient += j * residual;
        hessian += j * j.transpose();
    }
    Ok((
        radius,
        cost.mean()?,
        gradient / points.len() as Real,
        hessian / points.len() as Real,
    ))
}

pub(crate) fn refine<const D: usize>(
    points: &[[Real; 3]],
    mut center: SVector<Real, D>,
    solve: impl Fn(SMatrix<Real, D, D>, SVector<Real, D>) -> Option<SVector<Real, D>>,
    failure: fn() -> GeometryError,
) -> Result<(SVector<Real, D>, Real), GeometryError> {
    let mut damping = 1e-6;
    let mut current = statistics(points, center)?;
    for _ in 0..256 {
        let (radius, cost, gradient, hessian) = current;
        if gradient.norm() <= 1e-14 {
            return Ok((center, current.0));
        }
        let Some(mut step) = solve(
            hessian + SMatrix::<Real, D, D>::identity() * damping,
            -gradient,
        ) else {
            return Err(failure());
        };
        let bound = 2. * radius.max(1.);
        if step.norm() > bound {
            step *= bound / step.norm();
        }
        let candidate = center + step;
        if candidate.iter().any(|v| !v.is_finite()) {
            return Err(failure());
        }
        let proposed = statistics(points, candidate)?;
        if proposed.1 < cost {
            center = candidate;
            current = proposed;
            damping = (damping * 0.2).max(1e-16);
            if step.norm() <= 1e-13 * (1. + center.norm()) {
                return Ok((center, current.0));
            }
        } else {
            damping *= 10.;
            if damping > 1e12 {
                return Ok((center, current.0));
            }
        }
    }
    Err(failure())
}
