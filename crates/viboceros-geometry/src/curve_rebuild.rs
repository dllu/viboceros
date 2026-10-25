use std::f64::consts::TAU;

use crate::{
    CurveRef, GeometryError, NurbsCurve, Point3, Real, Tolerance, curve::ArcLengthSampler,
    nurbs::bspline_basis_values,
};

/// Largest degree accepted by Rhino's curve rebuild operation.
pub const MAX_CURVE_REBUILD_DEGREE: usize = 11;

/// Largest requested point count accepted by RhinoScript's `RebuildCurve`.
pub const MAX_CURVE_REBUILD_POINT_COUNT: usize = 1_000;

/// Uniform Rebuild collocation, shared with external basis/solve backends.
/// Targets and returned controls are normalized Cartesian offsets. Periodic
/// parameters include the final domain endpoint, without a repeated target.
pub struct CurveRebuildInterpolation<'a> {
    pub degree: usize,
    pub knots: &'a [Real],
    pub parameters: &'a [Real],
    pub periodic: bool,
    pub targets: &'a [[Real; 3]],
}

/// Reconstructs a curve as a non-rational, uniform NURBS curve with a requested
/// degree and point count.
///
/// Open curves interpolate equal-arc-length source locations at the Greville
/// parameters of a clamped uniform target. Closed curves interpolate one
/// circuit of equally spaced locations with a periodic uniform target; even
/// degrees use the half-span seam offset required by the periodic Greville
/// locations. This reproduces Rhino's `NurbsCurve.Rebuild` structure. When
/// requested, the two open-curve handle controls are projected onto the source
/// end tangent directions after interpolation. Undersized positive point
/// counts are raised to `degree + 1` for open curves or `max(3, degree)` for
/// closed curves, matching Rhino's automatic option adjustment.
pub fn try_rebuild_curve(
    source: CurveRef<'_>,
    point_count: usize,
    degree: usize,
    preserve_end_tangents: bool,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_rebuild_curve_with_interpolation(
        source,
        point_count,
        degree,
        preserve_end_tangents,
        tolerance,
        &mut solve_portable_rebuild,
    )
}

