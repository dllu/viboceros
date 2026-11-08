//! Consistent scan slabs, endpoint events and closed trim-region intervals.
use super::*;
#[cfg(test)]
mod tests;

pub(super) fn trimmed_isocurve_intervals(
    face: &BrepFace,
    varying_axis: usize,
    fixed_value: Real,
    tolerance: Tolerance,
) -> Result<Vec<[Real; 2]>, GeometryError> {
    debug_assert!(varying_axis < 2);
    require_finite([fixed_value], "B-rep isocurve parameter")?;
    let fixed_axis = 1 - varying_axis;
    let varying_domain = if varying_axis == 0 {
        face.surface.domain_u()
    } else {
        face.surface.domain_v()
    };
    let fixed_domain = if fixed_axis == 0 {
        face.surface.domain_u()
    } else {
        face.surface.domain_v()
    };
    if fixed_value < *fixed_domain.start() || fixed_value > *fixed_domain.end() {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter: fixed_value,
            domain_start: *fixed_domain.start(),
            domain_end: *fixed_domain.end(),
        });
    }

    let mut epsilon =
        trim_parameter_epsilon([*varying_domain.start(), *varying_domain.end()], tolerance);
    let mut fixed_epsilon =
        trim_parameter_epsilon([*fixed_domain.start(), *fixed_domain.end()], tolerance);
    let mut events = Vec::new();
    let mut overlaps = Vec::new();
    for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
        epsilon = epsilon.max(trim.tolerance[varying_axis]);
        fixed_epsilon = fixed_epsilon.max(trim.tolerance[fixed_axis]);
    }
    for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
        collect_trim_scan_data(
            &trim.curve,
            fixed_axis,
            varying_axis,
            fixed_value,
            fixed_epsilon,
            &mut events,
            &mut overlaps,
        )?;
    }

    let domain = [*varying_domain.start(), *varying_domain.end()];
    events.retain(|value| parameter_interval_contains(domain, *value, epsilon));
    for value in &mut events {
        *value = value.clamp(domain[0], domain[1]);
    }
    events.sort_by(Real::total_cmp);

    let mut intervals = Vec::new();
    let mut inside_start = None;
    let mut index = 0;
    while index < events.len() {
        let mut after = index + 1;
        while after < events.len() && events[after] - events[index] <= epsilon {
            after += 1;
        }
        if (after - index) % 2 == 1 {
            let crossing = events[index].midpoint(events[after - 1]);
            if let Some(start) = inside_start.take() {
                if start < crossing {
                    intervals.push([start, crossing]);
                }
            } else {
                inside_start = Some(crossing);
            }
        }
        index = after;
    }
    if let Some(start) = inside_start
        && start < domain[1]
    {
        intervals.push([start, domain[1]]);
    }

    for mut overlap in overlaps {
        overlap.sort_by(Real::total_cmp);
        overlap[0] = overlap[0].clamp(domain[0], domain[1]);
        overlap[1] = overlap[1].clamp(domain[0], domain[1]);
        if overlap[0] < overlap[1] {
            intervals.push(overlap);
        }
    }
    merge_trim_intervals(intervals, epsilon)
}

pub(super) fn parameter_interval_contains(interval: [Real; 2], value: Real, epsilon: Real) -> bool {
    (value >= interval[0] || interval[0] - value <= epsilon)
        && (value <= interval[1] || value - interval[1] <= epsilon)
}

fn merge_trim_intervals(
    mut intervals: Vec<[Real; 2]>,
    epsilon: Real,
) -> Result<Vec<[Real; 2]>, GeometryError> {
    intervals.sort_by(|left, right| {
        left[0]
            .total_cmp(&right[0])
            .then_with(|| left[1].total_cmp(&right[1]))
    });
    let mut merged: Vec<[Real; 2]> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        if let Some(previous) = merged.last_mut()
            && interval[0] - previous[1] <= epsilon
        {
            previous[1] = previous[1].max(interval[1]);
        } else {
            merged.push(interval);
        }
    }
    if merged.iter().any(|interval| {
        !interval[0].is_finite() || !interval[1].is_finite() || interval[0] >= interval[1]
    }) {
        return Err(GeometryError::TrimIntersectionDidNotConverge);
    }
    Ok(merged)
}

