use std::f64::consts::TAU;

use crate::{
    CurveRef, GeometryError, NurbsCurve, Point3, Real, Tolerance, curve::ArcLengthSampler,
    nurbs::bspline_basis_values,
};

/// Largest degree accepted by Rhino's curve rebuild operation.
pub const MAX_CURVE_REBUILD_DEGREE: usize = 11;

/// Largest requested point count accepted by RhinoScript's `RebuildCurve`.
pub const MAX_CURVE_REBUILD_POINT_COUNT: usize = 1_000;

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
    if degree == 0 || degree > MAX_CURVE_REBUILD_DEGREE {
        return Err(GeometryError::InvalidCurveRebuildDegree {
            actual: degree,
            maximum: MAX_CURVE_REBUILD_DEGREE,
        });
    }

    if point_count == 0 || point_count > MAX_CURVE_REBUILD_POINT_COUNT {
        return Err(GeometryError::InvalidCurveRebuildPointCount {
            actual: point_count,
            minimum: 1,
            maximum: MAX_CURVE_REBUILD_POINT_COUNT,
        });
    }

    let closed = source.is_closed()?;
    let point_count = if closed {
        point_count.max(degree).max(3)
    } else {
        point_count.max(degree + 1)
    };
    let mut sampler = ArcLengthSampler::try_new(source, tolerance)?;
    sampler.prepare_repeated_sampling(32)?;
    if closed {
        rebuild_closed(&sampler, point_count, degree)
    } else {
        rebuild_open(&sampler, point_count, degree, preserve_end_tangents)
    }
}