/// Retain Rebuild sampling, basis and tangent policy while delegating the
/// normalized control solve to the caller's backend.
pub fn try_rebuild_curve_with_interpolation<E: From<GeometryError>>(
    source: CurveRef<'_>,
    point_count: usize,
    degree: usize,
    preserve_end_tangents: bool,
    tolerance: Tolerance,
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<NurbsCurve, E> {
    rebuild_curve(
        source,
        point_count,
        degree,
        preserve_end_tangents,
        tolerance,
        false,
        solve,
    )
}

/// Rebuild on a clamped basis, including geometrically closed sources. This
/// preserves the supplied seam without imposing periodic seam derivatives.
pub fn try_rebuild_open_curve(
    source: CurveRef<'_>,
    point_count: usize,
    degree: usize,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_rebuild_open_curve_with_interpolation(
        source,
        point_count,
        degree,
        tolerance,
        &mut solve_portable_rebuild,
    )
}

/// Delegate the existing arc-length Greville solve for a clamped rebuild.
pub fn try_rebuild_open_curve_with_interpolation<E: From<GeometryError>>(
    source: CurveRef<'_>,
    point_count: usize,
    degree: usize,
    tolerance: Tolerance,
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<NurbsCurve, E> {
    rebuild_curve(source, point_count, degree, false, tolerance, true, solve)
}

fn rebuild_curve<E: From<GeometryError>>(
    source: CurveRef<'_>,
    point_count: usize,
    degree: usize,
    preserve_end_tangents: bool,
    tolerance: Tolerance,
    force_open: bool,
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<NurbsCurve, E> {
    if degree == 0 || degree > MAX_CURVE_REBUILD_DEGREE {
        return Err(GeometryError::InvalidCurveRebuildDegree {
            actual: degree,
            maximum: MAX_CURVE_REBUILD_DEGREE,
        }
        .into());
    }

    if point_count == 0 || point_count > MAX_CURVE_REBUILD_POINT_COUNT {
        return Err(GeometryError::InvalidCurveRebuildPointCount {
            actual: point_count,
            minimum: 1,
            maximum: MAX_CURVE_REBUILD_POINT_COUNT,
        }
        .into());
    }

    let closed = !force_open && source.is_closed()?;
    let point_count = if closed {
        point_count.max(degree).max(3)
    } else {
        point_count.max(degree + 1)
    };
    let mut sampler = ArcLengthSampler::try_new(source, tolerance)?;
    sampler.prepare_budgeted_repeated_sampling(32)?;
    if closed {
        rebuild_closed(&sampler, point_count, degree, solve)
    } else {
        rebuild_open(&sampler, point_count, degree, preserve_end_tangents, solve)
    }
}

fn rebuild_open<E: From<GeometryError>>(
    sampler: &ArcLengthSampler<'_>,
    point_count: usize,
    degree: usize,
    preserve_end_tangents: bool,
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<NurbsCurve, E> {
    let span_count = point_count - degree;
    let knots = clamped_uniform_knots(degree, point_count, span_count);
    let mut parameters = Vec::with_capacity(point_count);
    let mut targets = Vec::with_capacity(point_count);
    for control_index in 0..point_count {
        let parameter = greville_parameter(&knots, degree, control_index);
        parameters.push(parameter);
        targets.push(
            sampler.point_at_distance(sampler.total_length() * (parameter / span_count as Real))?,
        );
    }

    let mut controls = solve_rebuild_targets(degree, &knots, &parameters, false, &targets, solve)?;
    // Endpoint collocation rows are identities. Assigning the exact sampled
    // points also avoids retaining inconsequential solver roundoff there.
    controls[0] = targets[0];
    controls[point_count - 1] = targets[point_count - 1];

    if preserve_end_tangents && degree >= 2 && point_count >= 4 {
        let start_tangent = sampler.sample_at_distance(0.0)?.tangent();
        let end_tangent = sampler
            .sample_at_distance(sampler.total_length())?
            .tangent();
        let start_projection = controls[0]
            .vector_to(controls[1])?
            .dot(start_tangent.as_vector())?;
        controls[1] =
            controls[0].translated(start_tangent.as_vector().scaled(start_projection)?)?;
        let end_projection = controls[point_count - 1]
            .vector_to(controls[point_count - 2])?
            .dot(end_tangent.as_vector())?;
        controls[point_count - 2] = controls[point_count - 1]
            .translated(end_tangent.as_vector().scaled(end_projection)?)?;
    }

    Ok(NurbsCurve::try_new(degree, controls, knots)?)
}

/// Interpolates the source at affine parameter locations corresponding to a
/// clamped uniform curve's Greville abscissae. Match uses this when it needs an
/// extra control while keeping the source end-handle lengths determined by
/// Rhino's uniform preparation step. Unlike `try_rebuild_curve`, these targets
/// follow the source parameter rather than its arc length.
pub(crate) fn interpolate_affine_greville(
    source: &NurbsCurve,
    point_count: usize,
) -> Result<NurbsCurve, GeometryError> {
    try_interpolate_affine_greville_with_solver(source, point_count, &mut solve_portable_rebuild)
}

/// Match's affine parameter preparation, retaining uniform target knots and
/// pinned endpoints while delegating only normalized control interpolation.
pub fn try_interpolate_affine_greville_with_solver<E: From<GeometryError>>(
    source: &NurbsCurve,
    point_count: usize,
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<NurbsCurve, E> {
    let degree = source.degree();
    if point_count <= degree || point_count > MAX_CURVE_REBUILD_POINT_COUNT {
        return Err(GeometryError::InvalidCurveRebuildPointCount {
            actual: point_count,
            minimum: degree + 1,
            maximum: MAX_CURVE_REBUILD_POINT_COUNT,
        }
        .into());
    }
    let span_count = point_count - degree;
    let knots = clamped_uniform_knots(degree, point_count, span_count);
    let domain = source.domain();
    let start = *domain.start();
    let length = *domain.end() - start;
    let mut targets = Vec::with_capacity(point_count);
    let mut parameters = Vec::with_capacity(point_count);
    for control_index in 0..point_count {
        let parameter = greville_parameter(&knots, degree, control_index);
        let source_parameter = if control_index + 1 == point_count {
            *domain.end()
        } else {
            (parameter / span_count as Real).mul_add(length, start)
        };
        targets.push(source.evaluate(source_parameter)?);
        parameters.push(parameter);
    }
    let mut controls = solve_rebuild_targets(degree, &knots, &parameters, false, &targets, solve)?;
    controls[0] = targets[0];
    controls[point_count - 1] = targets[point_count - 1];
    Ok(NurbsCurve::try_new(degree, controls, knots)?)
}

fn rebuild_closed<E: From<GeometryError>>(
    sampler: &ArcLengthSampler<'_>,
    point_count: usize,
    degree: usize,
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<NurbsCurve, E> {
    let knots = periodic_uniform_knots(degree, point_count);
    let control_count = point_count + degree;
    let offset = if degree.is_multiple_of(2) { 0.5 } else { 0.0 };
    let targets = (0..point_count)
        .map(|index| {
            let parameter = index as Real + offset;
            sampler.point_at_distance(sampler.total_length() * (parameter / point_count as Real))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut parameters = (0..point_count)
        .map(|i| i as Real + offset)
        .collect::<Vec<_>>();
    parameters.push(point_count as Real);
    let unique_controls =
        solve_rebuild_targets(degree, &knots, &parameters, true, &targets, solve)?;
    let mut controls = Vec::with_capacity(control_count);
    controls.extend_from_slice(&unique_controls);
    controls.extend((0..degree).map(|index| unique_controls[index % point_count]));
    Ok(NurbsCurve::try_new(degree, controls, knots)?)
}

fn solve_rebuild_targets<E: From<GeometryError>>(
    degree: usize,
    knots: &[Real],
    parameters: &[Real],
    periodic: bool,
    targets: &[Point3],
    solve: &mut impl FnMut(&CurveRebuildInterpolation<'_>) -> Result<Vec<[Real; 3]>, E>,
) -> Result<Vec<Point3>, E> {
    let (normalized, scale, origin) = normalized_targets(targets);
    let input = CurveRebuildInterpolation {
        degree,
        knots,
        parameters,
        periodic,
        targets: &normalized,
    };
    let controls = solve(&input)?;
    if controls.len() != targets.len() {
        return Err(GeometryError::InvalidControlNet {
            context: "curve rebuild solver returned an inconsistent layout",
        }
        .into());
    }
    Ok(controls
        .into_iter()
        .map(|p| point_from_normalized(p, scale, origin))
        .collect::<Result<_, _>>()?)
}

/// Interpolate supplied targets at a clamped uniform basis's Greville sites.
/// Circle's low-count approximation samples angles by index rather than by
/// these parameters; retain the shared normalized collocation and solve.
pub(crate) fn interpolate_clamped_greville_targets(
    degree: usize,
    targets: &[Point3],
) -> Result<NurbsCurve, GeometryError> {
    if degree == 0 || degree > MAX_CURVE_REBUILD_DEGREE {
        return Err(GeometryError::InvalidCurveRebuildDegree {
            actual: degree,
            maximum: MAX_CURVE_REBUILD_DEGREE,
        });
    }
    if targets.len() <= degree || targets.len() > MAX_CURVE_REBUILD_POINT_COUNT + 1 {
        return Err(GeometryError::InvalidControlNet {
            context: "clamped Greville interpolation target count",
        });
    }
    let knots = clamped_uniform_knots(degree, targets.len(), targets.len() - degree);
    let parameters = (0..targets.len())
        .map(|i| greville_parameter(&knots, degree, i))
        .collect::<Vec<_>>();
    let mut controls = solve_rebuild_targets(
        degree,
        &knots,
        &parameters,
        false,
        targets,
        &mut solve_portable_rebuild,
    )?;
    controls[0] = targets[0];
    let last = controls.len() - 1;
    controls[last] = targets[last];
    NurbsCurve::try_new(degree, controls, knots)
}

fn solve_portable_rebuild(
    input: &CurveRebuildInterpolation<'_>,
) -> Result<Vec<[Real; 3]>, GeometryError> {
    let n = input.targets.len();
    if input.periodic {
        let basis = bspline_basis_values(
            input.knots,
            input.degree,
            n + input.degree,
            input.parameters[0],
        )?;
        let mut kernel = vec![0.0; n];
        for (i, coefficient) in basis.into_iter().enumerate() {
            kernel[i % n] += coefficient;
        }
        solve_circulant_collocation(&kernel, input.targets)
    } else {
        let rows = input
            .parameters
            .iter()
            .map(|t| bspline_basis_values(input.knots, input.degree, n, *t))
            .collect::<Result<Vec<_>, _>>()?;
        solve_banded_collocation(rows, input.targets)
    }
}

fn clamped_uniform_knots(degree: usize, point_count: usize, span_count: usize) -> Vec<Real> {
    let mut knots = Vec::with_capacity(point_count + degree + 1);
    knots.extend(std::iter::repeat_n(0.0, degree + 1));
    knots.extend((1..span_count).map(|index| index as Real));
    knots.extend(std::iter::repeat_n(span_count as Real, degree + 1));
    knots
}

pub(crate) fn periodic_uniform_knots(degree: usize, point_count: usize) -> Vec<Real> {
    let short_count = point_count + 2 * degree - 1;
    let first = -((degree - 1) as Real);
    let last = first + (short_count - 1) as Real;
    let mut knots = Vec::with_capacity(short_count + 2);
    knots.push(first);
    knots.extend((0..short_count).map(|index| first + index as Real));
    knots.push(last);
    knots
}

fn greville_parameter(knots: &[Real], degree: usize, control_index: usize) -> Real {
    knots[control_index + 1..=control_index + degree]
        .iter()
        .sum::<Real>()
        / degree as Real
}

fn normalized_targets(targets: &[Point3]) -> (Vec<[Real; 3]>, Real, [Real; 3]) {
    let origin = targets[0].to_array();
    let scale = targets
        .iter()
        .flat_map(|point| {
            let point = point.to_array();
            std::array::from_fn::<_, 3, _>(|coordinate| point[coordinate] - origin[coordinate])
        })
        .map(Real::abs)
        .fold(0.0_f64, Real::max)
        .max(1.0);
    (
        targets
            .iter()
            .map(|point| {
                let point = point.to_array();
                std::array::from_fn(|coordinate| (point[coordinate] - origin[coordinate]) / scale)
            })
            .collect(),
        scale,
        origin,
    )
}

fn point_from_normalized(
    coordinates: [Real; 3],
    scale: Real,
    origin: [Real; 3],
) -> Result<Point3, GeometryError> {
    Point3::try_new(
        coordinates[0].mul_add(scale, origin[0]),
        coordinates[1].mul_add(scale, origin[1]),
        coordinates[2].mul_add(scale, origin[2]),
    )
}

fn solve_banded_collocation(
    mut rows: Vec<Vec<Real>>,
    targets: &[[Real; 3]],
) -> Result<Vec<[Real; 3]>, GeometryError> {
    let count = rows.len();
    debug_assert_eq!(targets.len(), count);
    debug_assert!(rows.iter().all(|row| row.len() == count));
    let mut right_hand_side = targets.to_vec();
    let mut lower_bandwidth = 0;
    let mut upper_bandwidth = 0;
    for (row_index, row) in rows.iter().enumerate() {
        for (column_index, coefficient) in row.iter().enumerate() {
            if *coefficient != 0.0 {
                lower_bandwidth = lower_bandwidth.max(row_index.saturating_sub(column_index));
                upper_bandwidth = upper_bandwidth.max(column_index.saturating_sub(row_index));
            }
        }
    }

    for pivot_index in 0..count {
        let pivot = rows[pivot_index][pivot_index];
        if !pivot.is_finite() || pivot == 0.0 {
            return Err(GeometryError::CurveRebuildSolveFailed);
        }
        let final_row = (pivot_index + lower_bandwidth).min(count - 1);
        let final_column = (pivot_index + upper_bandwidth).min(count - 1);
        let band_start = pivot_index + 1;
        let band_end = final_column + 1;
        let pivot_band = rows[pivot_index][band_start..band_end].to_vec();
        let pivot_target = right_hand_side[pivot_index];
        for row_index in pivot_index + 1..=final_row {
            let factor = rows[row_index][pivot_index] / pivot;
            if factor == 0.0 {
                continue;
            }
            if !factor.is_finite() {
                return Err(GeometryError::CurveRebuildSolveFailed);
            }
            rows[row_index][pivot_index] = 0.0;
            for (value, pivot_value) in rows[row_index][band_start..band_end]
                .iter_mut()
                .zip(&pivot_band)
            {
                *value = (-factor).mul_add(*pivot_value, *value);
            }
            for (value, pivot_value) in right_hand_side[row_index].iter_mut().zip(pivot_target) {
                *value = (-factor).mul_add(pivot_value, *value);
            }
        }
    }

    for row_index in (0..count).rev() {
        let final_column = (row_index + upper_bandwidth).min(count - 1);
        let mut target = right_hand_side[row_index];
        for (coefficient, solved) in rows[row_index][row_index + 1..final_column + 1]
            .iter()
            .zip(&right_hand_side[row_index + 1..final_column + 1])
        {
            for (value, solved_value) in target.iter_mut().zip(solved) {
                *value = (-*coefficient).mul_add(*solved_value, *value);
            }
        }
        let pivot = rows[row_index][row_index];
        for value in &mut target {
            *value /= pivot;
            if !value.is_finite() {
                return Err(GeometryError::CurveRebuildSolveFailed);
            }
        }
        right_hand_side[row_index] = target;
    }

    Ok(right_hand_side)
}

fn solve_circulant_collocation(
    kernel: &[Real],
    targets: &[[Real; 3]],
) -> Result<Vec<[Real; 3]>, GeometryError> {
    let count = kernel.len();
    debug_assert_eq!(targets.len(), count);
    let right_hand_side = targets;
    let mut spectrum = Vec::with_capacity(count);

    for frequency in 0..count {
        let angle = TAU * (frequency as Real / count as Real);
        let (step_sine, step_cosine) = angle.sin_cos();
        let mut cosine = 1.0;
        let mut sine = 0.0;
        let mut eigenvalue = [0.0, 0.0];
        let mut transformed = [[0.0, 0.0]; 3];
        for index in 0..count {
            eigenvalue[0] += kernel[index] * cosine;
            eigenvalue[1] += kernel[index] * sine;
            for coordinate in 0..3 {
                transformed[coordinate][0] += right_hand_side[index][coordinate] * cosine;
                transformed[coordinate][1] -= right_hand_side[index][coordinate] * sine;
            }
            let next_cosine = cosine.mul_add(step_cosine, -sine * step_sine);
            sine = sine.mul_add(step_cosine, cosine * step_sine);
            cosine = next_cosine;
        }

        let denominator = eigenvalue[0].mul_add(eigenvalue[0], eigenvalue[1] * eigenvalue[1]);
        if !denominator.is_finite() || denominator <= Real::MIN_POSITIVE {
            return Err(GeometryError::CurveRebuildSolveFailed);
        }
        for value in &mut transformed {
            let real = value[0];
            let imaginary = value[1];
            value[0] = real.mul_add(eigenvalue[0], imaginary * eigenvalue[1]) / denominator;
            value[1] = imaginary.mul_add(eigenvalue[0], -real * eigenvalue[1]) / denominator;
        }
        spectrum.push(transformed);
    }

    let mut solution = vec![[0.0; 3]; count];
    for (frequency, transformed) in spectrum.into_iter().enumerate() {
        let angle = TAU * (frequency as Real / count as Real);
        let (step_sine, step_cosine) = angle.sin_cos();
        let mut cosine = 1.0;
        let mut sine = 0.0;
        for coordinates in &mut solution {
            for coordinate in 0..3 {
                coordinates[coordinate] +=
                    transformed[coordinate][0].mul_add(cosine, -transformed[coordinate][1] * sine);
            }
            let next_cosine = cosine.mul_add(step_cosine, -sine * step_sine);
            sine = sine.mul_add(step_cosine, cosine * step_sine);
            cosine = next_cosine;
        }
    }

    let inverse_count = 1.0 / count as Real;
    Ok(solution
        .into_iter()
        .map(|p| p.map(|value| value * inverse_count))
        .collect())
}

#[cfg(test)]
mod tests;
