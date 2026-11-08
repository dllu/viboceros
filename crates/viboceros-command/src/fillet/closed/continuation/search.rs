//! Enumerate tangent branches and rank their combined native arc-length cost.
use super::*;

pub(super) fn distance(
    curve: &Curve3,
    a: Real,
    b: Real,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    if a == b {
        return Ok(0.);
    }
    if let Curve3::Line(line) = curve {
        let d = line.domain();
        return Ok((a - b).abs() / (*d.end() - *d.start()) * line.length()?);
    }
    let interval = a.min(b)..=a.max(b);
    Ok(crate::curve_segments::trimmed(curve, interval)?
        .as_ref()
        .length(tolerance)?)
}

pub(super) fn score(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    contact: &contacts::Contacts,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    Ok(
        distance(curves[0], seed[0], contact.parameters[0], tolerance)?
            + distance(curves[1], seed[1], contact.parameters[1], tolerance)?,
    )
}

pub(super) fn solutions(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    open: usize,
    radius: Real,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<contacts::Contacts>, CommandError> {
    let mut starts = [vec![seed[0]], vec![seed[1]]];
    for i in 0..2 {
        let domain = curves[i].as_ref().domain();
        let count = if i == open { 8 } else { 4 };
        for j in 0..=count {
            starts[i].push(
                *domain.start() + (*domain.end() - *domain.start()) * j as Real / count as Real,
            );
        }
    }
    let mut found: Vec<contacts::Contacts> = Vec::new();
    for &a in &starts[0] {
        for &b in &starts[1] {
            let Some(candidate) = contacts::solve(curves, [a, b], radius, plane, tolerance)? else {
                continue;
            };
            if !tolerance.approx_eq(candidate.arc.radius(), radius) {
                continue;
            }
            let duplicate = found.iter().any(|old| {
                (0..2).all(|i| {
                    let width = curves[i].as_ref().domain();
                    let width = *width.end() - *width.start();
                    (old.parameters[i] - candidate.parameters[i]).abs() <= width * 1e-9
                })
            });
            if !duplicate {
                found.push(candidate);
            }
        }
    }
    Ok(found)
}
