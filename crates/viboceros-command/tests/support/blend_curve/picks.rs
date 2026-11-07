//! Input picks come from public pre-command receipts and prescribed source geometry.
use super::*;
fn edge_candidate(value: &Value, index: usize) -> &Value {
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let candidates = click["edge_pick_context"]["candidates"].as_array().unwrap();
    let mut ranked = candidates.iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        a["distance"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["distance"].as_f64().unwrap())
    });
    assert!(!ranked.is_empty(), "prescribed edge was captured");
    if ranked.len() > 1 {
        assert!(
            ranked[1]["distance"].as_f64().unwrap() - ranked[0]["distance"].as_f64().unwrap()
                > 1e-8,
            "prescribed edge has a unique nearest screen hit"
        );
    }
    ranked[0]
}
/// The physical click chooses the edge; authored indices in discovery recipes
/// are only intent metadata and never override a passive input receipt.
pub fn source_edge(value: &Value, index: usize, action: &Value) -> Option<usize> {
    action
        .get("edge")
        .map(|_| edge_candidate(value, index)["edge"].as_u64().unwrap() as usize)
}
fn measured(click: &Value) -> Option<viboceros_geometry::Point3> {
    let context = click.get("pick_context")?;
    assert_eq!(context["success"], true);
    let references = context["references"].as_array().unwrap();
    assert_eq!(references.len(), 1);
    assert_eq!(references[0]["selection_method"], 1);
    Some(native::point(&references[0]["point"]))
}
pub fn source_pick(
    value: &Value,
    index: usize,
    action: &Value,
    input: &Input,
) -> viboceros_geometry::Point3 {
    if action.get("edge").is_some() {
        return native::point(&edge_candidate(value, index)["point"]);
    }
    let source = &value["prescribed_case"]["curves"][action["curve"].as_u64().unwrap() as usize];
    let closed = reference_inputs::geometry(source)
        .curve_ref()
        .is_some_and(|c| c.is_closed().unwrap());
    if !input.preferences().pick_point && !closed {
        return native::point(&action["pick"]);
    }
    let click = value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    if let Some(point) = measured(click) {
        return point;
    }
    // A fresh, passive calibration may supply an earlier receipt's missing
    // parameter only when every recorded input below agrees exactly. No output
    // field participates in matching or determines the selected point.
    let calibration = native::read("blend_curve_point_pick_context");
    let mut calibrated = None;
    for row in calibration["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["prescribed_case"]["curves"] != value["prescribed_case"]["curves"]
            || v["prescribed_case"]["view"] != value["prescribed_case"]["view"]
            || v["construction_plane"] != value["construction_plane"]
            || v["model_tolerance"] != value["model_tolerance"]
        {
            continue;
        }
        for candidate in v["input_trace"][0]["clicks"].as_array().unwrap() {
            if candidate["curve"] != click["curve"]
                || candidate["pixel"] != click["pixel"]
                || candidate["ray"] != click["ray"]
            {
                continue;
            }
            if let Some(point) = measured(candidate) {
                if let Some(previous) = calibrated {
                    assert!(point.distance_to(previous).unwrap() < 1e-11);
                }
                calibrated = Some(point);
            }
        }
    }
    if let Some(point) = calibrated {
        return point;
    }
    native::point(&click["sight_point"])
}
