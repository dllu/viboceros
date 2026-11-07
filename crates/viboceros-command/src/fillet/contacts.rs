//! Circular tangent contacts without document edits or source reparameterization.
use super::*;
use viboceros_geometry::{CircleTangentConstraint, CircularArc3, Frame3, try_circle_tangent};

pub(super) struct Contacts {
    pub parameters: [Real; 2],
    pub points: [Point3; 2],
    pub arc: CircularArc3,
}

pub(super) fn solve(
    curves: [&Curve3; 2],
    seed: [Real; 2],
    radius: Real,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Contacts, CommandError> {
    let constraints = [0, 1].map(|i| CircleTangentConstraint::Curve {
        curve: curves[i].as_ref(),
        parameter: seed[i],
        fixed: false,
    });
    let solution = try_circle_tangent(&constraints, Some(radius), plane, tolerance)?.ok_or(
        CommandError::Usage("No tangent fillet satisfies these picks"),
    )?;
    let circle = solution.circle;
    let radial = circle.center().vector_to(solution.contact_points[1])?;
    let sweep = radial
        .dot(circle.y_axis().as_vector())?
        .atan2(radial.dot(circle.x_axis().as_vector())?)
        .rem_euclid(std::f64::consts::TAU);
    Ok(Contacts {
        parameters: [0, 1]
            .map(|i| solution.parameters[i].expect("curve constraint has a parameter")),
        points: [solution.contact_points[0], solution.contact_points[1]],
        arc: CircularArc3::try_from_circle_sweep(circle, sweep)?,
    })
}
