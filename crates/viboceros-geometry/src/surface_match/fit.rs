use super::*;
use crate::WeightedPoint3;
use jets::Vec3;

pub(super) fn fit<E: From<GeometryError>>(
    source: &NurbsSurface,
    target: &NurbsSurface,
    reverse: bool,
    options: SurfaceMatchOptions,
    _tolerance: Tolerance,
    solve: &mut super::Solve<'_, E>,
) -> Result<NurbsSurface, E> {
    let n = source.control_point_count_u();
    let m = source.control_point_count_v();
    let p = source.degree_u();
    let q = source.degree_v();
    if m.checked_pow(3).is_none_or(|n| n > 64_000_000) {
        return Err(error("boundary interpolation work budget exceeded").into());
    }
    let end = *source.domain_u().end();
    let ks = source.knots_u();
    let d0 = p as Real / (end - ks[n - 1]);
    let d1 = if p > 1 {
        p as Real / (end - ks[n - 2])
    } else {
        0.
    };
    let dd = if p > 1 {
        (p - 1) as Real / (end - ks[n - 1])
    } else {
        0.
    };
    let rows = options.continuity.order() + 1;
    let mut parameters = Vec::with_capacity(m);
    let mut targets = Vec::with_capacity(m * rows * 4);
    let target_curve = options
        .closest_points
        .then(|| target.isocurve_v(*target.domain_u().start()))
        .transpose()?;
    let closest = target_curve
        .as_ref()
        .map(|c| c.prepare_closest_parameter_search())
        .transpose()?;
    let origin = source.control_points()[0].point();
    let second_curve = if rows > 2 {
        Some(crate::NurbsCurve::try_new_rational(
            q,
            (0..m)
                .map(|j| source.control_points()[j * n + n - 3])
                .collect(),
            source.knots_v().to_vec(),
        )?)
    } else {
        None
    };
    for j in 0..m {
        let parameter = crate::nurbs::stable_knot_mean(&source.knots_v()[j + 1..=j + q])?;
        parameters.push(parameter);
        let f = (parameter - source.domain_v().start())
            / (source.domain_v().end() - source.domain_v().start());
        let mut original = source.evaluate_with_second_derivatives(end, parameter)?;
        let target_v = if let Some(closest) = &closest {
            closest.closest_parameter(original.point, _tolerance)?
        } else {
            target.parameter_at_v(if reverse { 1. - f } else { f })?
        };
        let desired =
            target.evaluate_with_second_derivatives(*target.domain_u().start(), target_v)?;
        let basis = source.basis_values(end, parameter)?[1].clone();
        let weights = (0..rows)
            .map(|i| {
                basis
                    .iter()
                    .enumerate()
                    .map(|(j, b)| b * source.control_points()[j * n + n - 1 - i].weight())
                    .sum::<Real>()
            })
            .collect::<Vec<_>>();
        if let Some(curve) = &second_curve {
            let delta = original
                .point
                .vector_to(curve.evaluate(parameter)?)?
                .scaled(dd * d1 * (1. - weights[2]) / weights[0])?;
            original.derivative_uu = crate::Vector3::try_from(std::array::from_fn(|i| {
                original.derivative_uu.to_array()[i] + delta.to_array()[i]
            }))?;
        }
        // The SDK's nonuniform-cross-knot rule scales the normal term by
        // adjacent/last endpoint coefficients; its tangent term is unchanged.
        let derivatives = jets::desired(
            original,
            desired,
            options.iso,
            options.continuity,
            if p > 1 { d1 / d0 } else { 1. },
        )?;
        let point = Vec3::from_row_slice(&origin.vector_to(desired.point)?.to_array());
        let h0 = point * weights[0];
        targets.extend([h0.x, h0.y, h0.z, weights[0]]);
        if rows > 1 {
            let wdu = d0 * (weights[0] - weights[1]);
            let hdu = derivatives[0] * weights[0] + point * wdu;
            let h1 = h0 - hdu / d0;
            targets.extend([h1.x, h1.y, h1.z, weights[1]]);
            if rows > 2 {
                // Rhino's curvature row uses unit weights. Boundary and first
                // derivative row weights retain the source's rational basis.
                let w2 = 1.;
                let wdd = dd * (d0 * weights[0] - (d0 + d1) * weights[1] + d1 * w2);
                let hdd = derivatives[1] * weights[0] + derivatives[0] * (2. * wdu) + point * wdd;
                let h2 = (hdd / dd - h0 * d0 + h1 * (d0 + d1)) / d1;
                targets.extend([h2.x, h2.y, h2.z, w2]);
            }
        }
    }
    let solved = solve(&NurbsInterpolationSystem {
        degree: q,
        knots: source.knots_v(),
        parameters: &parameters,
        periodic: false,
        columns: rows * 4,
        targets: &targets,
    })?;
    if solved.len() != m * rows * 4 || solved.iter().any(|v| !v.is_finite()) {
        return Err(error("interpolation backend returned an invalid boundary strip").into());
    }
    let mut controls = source.control_points().to_vec();
    for j in 0..m {
        for row in 0..rows {
            let h = &solved[(j * rows + row) * 4..(j * rows + row + 1) * 4];
            if h[3] <= 0. {
                return Err(error("matching produced nonpositive weights").into());
            }
            let offset = crate::Vector3::try_new(h[0] / h[3], h[1] / h[3], h[2] / h[3])?;
            let point = origin.translated(offset)?;
            controls[j * n + n - 1 - row] = WeightedPoint3::try_new(point, h[3])?;
        }
    }
    Ok(NurbsSurface::try_new_rational(
        p,
        q,
        n,
        m,
        controls,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )?)
}
