//! Parameter-domain precision and independent Native length measurements.
use serde_json::Value;
use viboceros_document::Document;
/// Native BlendCrv domains use GetLength's default desired 1e-8 fractional
/// precision. Keep the earlier absolute floor for small parameter intervals.
pub fn parameter_domain_ratio(doc: &Document, target: &Value) -> f64 {
    let mut maximum = 0_f64;
    for row in target.as_array().unwrap() {
        if row.get("curve").is_none() {
            continue;
        }
        let object = doc
            .objects()
            .find(|o| {
                o.attributes().name().unwrap_or("") == row["attributes"]["name"].as_str().unwrap()
            })
            .unwrap();
        let domain = object.geometry().curve_ref().unwrap().domain();
        let [a, b]: [f64; 2] = serde_json::from_value(row["curve"]["domain"].clone()).unwrap();
        let error = (domain.start() - a).abs().max((domain.end() - b).abs());
        maximum = maximum.max(error / 2e-6_f64.max(1e-8 * (b - a).abs()));
    }
    maximum
}
/// Compare the actual geometry's length against an independent public Native
/// 1e-12 measurement. These observations never determine the parameter domain.
pub fn tight_length_error(doc: &Document, target: &Value) -> f64 {
    let mut maximum = 0_f64;
    for row in target.as_array().unwrap() {
        let Some(measurement) = row.get("length_measurements") else {
            continue;
        };
        let object = doc
            .objects()
            .find(|o| {
                o.attributes().name().unwrap_or("") == row["attributes"]["name"].as_str().unwrap()
            })
            .unwrap();
        let length = object
            .geometry()
            .curve_ref()
            .unwrap()
            .length(viboceros_geometry::Tolerance::DEFAULT)
            .unwrap();
        let tight = measurement["fractional"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["length"]
            .as_f64()
            .unwrap();
        maximum = maximum.max((length - tight).abs());
    }
    maximum
}
