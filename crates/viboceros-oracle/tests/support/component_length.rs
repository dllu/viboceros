use serde_json::Value;
pub fn measurement(row: &Value) -> Option<(f64, f64)> {
    row["history"]
        .as_str()
        .unwrap()
        .lines()
        .rev()
        .find_map(|s| {
            let s = s
                .strip_prefix("Length = ")
                .or_else(|| s.strip_prefix("Cumulative length = "))?;
            let mut tokens = s.split_whitespace();
            let value = tokens.next()?.parse::<f64>().ok()?;
            let scale = match tokens.next()? {
                "millimeters" => 1.,
                "centimeters" => 10.,
                unit => panic!("unqualified Length unit: {unit}"),
            };
            Some((value * scale, 0.00051 * scale))
        })
}
