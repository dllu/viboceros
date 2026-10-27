//! Tighten the U convex hull without treating rounded knot edits as exact.
use super::*;
use serde_json::json;

fn refine(patch: &Value, knots: &Value, intervals: usize, reserve: f64) -> Result<Value, String> {
    let nv = patch["control_count"][1].as_u64().unwrap() as usize;
    let mut columns = Vec::with_capacity(nv);
    for v in 0..nv {
        let source = curve(patch, 0, v, knots);
        let domain = source.domain();
        let degree = source.degree();
        let mut result = source
            .try_insert_knot(*domain.start(), degree + 1)
            .and_then(|c| c.try_insert_knot(*domain.end(), degree + 1))
            .map_err(|e| e.to_string())?;
        for i in 1..intervals {
            let t = source
                .parameter_at(i as f64 / intervals as f64)
                .map_err(|e| e.to_string())?;
            result = result
                .try_insert_knot(t, degree)
                .map_err(|e| e.to_string())?;
        }
        // This complete curve bound includes all rounding in clamping and
        // insertion. Positive V basis functions extend it over the patch.
        certify(&source, &result, reserve)?;
        columns.push(result);
    }
    let mut result = patch.clone();
    result["control_count"][0] = json!(columns[0].control_points().len());
    result["knots_u"] = json!(columns[0].knots());
    result["domain_u"] = json!([columns[0].domain().start(), columns[0].domain().end()]);
    result["control_points"] = json!(
        columns
            .iter()
            .flat_map(|c| c
                .control_points()
                .iter()
                .map(|p| json!({"point":p.point().to_array(),"weight":p.weight()})))
            .collect::<Vec<_>>()
    );
    Ok(result)
}

pub(super) fn bound(a: &Value, b: &Value, limit: f64) -> Result<f64, String> {
    let reserve = limit * 1e-4;
    let remaining = (limit - 2. * reserve).next_down();
    if reserve <= 0. || remaining <= 0. {
        return Err("unrepresentable refinement reserve".into());
    }
    for intervals in [2, 4, 8, 16, 32, 64] {
        let a = refine(a, &b["knots_u"], intervals, reserve)?;
        let b = refine(b, &b["knots_u"], intervals, reserve)?;
        if a["knots_u"] != b["knots_u"] {
            return Err("refined U bases differ".into());
        }
        if let Ok(bound) = parameter_bound(&a, &b, remaining) {
            return Ok((2. * reserve + bound).next_up().min(limit));
        }
    }
    Err("complete refined surface bound unresolved".into())
}