fn rebuild_open(
    sampler: &ArcLengthSampler<'_>,
    point_count: usize,
    degree: usize,
    preserve_end_tangents: bool,
) -> Result<NurbsCurve, GeometryError> {
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

    let rows = parameters
        .into_iter()
        .map(|parameter| bspline_basis_values(&knots, degree, point_count, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let mut controls = solve_banded_collocation(rows, &targets)?;
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

    NurbsCurve::try_new(degree, controls, knots)
}

fn rebuild_closed(
    sampler: &ArcLengthSampler<'_>,
    point_count: usize,
    degree: usize,
) -> Result<NurbsCurve, GeometryError> {
    let knots = periodic_uniform_knots(degree, point_count);
    let control_count = point_count + degree;
    let offset = if degree.is_multiple_of(2) { 0.5 } else { 0.0 };
    let targets = (0..point_count)
        .map(|index| {
            let parameter = index as Real + offset;
            sampler.point_at_distance(sampler.total_length() * (parameter / point_count as Real))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let basis = bspline_basis_values(&knots, degree, control_count, offset)?;
    let mut kernel = vec![0.0; point_count];
    for (control_index, coefficient) in basis.into_iter().enumerate() {
        kernel[control_index % point_count] += coefficient;
    }
    let unique_controls = solve_circulant_collocation(&kernel, &targets)?;
    let mut controls = Vec::with_capacity(control_count);
    controls.extend_from_slice(&unique_controls);
    controls.extend((0..degree).map(|index| unique_controls[index % point_count]));
    NurbsCurve::try_new(degree, controls, knots)
}

fn clamped_uniform_knots(degree: usize, point_count: usize, span_count: usize) -> Vec<Real> {
    let mut knots = Vec::with_capacity(point_count + degree + 1);
    knots.extend(std::iter::repeat_n(0.0, degree + 1));
    knots.extend((1..span_count).map(|index| index as Real));
    knots.extend(std::iter::repeat_n(span_count as Real, degree + 1));
    knots
}

fn periodic_uniform_knots(degree: usize, point_count: usize) -> Vec<Real> {
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
    targets: &[Point3],
) -> Result<Vec<Point3>, GeometryError> {
    let count = rows.len();
    debug_assert_eq!(targets.len(), count);
    debug_assert!(rows.iter().all(|row| row.len() == count));
    let (mut right_hand_side, scale, origin) = normalized_targets(targets);
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

    right_hand_side
        .into_iter()
        .map(|coordinates| point_from_normalized(coordinates, scale, origin))
        .collect()
}

fn solve_circulant_collocation(
    kernel: &[Real],
    targets: &[Point3],
) -> Result<Vec<Point3>, GeometryError> {
    let count = kernel.len();
    debug_assert_eq!(targets.len(), count);
    let (right_hand_side, scale, origin) = normalized_targets(targets);
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
    solution
        .into_iter()
        .map(|coordinates| {
            point_from_normalized(
                coordinates.map(|value| value * inverse_count),
                scale,
                origin,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSegment, Polyline3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn assert_near(actual: Real, expected: Real, epsilon: Real) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "expected {expected:.16e}, got {actual:.16e}"
        );
    }

    fn assert_point_near(actual: Point3, expected: Point3, epsilon: Real) {
        for (actual, expected) in actual.to_array().into_iter().zip(expected.to_array()) {
            assert_near(actual, expected, epsilon);
        }
    }

    #[test]
    fn open_line_matches_rhino_uniform_cubic_structure() {
        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve =
            try_rebuild_curve(CurveRef::Line(&line), 6, 3, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.domain(), 0.0..=3.0);
        assert_eq!(
            curve.knots(),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0]
        );
        for (control, x) in curve.control_points().iter().zip([
            0.0,
            10.0 / 9.0,
            10.0 / 3.0,
            20.0 / 3.0,
            80.0 / 9.0,
            10.0,
        ]) {
            assert_point_near(control.point(), point(x, 0.0, 0.0), 8.0e-15);
            assert_eq!(control.weight(), 1.0);
        }
    }

    #[test]
    fn open_rebuild_interpolates_equal_length_greville_stations() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 4.0, 0.0),
                point(7.0, -2.0, 1.0),
                point(10.0, 0.0, 0.0),
            ],
            vec![5.0, 5.0, 5.0, 5.0, 15.0, 15.0, 15.0, 15.0],
        )
        .unwrap();
        let curve = try_rebuild_curve(
            CurveRef::NurbsCurve(&source),
            6,
            3,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        sampler.prepare_repeated_sampling(32).unwrap();
        for (index, parameter) in [0.0, 1.0 / 3.0, 1.0, 2.0, 8.0 / 3.0, 3.0]
            .into_iter()
            .enumerate()
        {
            let expected = sampler
                .point_at_distance(sampler.total_length() * (parameter / 3.0))
                .unwrap();
            assert_point_near(curve.evaluate(parameter).unwrap(), expected, 2.0e-12);
            if index == 0 || index == 5 {
                assert_eq!(curve.evaluate(parameter).unwrap(), expected);
            }
        }
    }

    #[test]
    fn preserving_tangents_projects_only_the_end_handles() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 4.0, 0.0),
                point(7.0, -2.0, 1.0),
                point(10.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let free = try_rebuild_curve(
            CurveRef::NurbsCurve(&source),
            6,
            3,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let preserved = try_rebuild_curve(
            CurveRef::NurbsCurve(&source),
            6,
            3,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            &free.control_points()[2..4],
            &preserved.control_points()[2..4]
        );
        let source_start = CurveRef::NurbsCurve(&source)
            .evaluate_with_tangent(0.0)
            .unwrap()
            .tangent();
        let source_end = CurveRef::NurbsCurve(&source)
            .evaluate_with_tangent(1.0)
            .unwrap()
            .tangent();
        let rebuilt_start = CurveRef::NurbsCurve(&preserved)
            .evaluate_with_tangent(0.0)
            .unwrap()
            .tangent();
        let rebuilt_end = CurveRef::NurbsCurve(&preserved)
            .evaluate_with_tangent(3.0)
            .unwrap()
            .tangent();
        assert_near(
            source_start
                .as_vector()
                .dot(rebuilt_start.as_vector())
                .unwrap(),
            1.0,
            2.0e-14,
        );
        assert_near(
            source_end.as_vector().dot(rebuilt_end.as_vector()).unwrap(),
            1.0,
            2.0e-14,
        );
    }

    #[test]
    fn closed_rebuilds_use_rhino_periodic_layout_and_arc_stations() {
        let square = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let square_sampler =
            ArcLengthSampler::try_new(CurveRef::Polyline(&square), Tolerance::DEFAULT).unwrap();
        for degree in 1..=5 {
            let curve = try_rebuild_curve(
                CurveRef::Polyline(&square),
                8,
                degree,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(curve.degree(), degree);
            assert_eq!(curve.control_points().len(), 8 + degree);
            assert_eq!(curve.domain(), 0.0..=8.0);
            assert!(curve.is_closed().unwrap());
            assert_eq!(curve.is_periodic(), degree > 1);
            let offset = if degree.is_multiple_of(2) { 0.5 } else { 0.0 };
            for index in 0..8 {
                let parameter = index as Real + offset;
                let expected = square_sampler.point_at_distance(2.0 * parameter).unwrap();
                assert_point_near(curve.evaluate(parameter).unwrap(), expected, 2.0e-12);
            }
        }

        let cubic = try_rebuild_curve(CurveRef::Polyline(&square), 8, 3, false, Tolerance::DEFAULT)
            .unwrap();
        assert_point_near(
            cubic.control_points()[0].point(),
            point(2.0 / 7.0, 2.0, 0.0),
            2.0e-12,
        );
        assert_point_near(
            cubic.control_points()[1].point(),
            point(-4.0 / 7.0, -4.0 / 7.0, 0.0),
            2.0e-12,
        );

        let raised =
            try_rebuild_curve(CurveRef::Polyline(&square), 2, 4, false, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(raised.control_points().len(), 8);
        assert_eq!(raised.domain(), 0.0..=4.0);
    }

    #[test]
    fn validates_degree_and_open_or_closed_point_limits() {
        let line = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            try_rebuild_curve(CurveRef::Line(&line), 6, 0, false, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveRebuildDegree {
                actual: 0,
                maximum: MAX_CURVE_REBUILD_DEGREE,
            })
        );
        let raised =
            try_rebuild_curve(CurveRef::Line(&line), 3, 3, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(raised.control_points().len(), 4);
        assert_eq!(raised.domain(), 0.0..=1.0);
        assert_eq!(
            try_rebuild_curve(CurveRef::Line(&line), 0, 3, false, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveRebuildPointCount {
                actual: 0,
                minimum: 1,
                maximum: MAX_CURVE_REBUILD_POINT_COUNT,
            })
        );
        assert_eq!(
            try_rebuild_curve(
                CurveRef::Line(&line),
                MAX_CURVE_REBUILD_POINT_COUNT + 1,
                3,
                false,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveRebuildPointCount {
                actual: MAX_CURVE_REBUILD_POINT_COUNT + 1,
                minimum: 1,
                maximum: MAX_CURVE_REBUILD_POINT_COUNT,
            })
        );
    }

    #[test]
    fn thousand_point_open_rebuild_remains_bounded_and_exact_for_a_line() {
        let offset = 1.0e12;
        let line = LineSegment::try_new(
            point(offset - 5.0, offset + 2.0, offset + 1.0),
            point(offset + 9.0, offset - 3.0, offset + 4.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = try_rebuild_curve(
            CurveRef::Line(&line),
            MAX_CURVE_REBUILD_POINT_COUNT,
            MAX_CURVE_REBUILD_DEGREE,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(curve.control_points().len(), MAX_CURVE_REBUILD_POINT_COUNT);
        assert_point_near(
            curve.evaluate(*curve.domain().start()).unwrap(),
            line.start(),
            2.0e-12,
        );
        assert_point_near(
            curve.evaluate(*curve.domain().end()).unwrap(),
            line.end(),
            2.0e-12,
        );
        assert_point_near(
            curve.evaluate(0.5 * *curve.domain().end()).unwrap(),
            point(offset + 2.0, offset - 0.5, offset + 2.5),
            2.0e-3,
        );
    }

    #[test]
    fn thousand_point_closed_rebuild_is_stable_far_from_the_origin() {
        let offset = 1.0e12;
        let square = Polyline3::try_new(
            vec![
                point(offset, offset, offset),
                point(offset + 400.0, offset, offset),
                point(offset + 400.0, offset + 400.0, offset),
                point(offset, offset + 400.0, offset),
                point(offset, offset, offset),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = try_rebuild_curve(
            CurveRef::Polyline(&square),
            MAX_CURVE_REBUILD_POINT_COUNT,
            MAX_CURVE_REBUILD_DEGREE,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            curve.control_points().len(),
            MAX_CURVE_REBUILD_POINT_COUNT + MAX_CURVE_REBUILD_DEGREE
        );
        assert!(curve.is_periodic());

        let sampler =
            ArcLengthSampler::try_new(CurveRef::Polyline(&square), Tolerance::DEFAULT).unwrap();
        for index in [0, 137, 500, 999] {
            let expected = sampler
                .point_at_distance(
                    sampler.total_length()
                        * (index as Real / MAX_CURVE_REBUILD_POINT_COUNT as Real),
                )
                .unwrap();
            assert_point_near(curve.evaluate(index as Real).unwrap(), expected, 2.0e-3);
        }
    }
}
