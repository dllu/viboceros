//! Complete polynomial tensor bounds reuse the existing exact curve certificate.
//! An intermediate patch has the actual controls/V basis and Native U basis.
//! Every U control row shares the same monotone V correspondence.
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, Point3};
#[path = "loft_refit_surface_bound/correspondence.rs"]
mod correspondence;
#[path = "loft_refit_surface_bound/knots.rs"]
mod knots;
#[path = "loft_refit_surface_bound/refinement.rs"]
mod refinement;

fn curve(patch: &Value, axis: usize, index: usize, knots: &Value) -> NurbsCurve {
    let nu = patch["control_count"][0].as_u64().unwrap() as usize;
    let points = patch["control_points"].as_array().unwrap();
    let point = |p: &Value| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap()).unwrap()
    };
    let controls = if axis == 0 {
        points[index * nu..(index + 1) * nu]
            .iter()
            .map(point)
            .collect()
    } else {
        points.iter().skip(index).step_by(nu).map(point).collect()
    };
    NurbsCurve::try_new(
        patch["degree"][axis].as_u64().unwrap() as usize,
        controls,
        serde_json::from_value(knots.clone()).unwrap(),
    )
    .unwrap()
}
fn certify(a: &NurbsCurve, b: &NurbsCurve, limit: f64) -> Result<f64, String> {
    a.parameter_curve_deviation_bound(b, limit)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "complete curve bound unresolved".into())
}

fn parameter_bound(a: &Value, b: &Value, limit: f64) -> Result<f64, String> {
    let nu = a["control_count"][0].as_u64().unwrap() as usize;
    let mut maximum = 0_f64;
    for u in 0..nu {
        maximum = maximum.max(certify(
            &curve(a, 1, u, &a["knots_v"]),
            &curve(b, 1, u, &b["knots_v"]),
            limit,
        )?);
    }
    Ok(maximum)
}

pub fn bound(actual: &Value, native: &Value, limit: f64) -> Result<f64, String> {
    let a = actual.as_array().ok_or("actual patches missing")?;
    let b = native.as_array().ok_or("native patches missing")?;
    if a.is_empty() || a.len() != b.len() {
        return Err("patch count differs".into());
    }
    let mut maximum = 0_f64;
    for (a, b) in a.iter().zip(b) {
        if a["degree"][0] != b["degree"][0] || a["control_count"][0] != b["control_count"][0] {
            return Err("matching U degree and control count required".into());
        }
        for patch in [a, b] {
            if patch["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["weight"].as_f64() != Some(1.))
            {
                return Err("polynomial patch required".into());
            }
            for (axis, key) in [(0, "knots_u"), (1, "knots_v")] {
                let c = curve(patch, axis, 0, &patch[key]);
                let domain = if axis == 0 { "domain_u" } else { "domain_v" };
                if patch[domain][0].as_f64() != Some(*c.domain().start())
                    || patch[domain][1].as_f64() != Some(*c.domain().end())
                {
                    return Err("curve active domain differs from recorded patch".into());
                }
            }
        }
        let different_u = a["knots_u"] != b["knots_u"];
        let reserve = if different_u { limit * 1e-4 } else { 0. };
        let remaining = if reserve == 0. {
            limit
        } else {
            (limit - reserve).next_down()
        };
        if !limit.is_finite()
            || limit <= 0.
            || !remaining.is_finite()
            || remaining <= 0.
            || reserve >= limit
            || (different_u && reserve == 0.)
        {
            return Err("invalid or unrepresentable surface error budget".into());
        }
        if reserve > 0. {
            for v in 0..a["control_count"][1].as_u64().unwrap() as usize {
                certify(
                    &curve(a, 0, v, &a["knots_u"]),
                    &curve(a, 0, v, &b["knots_u"]),
                    reserve,
                )?;
            }
        }
        let row_bound = knots::bound(a, b, remaining)
            .or_else(|_| parameter_bound(a, b, remaining))
            .or_else(|_| refinement::bound(a, b, remaining))
            .or_else(|_| correspondence::bound(a, b, remaining))?;
        // Charge the entire first-leg reserve. Downward subtraction keeps its
        // exact sum with the second-leg budget within the requested limit.
        maximum = maximum.max((reserve + row_bound).next_up().min(limit));
    }
    Ok(maximum)
}

#[cfg(test)]
#[path = "loft_refit_surface_bound/tests.rs"]
mod tests;
