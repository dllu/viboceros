use super::chain_native as chain;
use serde_json::Value;
pub fn phase(state: &Value) -> &'static str {
    if state["in_command"] == 0 || state["prompt"] == "Command" {
        return "idle";
    }
    let prompt = state["prompt"].as_str().unwrap();
    if prompt.starts_with("Select branch") {
        "branch"
    } else if prompt.starts_with("Select connected") {
        "connected"
    } else if prompt.starts_with("Select curve in chain") {
        "seed"
    } else if prompt.starts_with("Select curves to measure") {
        "parent"
    } else if prompt.starts_with("Units") {
        "units"
    } else {
        panic!("unqualified Length prompt: {prompt}")
    }
}
pub fn normalize(state: &Value) -> Value {
    let mut result = chain::normalize(state);
    result["phase"] = serde_json::json!(phase(state));
    result
}
pub fn normalize_trace(state: &Value, settled: &Value) -> Value {
    // Older EndCommand callbacks precede Length's selection cleanup. Preserve
    // those raw events; compare terminal inputs at their settled return boundary.
    let mut result = normalize(if phase(state) == "idle" {
        settled
    } else {
        state
    });
    result["objects"] = state["objects"].clone();
    result
}
pub fn length_mm(history: &str) -> Option<(f64, f64)> {
    history.lines().rev().find_map(|line| {
        let s = line
            .strip_prefix("Length = ")
            .or_else(|| line.strip_prefix("Cumulative length = "))?;
        let mut words = s.split_whitespace();
        let value = words.next()?.parse::<f64>().ok()?;
        let scale = match words.next()? {
            "millimeters" | "millimeter" => 1.,
            "centimeters" | "centimeter" => 10.,
            other => panic!("unqualified native length unit: {other}"),
        };
        Some((value * scale, 0.00051 * scale))
    })
}
pub fn observation(base: &std::path::Path, family: &str) -> Value {
    use std::io::Read;
    let path = base.join(format!("observations/{family}.json"));
    let raw = if path.exists() {
        std::fs::read(path).unwrap()
    } else {
        let bytes = std::fs::read(path.with_extension("json.gz")).unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut raw)
            .unwrap();
        raw
    };
    serde_json::from_slice(&raw).unwrap()
}
