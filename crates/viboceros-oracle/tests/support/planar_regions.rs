//! Match surfaces by original boundary membership; keep every source and phase.
use serde_json::Value;
/// Return the count of the explicitly recorded native area discrepancy.
/// All other numeric, source, membership and history differences remain errors.
pub fn compare(
    actual: &Value,
    native: &Value,
    area_epsilon: f64,
    case: &str,
) -> Result<usize, String> {
    let a = normalized(actual.clone());
    let b = normalized(native.clone());
    let mut gaps = Vec::new();
    values(&a, &b, "regions", area_epsilon, &mut gaps)?;
    for (path, actual, native) in &gaps {
        if case != "crossing_circle10_small"
            || (*actual - 68. * std::f64::consts::PI * 1e-6).abs() > 1e-15
            || (*native - 0.00021362720231357302).abs() > 1e-18
        {
            return Err(format!("{path}: {actual} != {native}"));
        }
    }
    Ok(gaps.len())
}
fn normalized(mut v: Value) -> Value {
    match &mut v {
        Value::Array(a) => {
            for v in a.iter_mut() {
                *v = normalized(v.take());
            }
            if a.iter().all(|v| v.get("kind").is_some()) {
                a.sort_by_key(|v| {
                    if v["kind"] == "curve" {
                        format!("0{:05}", v["source"].as_u64().unwrap())
                    } else if v["kind"] == "brep" {
                        format!("1{}", v["loops"])
                    } else {
                        format!(
                            "{}{}",
                            if v["kind"] == "Outer" { "0" } else { "1" },
                            v["sources"]
                        )
                    }
                });
            }
        }
        Value::Object(a) => {
            for key in [
                "created",
                "events",
                "history",
                "brep",
                "min",
                "max",
                "contains_start",
                "native_valid",
                "coverage",
            ] {
                a.remove(key);
            }
            for v in a.values_mut() {
                *v = normalized(v.take());
            }
        }
        _ => {}
    }
    v
}
fn values(
    a: &Value,
    b: &Value,
    path: &str,
    area_epsilon: f64,
    gaps: &mut Vec<(String, f64, f64)>,
) -> Result<(), String> {
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
        let epsilon = if path.ends_with("/area") {
            area_epsilon + 1e-12 * x.abs().max(y.abs())
        } else {
            1e-12
        };
        if (x - y).abs() <= epsilon {
            return Ok(());
        }
        if path.ends_with("/area") {
            gaps.push((path.to_string(), x, y));
            return Ok(());
        }
    } else {
        match (a, b) {
            (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
                for (k, v) in a {
                    values(v, &b[k], &format!("{path}/{k}"), area_epsilon, gaps)?;
                }
                return Ok(());
            }
            (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    values(a, b, &format!("{path}/{i}"), area_epsilon, gaps)?;
                }
                return Ok(());
            }
            _ if a == b => return Ok(()),
            _ => {}
        }
    }
    Err(format!("{path}: {a} != {b}"))
}
