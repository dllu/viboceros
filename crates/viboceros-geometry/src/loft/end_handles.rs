//! Prescribe the neighboring control row in the profile's rational basis.
use super::*;
use crate::nurbs::bspline_basis_nonzero;

pub(super) fn match_row<E: From<GeometryError>>(
    profile: &NurbsCurve,
    nominal: Vec<WeightedPoint3>,
    side: usize,
    tangents: &mut LoftEndTangents<'_, E>,
) -> Result<Vec<WeightedPoint3>, E> {
    let degree = profile.degree();
    let knots = profile.knots();
    let boundary = profile.control_points();
    let nominal = NurbsCurve::try_new_rational(degree, nominal, knots.to_vec())?;
    let scale = boundary
        .iter()
        .map(|control| control.weight().abs())
        .fold(0., Real::max);
    let parameters = (0..boundary.len())
        .map(|i| knots[i + 1..=i + degree].iter().sum::<Real>() / degree as Real)
        .collect::<Vec<_>>();
    let mut targets = Vec::with_capacity(4 * boundary.len());
    for &parameter in &parameters {
        let (point, along_profile) = profile.evaluate_with_derivative(parameter)?;
        let transverse = (tangents.direction)(side, point, along_profile)?.normalized_nonzero()?;
        let transverse = transverse.as_vector();
        let offset = point.vector_to(nominal.evaluate(parameter)?)?;
        let distance = offset.length()?.copysign(offset.dot(transverse)?);
        let (first, basis) = bspline_basis_nonzero(knots, degree, boundary.len(), parameter)?;
        let weight = basis
            .iter()
            .zip(&boundary[first..])
            .map(|(basis, control)| basis * (control.weight() / scale))
            .sum::<Real>();
        targets.extend(transverse.scaled(distance * weight)?.to_array());
        targets.push(weight);
    }
    let solution = (tangents.solve_offsets)(&NurbsInterpolationSystem {
        degree,
        knots,
        parameters: &parameters,
        periodic: false,
        columns: 4,
        targets: &targets,
    })?;
    if solution.len() != targets.len() {
        return Err(GeometryError::InvalidControlNet {
            context: "Loft tangent offset solve returned an inconsistent layout",
        }
        .into());
    }
    boundary
        .iter()
        .zip(solution.chunks_exact(4))
        .map(|(control, row)| {
            let weight = control.weight() / scale;
            let offset = Vector3::try_new(row[0] / weight, row[1] / weight, row[2] / weight)?;
            Ok(WeightedPoint3::try_new(
                control.point().translated(offset)?,
                control.weight(),
            )?)
        })
        .collect()
}