fn collect_trim_scan_data(
    curve: &NurbsCurve2,
    fixed_axis: usize,
    varying_axis: usize,
    fixed_value: Real,
    fixed_epsilon: Real,
    events: &mut Vec<Real>,
    overlaps: &mut Vec<[Real; 2]>,
) -> Result<(), GeometryError> {
    let spans = scalar_bezier_spans(curve, fixed_axis, fixed_value)?;
    // Scalar Bezier coefficients are coordinate differences divided by the
    // largest coordinate, then weighted by each control's relative weight.
    // A bound using the smallest positive weight proves the entire rational
    // span lies inside the scan slab. This catches trim edges at a padded
    // planar support surface's boundary without treating other spans as
    // collinear.
    let coordinate_scale = curve
        .control_points()
        .iter()
        .map(|control| parameter_coordinate(control.point(), fixed_axis).abs())
        .fold(fixed_value.abs(), Real::max)
        .max(1.0);
    let weight_min = curve
        .control_points()
        .iter()
        .map(|control| control.weight())
        .fold(Real::INFINITY, Real::min);
    let weight_max = curve
        .control_points()
        .iter()
        .map(|control| control.weight())
        .fold(0.0, Real::max);
    let near_zero = (fixed_epsilon / coordinate_scale) * (weight_min / weight_max);
    let seam_overlap = spans.first().into_iter().chain(spans.last()).any(|span| {
        span.coefficients
            .iter()
            .all(|coefficient| *coefficient == 0.)
    });
    let mut roots = Vec::new();
    for span in spans {
        if span
            .coefficients
            .iter()
            .all(|coefficient| coefficient.abs() <= near_zero)
        {
            let start = curve.evaluate(span.parameter[0])?;
            let end = curve.evaluate(span.parameter[1])?;
            overlaps.push([
                parameter_coordinate(start, varying_axis),
                parameter_coordinate(end, varying_axis),
            ]);
        } else {
            collect_bernstein_roots(
                &span.coefficients,
                span.parameter,
                0,
                true,
                true,
                &mut roots,
            );
        }
    }

    let curve_domain = curve.domain();
    // A span within the scan slab is represented as an overlap above. Its
    // incident endpoints must use that same slab: mixing exact endpoint signs
    // with tolerant overlaps can leave an unmatched crossing and remove a
    // whole region far beyond a nearly horizontal hole edge.
    for parameter in [*curve_domain.start(), *curve_domain.end()] {
        let coordinate = parameter_coordinate(curve.evaluate(parameter)?, fixed_axis);
        if (coordinate - fixed_value).abs() <= fixed_epsilon {
            roots.push(parameter);
        }
    }
    roots.sort_by(Real::total_cmp);
    let root_epsilon = floating_parameter_epsilon([*curve_domain.start(), *curve_domain.end()]);
    let mut unique_roots = Vec::with_capacity(roots.len());
    for root in roots {
        let root = if root - *curve_domain.start() <= root_epsilon {
            *curve_domain.start()
        } else if *curve_domain.end() - root <= root_epsilon {
            *curve_domain.end()
        } else {
            root
        };
        if unique_roots
            .last()
            .is_none_or(|previous| root - *previous > root_epsilon)
        {
            unique_roots.push(root);
        }
    }

    let closed = seam_overlap
        && curve.evaluate(*curve_domain.start())? == curve.evaluate(*curve_domain.end())?;
    for (index, root) in unique_roots.iter().copied().enumerate() {
        if closed && root == *curve_domain.end() {
            continue;
        }
        let before_bound = index
            .checked_sub(1)
            .map_or(*curve_domain.start(), |previous| unique_roots[previous]);
        let after_bound = unique_roots
            .get(index + 1)
            .copied()
            .unwrap_or(*curve_domain.end());
        let before = (root > *curve_domain.start())
            .then(|| coordinate_sign_between(curve, fixed_axis, fixed_value, before_bound, root))
            .transpose()?
            .flatten();
        let after = (root < *curve_domain.end())
            .then(|| coordinate_sign_between(curve, fixed_axis, fixed_value, root, after_bound))
            .transpose()?
            .flatten();
        let root_coordinate = parameter_coordinate(curve.evaluate(root)?, fixed_axis);
        let at_root = if (root_coordinate - fixed_value).abs() <= fixed_epsilon {
            None
        } else if root_coordinate < fixed_value {
            Some(-1)
        } else if root_coordinate > fixed_value {
            Some(1)
        } else {
            None
        };
        let toggles = if closed && root == *curve_domain.start() {
            let previous = unique_roots
                .iter()
                .rev()
                .copied()
                .find(|p| *p < *curve_domain.end())
                .unwrap_or(*curve_domain.start());
            let wrapped_before = coordinate_sign_between(
                curve,
                fixed_axis,
                fixed_value,
                previous,
                *curve_domain.end(),
            )?;
            matches!(
                (wrapped_before, after),
                (Some(-1), Some(1)) | (Some(1), Some(-1))
            )
        } else if root == *curve_domain.start() {
            at_root.map_or(after == Some(1), |at_root| {
                after.is_some_and(|after| after != at_root)
            })
        } else if root == *curve_domain.end() {
            at_root.map_or(before == Some(1), |at_root| {
                before.is_some_and(|before| before != at_root)
            })
        } else {
            matches!((before, after), (Some(-1), Some(1)) | (Some(1), Some(-1)))
        };
        if toggles {
            let point = curve.evaluate(root)?;
            events.push(parameter_coordinate(point, varying_axis));
        }
    }
    Ok(())
}

fn coordinate_sign_between(
    curve: &NurbsCurve2,
    fixed_axis: usize,
    fixed_value: Real,
    start: Real,
    end: Real,
) -> Result<Option<i8>, GeometryError> {
    for fraction in [0.5, 0.25, 0.75, 0.125, 0.875] {
        let parameter = start.mul_add(1.0 - fraction, end * fraction);
        if parameter <= start || parameter >= end {
            continue;
        }
        let coordinate = parameter_coordinate(curve.evaluate(parameter)?, fixed_axis);
        if coordinate < fixed_value {
            return Ok(Some(-1));
        }
        if coordinate > fixed_value {
            return Ok(Some(1));
        }
    }
    Ok(None)
}
