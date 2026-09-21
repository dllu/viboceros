use super::*;

pub(super) fn intervals(
    a: &NurbsCurve,
    b: &NurbsCurve,
    distance: Real,
    budget: &mut Budget,
) -> Result<Option<[[Real; 2]; 2]>, GeometryError> {
    let (Some(a_end), Some(b_end)) = (
        certificate::linear_endpoints(a),
        certificate::linear_endpoints(b),
    ) else {
        return Ok(None);
    };
    let [start, end] = a_end.map(Point3::to_array);
    let b_coords = b_end.map(Point3::to_array);
    let axis = (0..3)
        .max_by(|&i, &j| {
            (end[i] - start[i])
                .abs()
                .total_cmp(&(end[j] - start[j]).abs())
        })
        .unwrap();
    let (b0, b1) = (b_coords[0][axis], b_coords[1][axis]);
    if b0 == b1 {
        return Ok(None);
    }
    // Original scalar endpoints make point-only contact exactly zero extent;
    // no projection or tolerance-expanded interval invents a tiny overlap.
    let low = start[axis].min(end[axis]).max(b0.min(b1));
    let high = start[axis].max(end[axis]).min(b0.max(b1));
    if low >= high {
        return Ok(None);
    }
    let complete_a = low == start[axis].min(end[axis]) && high == start[axis].max(end[axis]);
    let complete_b = low == b0.min(b1) && high == b0.max(b1);
    if !complete_a && !complete_b {
        // Interleaved segments contribute one original tip each. When those
        // tips are already a vertex contact, this is a corner adjustment, not
        // a tiny pair of cut edges. A complete short edge remains eligible.
        let inside = |p: &&Point3| {
            let coordinate = p.to_array()[axis];
            coordinate >= low && coordinate <= high
        };
        let a_tip = a_end
            .iter()
            .find(inside)
            .expect("crossing interval has an original tip");
        let b_tip = b_end
            .iter()
            .find(inside)
            .expect("crossing interval has an original tip");
        if certificate::point_bound(*a_tip, *b_tip, distance).is_some() {
            return Ok(None);
        }
    }
    let mut parameters = [[0.; 2]; 2];
    let mut points = [[a_end[0]; 2]; 2];
    for (i, curve) in [a, b].into_iter().enumerate() {
        for (j, target) in [low, high].into_iter().enumerate() {
            let (parameter, point) = parameter_at_coordinate(curve, axis, target, budget)?;
            parameters[i][j] = parameter;
            points[i][j] = point;
        }
    }
    if parameters.iter().any(|p| p[0] == p[1])
        || points.iter().any(|p| p[0] == p[1])
        || (0..2).any(|i| certificate::point_bound(points[0][i], points[1][i], distance).is_none())
    {
        return Ok(None);
    }
    for p in &mut parameters {
        p.sort_by(Real::total_cmp);
    }
    // Check the actual retained-control trim representations before proposing
    // cuts, since knot insertion can introduce small non-collinear rounding.
    budget.charge(
        a.control_points()
            .len()
            .saturating_add(b.control_points().len()),
    )?;
    let ac = a.try_trimmed(parameters[0][0]..=parameters[0][1])?;
    let bc = b.try_trimmed(parameters[1][0]..=parameters[1][1])?;
    Ok(full_match(&ac, &bc, distance, budget)?.map(|_| parameters))
}

fn ordered(value: Real) -> u64 {
    let bits = value.to_bits();
    if value.is_sign_negative() {
        !bits
    } else {
        bits ^ (1 << 63)
    }
}
fn from_ordered(bits: u64) -> Real {
    Real::from_bits(if bits & (1 << 63) != 0 {
        bits ^ (1 << 63)
    } else {
        !bits
    })
}

fn parameter_at_coordinate(
    curve: &NurbsCurve,
    axis: usize,
    target: Real,
    budget: &mut Budget,
) -> Result<(Real, Point3), GeometryError> {
    let ends = certificate::linear_endpoints(curve).expect("preflighted straight edge");
    let domain = curve.domain();
    let (start, end) = (*domain.start(), *domain.end());
    if target == ends[0].to_array()[axis] {
        return Ok((start, ends[0]));
    }
    if target == ends[1].to_array()[axis] {
        return Ok((end, ends[1]));
    }
    let increasing = ends[0].to_array()[axis] < ends[1].to_array()[axis];
    let controls = curve.control_points();
    if curve.degree() == 1 && controls.len() == 2 && controls[0].weight() == controls[1].weight() {
        let sign = if increasing { 1. } else { -1. };
        if let Ok(parameter) = crate::parameter::map_parameter(
            sign * target,
            sign * ends[0].to_array()[axis]..=sign * ends[1].to_array()[axis],
            start..=end,
        ) {
            budget.charge(4)?;
            if parameter > start && parameter < end {
                return Ok((parameter, curve.evaluate(parameter)?));
            }
        }
    }
    let mut lo = ordered(start);
    let mut hi = ordered(end);
    // Binary search the finite floating-point parameter lattice: at most 64
    // evaluations, even for subnormal intervals and extreme rational speeds.
    while hi - lo > 1 {
        budget.charge(curve.degree().saturating_add(1).saturating_pow(2))?;
        let mid = lo + (hi - lo) / 2;
        let parameter = from_ordered(mid);
        let point = curve.evaluate(parameter)?;
        let value = point.to_array()[axis];
        if value == target {
            return Ok((parameter, point));
        }
        if (value < target) == increasing {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let a = from_ordered(lo);
    let b = from_ordered(hi);
    budget.charge(
        curve
            .degree()
            .saturating_add(1)
            .saturating_pow(2)
            .saturating_mul(2),
    )?;
    let ap = curve.evaluate(a)?;
    let bp = curve.evaluate(b)?;
    if (ap.to_array()[axis] - target).abs() <= (bp.to_array()[axis] - target).abs() {
        Ok((a, ap))
    } else {
        Ok((b, bp))
    }
}
