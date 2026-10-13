//! Shared periodic basis and homogeneous packing for curves and surface rows.
use super::*;

struct Basis {
    degree: usize,
    count: usize,
    knots: Vec<Real>,
    parameters: Vec<Real>,
    shift: usize,
}

impl Basis {
    fn new(source: &NurbsCurve) -> Result<Self, GeometryError> {
        let degree = source.degree;
        if degree < 2 {
            return Err(GeometryError::PeriodicNurbsDegreeTooLow);
        }
        let count = source.control_points.len();
        let required = if degree == 2 {
            5
        } else {
            degree
                .checked_mul(2)
                .ok_or(GeometryError::PeriodicNurbsDegreeTooLow)?
        };
        if count < required {
            return Err(GeometryError::InsufficientSmoothPeriodicControlPoints {
                degree,
                required,
                actual: count,
            });
        }
        let knots = periodic_knots_preserving_active(degree, count, &source.knots)?;
        let (mut parameters, shift) =
            periodic_greville_parameters(degree, count, count - degree, &knots)?;
        parameters.push(knots[count]);
        Ok(Self {
            degree,
            count,
            knots,
            parameters,
            shift,
        })
    }

    fn finish(
        &self,
        solution: &[Real],
        columns: usize,
        offset: usize,
        scale: Real,
    ) -> Result<NurbsCurve, GeometryError> {
        let unique = self.count - self.degree;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(self.count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "periodic curve controls exceed addressable memory",
            })?;
        for row in 0..unique {
            let values = &solution[row * columns + offset..row * columns + offset + 4];
            let normalized_weight = values[3];
            let weight = normalized_weight * scale;
            let coordinates = [
                values[0] / normalized_weight,
                values[1] / normalized_weight,
                values[2] / normalized_weight,
            ];
            require_finite(
                coordinates.into_iter().chain([normalized_weight, weight]),
                "smooth periodic NURBS controls",
            )?;
            if normalized_weight == 0. || weight == 0. {
                return Err(GeometryError::PeriodicInterpolationSolveFailed);
            }
            controls.push(WeightedPoint3::try_new(
                Point3::try_from(coordinates)?,
                weight,
            )?);
        }
        for index in 0..self.degree {
            controls.push(controls[index]);
        }
        NurbsCurve::try_new_rational(self.degree, controls, self.knots.clone())
    }
}

/// All curves share the same source layout. Each curve keeps its own gauge;
/// matrix rows are stations and columns are consecutive four-channel groups.
pub(crate) fn smooth_curves<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let source = curves.first().ok_or(GeometryError::InvalidControlNet {
        context: "periodic interpolation requires at least one control curve",
    })?;
    let basis = Basis::new(source)?;
    if curves.iter().any(|curve| {
        curve.degree != source.degree
            || curve.knots != source.knots
            || curve.control_points.len() != basis.count
    }) {
        return Err(GeometryError::InvalidControlNet {
            context: "periodic control curves have inconsistent source layouts",
        }
        .into());
    }
    let columns = curves
        .len()
        .checked_mul(4)
        .ok_or(GeometryError::InvalidControlNet {
            context: "periodic interpolation column count overflowed usize",
        })?;
    let target_count = (basis.count - basis.degree).checked_mul(columns).ok_or(
        GeometryError::InvalidControlNet {
            context: "periodic interpolation target count overflowed usize",
        },
    )?;
    let scales = curves
        .iter()
        .map(|curve| {
            curve
                .control_points
                .iter()
                .map(|c| c.weight.abs())
                .fold(0., Real::max)
        })
        .collect::<Vec<_>>();
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(target_count)
        .map_err(|_| GeometryError::InvalidControlNet {
            context: "periodic interpolation targets exceed addressable memory",
        })?;
    for parameter in &basis.parameters[..basis.parameters.len() - 1] {
        for (curve, scale) in curves.iter().zip(&scales) {
            targets.extend(curve.evaluate_scaled_homogeneous(*parameter, *scale)?);
        }
    }
    let solution = solve(
        &crate::NurbsInterpolationSystem {
            degree: basis.degree,
            knots: &basis.knots,
            parameters: &basis.parameters,
            periodic: true,
            columns,
            targets: &targets,
        },
        basis.shift,
    )?;
    if solution.len() != target_count {
        return Err(GeometryError::PeriodicInterpolationSolveFailed.into());
    }
    scales
        .into_iter()
        .enumerate()
        .map(|(curve, scale)| Ok(basis.finish(&solution, columns, curve * 4, scale)?))
        .collect()
}
