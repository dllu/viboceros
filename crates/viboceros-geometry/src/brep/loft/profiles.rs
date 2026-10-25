//! Parallel-section ordering, traversal alignment, and nearest-knot seams.
use super::*;

pub(super) fn prepare(
    curves: &[NurbsCurve],
    options: LoftOptions,
    tolerance: Tolerance,
) -> Result<(Vec<NurbsCurve>, bool), GeometryError> {
    if curves
        .iter()
        .any(|c| c.control_points().len() > MAX_LOFT_SECTION_CONTROLS)
    {
        return Err(GeometryError::LoftResourceLimit {
            context: "section controls",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        });
    }
    let mut profiles = curves.to_vec();
    let oriented = sort_parallel_profiles(&mut profiles, tolerance)?;
    if profiles
        .iter()
        .map(NurbsCurve::is_closed)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .all(|c| c)
    {
        align_closed_profiles(&mut profiles, !options.natural_seams)?;
    }
    Ok((profiles, oriented))
}

pub(super) fn sort_parallel_profiles(
    curves: &mut [NurbsCurve],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for curve in curves.iter() {
        if !curve.is_closed()? || !curve.is_planar(tolerance)? {
            return Ok(false);
        }
    }
    let normals = curves
        .iter()
        .map(area_normal)
        .collect::<Result<Vec<_>, _>>()?;
    let Some(axis) = normals[0] else {
        return Ok(false);
    };
    for normal in &normals {
        let Some(normal) = normal else {
            return Ok(false);
        };
        if axis.cross(*normal)?.length()? > tolerance.angular().max(1e-12) {
            return Ok(false);
        }
    }
    let origin = curves[0].control_points()[0].point();
    let mut ordered = Vec::with_capacity(curves.len());
    for (curve, normal) in curves.iter().zip(normals) {
        let offset = origin
            .vector_to(curve.control_points()[0].point())?
            .dot(axis)?;
        ordered.push((offset, curve.clone(), normal.unwrap()));
    }
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Sorting follows the first selected section's area normal. The first
    // sorted section retains its original traversal for the later tangent pass.
    let reference = ordered[0].2;
    for (curve, (_, source, _)) in curves.iter_mut().zip(ordered) {
        *curve = source;
    }
    Ok(reference.dot(axis)? > 0.)
}

pub(super) fn align_closed_profiles(
    curves: &mut [NurbsCurve],
    move_seams: bool,
) -> Result<(), GeometryError> {
    let Some(first) = curves.first() else {
        return Ok(());
    };
    let mut previous = first.evaluate(*first.domain().start())?;
    let mut previous_tangent = seam_tangent(first)?;
    for curve in &mut curves[1..] {
        let domain = curve.domain();
        let mut best = (*domain.start(), f64::INFINITY);
        for group in curve.knots().chunk_by(|a, b| a == b).filter(|_| move_seams) {
            let t = group[0];
            if t < *domain.start() || t >= *domain.end() {
                continue;
            }
            let distance = curve.evaluate(t)?.distance_to(previous)?;
            if distance < best.1 {
                best = (t, distance);
            }
        }
        if best.0 != *domain.start() {
            *curve = curve.try_change_closed_seam(best.0)?;
        }
        let mut tangent = seam_tangent(curve)?;
        // Retain native scalar operation order and negate the cached vector
        // when reversing. Re-evaluation would change rounding in constant
        // coordinates and can choose a different direction at a right angle.
        let dot = previous_tangent[0] * tangent[0]
            + previous_tangent[1] * tangent[1]
            + previous_tangent[2] * tangent[2];
        if dot < 0. {
            *curve = curve.reversed()?;
            tangent = tangent.map(|x| -x);
        }
        previous = curve.evaluate(*curve.domain().start())?;
        previous_tangent = tangent;
    }
    Ok(())
}

fn seam_tangent(curve: &NurbsCurve) -> Result<[Real; 3], GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|c| {
            let p = c.point();
            [p.x(), p.y(), p.z(), c.weight()]
        })
        .collect::<Vec<_>>();
    viboceros_opennurbs::curve_tangents(
        curve.degree(),
        &controls,
        curve.knots(),
        &[*curve.domain().start()],
    )?[0]
        .ok_or(GeometryError::Degenerate {
            context: "Loft section seam tangent",
        })
}

fn area_normal(curve: &NurbsCurve) -> Result<Option<Vector3>, GeometryError> {
    // Integrate in local, scaled coordinates. Only the area direction is
    // needed; do not form squared world-scale areas or reciprocal tiny weights.
    let origin = curve.control_points()[0].point();
    let points = curve
        .control_points()
        .iter()
        .map(|c| origin.vector_to(c.point()))
        .collect::<Result<Vec<_>, _>>()?;
    let scale = points
        .iter()
        .flat_map(|p| p.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    let weight_scale = curve
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max);
    let controls = points
        .iter()
        .zip(curve.control_points())
        .map(|(p, c)| {
            WeightedPoint3::try_new(
                Point3::try_new(p.x() / scale, p.y() / scale, p.z() / scale)?,
                c.weight() / weight_scale,
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let local = NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())?
        .try_reparameterized(0.0..=1.0)?;
    let mut area = [0.0; 3];
    for (a, b) in local.spans() {
        for (axis, value) in area.iter_mut().enumerate() {
            *value += crate::integration::integrate_adaptive(a, b, 1e-12 * (b - a), 1e-10, |t| {
                let (p, tangent) = local.evaluate_with_derivative(t)?;
                Ok(Vector3::try_new(p.x(), p.y(), p.z())?
                    .cross(tangent)?
                    .to_array()[axis])
            })?;
        }
    }
    let area = Vector3::try_from(area)?;
    if area.length()? <= 1e-10 {
        return Ok(None);
    }
    Ok(Some(area.normalized_nonzero()?.as_vector()))
}
