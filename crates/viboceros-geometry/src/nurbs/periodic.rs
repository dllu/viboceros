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
        Self::with_knots(degree, count, &source.knots)
    }
    fn with_knots(
        degree: usize,
        count: usize,
        source_knots: &[Real],
    ) -> Result<Self, GeometryError> {
        if count < 2 * degree || degree < 2 {
            return Err(GeometryError::NoRemovableKnot {
                parameter: source_knots[degree],
            });
        }
        let knots = periodic_knots_preserving_active(degree, count, source_knots)?;
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
    fit_curves(curves, Basis::new(source)?, solve)
}
fn fit_curves<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    basis: Basis,
    solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let source = &curves[0];

    if curves.iter().any(|curve| {
        curve.degree != source.degree
            || curve.knots != source.knots
            || curve.control_points.len() != source.control_points.len()
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

impl NurbsCurve {
    /// Remove one interior knot while retaining the cyclic basis and repeated
    /// controls. Replacement controls interpolate the source at target Greville sites.
    pub fn try_remove_periodic_knot_with_solver<E: From<GeometryError>>(
        &self,
        parameter: Real,
        solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        self.validate_parameter(parameter)?;
        if !self.is_periodic() {
            return Err(GeometryError::InvalidControlNet {
                context: "periodic removal requires a periodic curve",
            }
            .into());
        }
        let index = self.closest_active_knot_index_by_curve_point(parameter)?;
        let knot = removable_knot_at_index(self.degree, &self.knots, index, parameter)?;
        self.try_remove_periodic_knot_value_with_solver(knot, solve)
    }
    /// Fit after removing an explicitly selected cyclic knot value.
    pub fn try_remove_periodic_knot_value_with_solver<E: From<GeometryError>>(
        &self,
        knot: Real,
        solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        if !self.is_periodic()
            || !knot.is_finite()
            || knot <= *self.domain().start()
            || knot >= *self.domain().end()
            || !self.knots.contains(&knot)
        {
            return Err(GeometryError::NoRemovableKnot { parameter: knot }.into());
        }
        let mut knots = self.knots.clone();
        let index = knots.partition_point(|k| *k <= knot) - 1;
        knots.remove(index);
        let basis = Basis::with_knots(self.degree, self.control_points.len() - 1, &knots)?;
        Ok(fit_curves(std::slice::from_ref(self), basis, solve)?.remove(0))
    }
    pub fn try_remove_periodic_knot_value(&self, knot: Real) -> Result<Self, GeometryError> {
        self.try_remove_periodic_knot_value_with_solver(knot, &mut portable_periodic_interpolation)
    }
    pub fn try_remove_periodic_knot(&self, parameter: Real) -> Result<Self, GeometryError> {
        self.try_remove_periodic_knot_with_solver(parameter, &mut portable_periodic_interpolation)
    }
}
