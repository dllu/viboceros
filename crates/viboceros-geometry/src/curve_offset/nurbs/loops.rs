//! Closed planar offsets with oppositely wound cusp loops removed.
use super::*;
mod contacts;
mod cycles;
mod forward;
mod limits;
#[cfg(test)]
mod tests;

pub(crate) fn offset_closed_region(
    source: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
    operations: Option<CurveOffsetOperations>,
) -> Result<NurbsCurve, GeometryError> {
    if !source.is_closed()? {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    let source = source.try_reparameterized(0. ..=1.)?;
    let normal = offset_plane(&source, fallback, tolerance)?;
    let orientation = nurbs_region_inward_sign(&source, normal, tolerance, operations)?;
    if limits::collapsed(&source, normal, distance * orientation, tolerance)? {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    // The same Hermite fitter represents the full normal locus, including
    // reversed stretches. Backend smooth offsets have a regular-curve contract;
    // structural intersection/trim producers remain available below.
    // Intersections amplify independent fit errors at shallow crossing angles.
    // Spend a tighter fit budget on this exceptional branch before trimming.
    let fit = Tolerance::try_new(
        tolerance.absolute() * 0.01,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let raw = fit_offset_nurbs(&source, distance, normal, fit, true, None, true)?;
    let contacts = contacts::find(&raw, &source, fit, operations, |t| {
        let (_, v, a) = source.evaluate_with_second_derivative(t)?;
        let speed = v.length()?;
        let gain = 1. - distance * v.cross(a)?.dot(normal.as_vector())? / speed.powi(3);
        Ok(gain > 0.)
    })?;
    if contacts.is_empty() {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    let mut candidates = Vec::new();
    for [a, b] in contacts {
        for (start, end) in [(a, b), (b, a + 1.)] {
            let arc = cyclic_trim(&raw, start, end, None)?;
            let area = cycles::area(&arc, normal)?;
            if area * orientation < 0. {
                candidates.push([start, end]);
            }
        }
    }
    let removed = cycles::minimal(candidates)?;
    if removed.is_empty() {
        return Err(GeometryError::SelfIntersectingOffsetRegion);
    }
    let mut kept = Vec::new();
    for (i, &[_, end]) in removed.iter().enumerate() {
        let next = if i + 1 < removed.len() {
            removed[i + 1][0]
        } else {
            removed[0][0] + 1.
        };
        if end > next {
            return Err(GeometryError::DegenerateOffsetRegion);
        }
        if end == next {
            continue;
        }
        let start = end.rem_euclid(1.);
        kept.push([start, start + next - end]);
    }
    if kept.is_empty() {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    for &[a, b] in &kept {
        for [start, end] in [[a, b.min(1.)], [0., (b - 1.).max(0.)]] {
            for (lo, hi) in source.spans() {
                let (lo, hi) = (lo.max(start), hi.min(end));
                if lo >= hi {
                    continue;
                }
                for i in 0..=16 {
                    let t = lo + (hi - lo) * (i as Real / 16.);
                    offset_sample(
                        &source,
                        distance,
                        normal,
                        t,
                        if i == 16 {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                    )?;
                }
            }
        }
    }
    kept.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let mut pieces = kept
        .into_iter()
        .map(|[a, b]| cyclic_trim(&raw, a, b, operations))
        .collect::<Result<Vec<_>, _>>()?;
    close_junctions(&mut pieces, tolerance)?;
    let result = PolyCurve3::try_new(
        pieces
            .into_iter()
            .map(CurveSegment3::NurbsCurve)
            .collect::<Vec<_>>(),
    )?
    .to_nurbs()?;
    // The B-rep caller validates the complete region, including other boundaries.
    if cycles::area(&result, normal)? * orientation <= 0. {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    Ok(result)
}

fn cyclic_trim(
    curve: &NurbsCurve,
    start: Real,
    end: Real,
    operations: Option<CurveOffsetOperations>,
) -> Result<NurbsCurve, GeometryError> {
    if !(0. ..1.).contains(&start) || end <= start || end - start >= 1. {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    if end <= 1. {
        return offset_trim(curve, start..=end, operations);
    }
    let pieces = [
        offset_trim(curve, start..=1., operations)?,
        offset_trim(curve, 0. ..=end - 1., operations)?,
    ];
    PolyCurve3::try_new(
        pieces
            .into_iter()
            .map(CurveSegment3::NurbsCurve)
            .collect::<Vec<_>>(),
    )?
    .to_nurbs()
}
fn close_junctions(pieces: &mut [NurbsCurve], tolerance: Tolerance) -> Result<(), GeometryError> {
    for i in 0..pieces.len() {
        let j = (i + 1) % pieces.len();
        if i == j {
            let c = &pieces[i];
            let a = c.evaluate(*c.domain().start())?;
            let b = c.evaluate(*c.domain().end())?;
            if a.distance_to(b)? > tolerance.absolute() * 0.1 {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
            let point = a.midpoint(b)?;
            let mut controls = c.control_points().to_vec();
            let last = controls.len() - 1;
            controls[0] = WeightedPoint3::try_new(point, controls[0].weight())?;
            controls[last] = WeightedPoint3::try_new(point, controls[last].weight())?;
            pieces[i] = NurbsCurve::try_new_rational(c.degree(), controls, c.knots().to_vec())?;
        } else if i < j {
            let (a, b) = pieces.split_at_mut(j);
            align_trimmed_ends(&mut a[i], &mut b[0], tolerance)?;
        } else {
            let (a, b) = pieces.split_at_mut(i);
            align_trimmed_ends(&mut b[0], &mut a[j], tolerance)?;
        }
    }
    Ok(())
}
