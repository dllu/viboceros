//! Restrict the exact parameter domain, not a rounded knot-insertion result.
use super::*;

pub(in crate::brep::join_edges) fn restricted_curve_bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    interval: [Real; 2],
    limit: Real,
    tighten: bool,
    mut charge: impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    let domain = b.domain();
    if interval[0].min(interval[1]) == *domain.start()
        && interval[0].max(interval[1]) == *domain.end()
        && interval.iter().all(|t| t.is_finite())
    {
        let reversed = interval[0] > interval[1];
        return if tighten {
            refined_curve_bound(a, b, reversed, limit, charge)
        } else {
            whole_curve_bound(a, b, reversed, limit, charge)
        };
    }
    if [a, b].iter().any(|c| c.degree() > MAX_DEGREE) {
        return Ok(None);
    }
    let Some(a) = extract::Spline::new(a, false, &mut charge)? else {
        return Ok(None);
    };
    let Some(b) = extract::Spline::restricted(b, interval, &mut charge)? else {
        return Ok(None);
    };
    splines_bound(&a, &b, limit, tighten, &mut charge)
}

/// Endpoint approached from inside an oriented restriction. At a full-order
/// knot the two sides need not agree; never substitute a rounded evaluation or
/// the default right-hand value for the restriction's actual endpoint.
pub(in crate::brep::join_edges) fn restricted_endpoint_bound(
    point: Point3,
    curve: &NurbsCurve,
    interval: [Real; 2],
    end: bool,
    limit: Real,
    mut charge: impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    let domain = curve.domain();
    let t = interval[usize::from(end)];
    if interval[0] != interval[1]
        && interval.iter().all(|t| t.is_finite() && domain.contains(t))
        && (t == *domain.start() || t == *domain.end())
        && let Some(endpoint) = endpoint(curve, t == *domain.end())
    {
        charge(curve.degree() + 1)?;
        return Ok(point_bound(point, endpoint, limit));
    }
    let Some(h) = endpoint_value(curve, interval, end, &mut charge)? else {
        return Ok(None);
    };
    let coordinates = point.to_array();
    let difference = std::array::from_fn(|i| {
        if i == 3 {
            h[3].clone()
        } else {
            &h[i] - rational(coordinates[i]) * &h[3]
        }
    });
    Ok(refine::norm_bound(&difference, limit))
}

/// Exact endpoint-to-endpoint gap, including unclamped stored edges. The first
/// curve uses its complete domain, the second an oriented restriction.
pub(in crate::brep::join_edges) fn curve_endpoint_bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    interval: [Real; 2],
    ends: [bool; 2],
    limit: Real,
    mut charge: impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    let ad = a.domain();
    let Some(a) = endpoint_value(a, [*ad.start(), *ad.end()], ends[0], &mut charge)? else {
        return Ok(None);
    };
    let Some(b) = endpoint_value(b, interval, ends[1], &mut charge)? else {
        return Ok(None);
    };
    let difference = std::array::from_fn(|i| {
        if i == 3 {
            &a[3] * &b[3]
        } else {
            &a[i] * &b[3] - &b[i] * &a[3]
        }
    });
    Ok(refine::norm_bound(&difference, limit))
}

pub(super) fn endpoint_value(
    curve: &NurbsCurve,
    interval: [Real; 2],
    end: bool,
    charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<H>, GeometryError> {
    let domain = curve.domain();
    if interval
        .iter()
        .any(|t| !t.is_finite() || !domain.contains(t))
        || interval[0] == interval[1]
    {
        return Ok(None);
    }
    charge(curve.degree() + 1)?;
    let t = interval[usize::from(end)];
    if (t == *domain.start() || t == *domain.end())
        && let Some(endpoint) = endpoint(curve, t == *domain.end())
    {
        let coordinates = endpoint.to_array();
        return Ok(Some(std::array::from_fn(|i| {
            if i == 3 {
                rational(1.)
            } else {
                rational(coordinates[i])
            }
        })));
    }
    if curve.degree() > MAX_DEGREE {
        return Ok(None);
    }
    let Some(curve) = extract::Spline::restricted(curve, interval, charge)? else {
        return Ok(None);
    };
    let (mut controls, _) = curve.end_span(end, charge)?;
    let i = if end { controls.len() - 1 } else { 0 };
    Ok(Some(controls.swap_remove(i)))
}
