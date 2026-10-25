//! Traversal alignment and nearest-knot seams for private Loft profiles.
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
    let oriented = super::ordering::sort_profiles(&mut profiles, tolerance)?;
    let closed = profiles
        .iter()
        .map(NurbsCurve::is_closed)
        .collect::<Result<Vec<_>, _>>()?;
    if closed.iter().all(|c| *c) {
        align_closed_profiles(&mut profiles, !options.natural_seams)?;
    } else if closed.iter().all(|c| !c) {
        align_open_profiles(&mut profiles)?;
    }
    Ok((profiles, oriented))
}

fn align_open_profiles(curves: &mut [NurbsCurve]) -> Result<(), GeometryError> {
    let Some(first) = curves.first() else {
        return Ok(());
    };
    let mut previous = [
        first.evaluate(*first.domain().start())?,
        first.evaluate(*first.domain().end())?,
    ];
    for curve in &mut curves[1..] {
        let ends = [
            curve.evaluate(*curve.domain().start())?,
            curve.evaluate(*curve.domain().end())?,
        ];
        let distances = [
            previous[0].distance_to(ends[0])?,
            previous[1].distance_to(ends[1])?,
            previous[0].distance_to(ends[1])?,
            previous[1].distance_to(ends[0])?,
        ];
        let mut forward = distances[0] + distances[1];
        let mut backward = distances[2] + distances[3];
        if !forward.is_finite() || !backward.is_finite() {
            let scale = distances.into_iter().fold(0_f64, f64::max);
            forward = distances[0] / scale + distances[1] / scale;
            backward = distances[2] / scale + distances[3] / scale;
        }
        if backward < forward {
            *curve = curve.reversed()?;
            previous = [ends[1], ends[0]];
        } else {
            previous = ends;
        }
    }
    Ok(())
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

pub(super) fn area_normal(curve: &NurbsCurve) -> Result<Option<Vector3>, GeometryError> {
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
