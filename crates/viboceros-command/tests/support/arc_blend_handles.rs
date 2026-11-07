//! Original scene recipes and public cursor calibration, independent of outputs.
use super::{native, reference_inputs};
use serde_json::Value;
use viboceros_command::arc_blend::{Handle, Input};
use viboceros_geometry::Point3;
pub const FAMILIES: [&str; 5] = [
    "arc_blend_handles",
    "arc_blend_handle_getters",
    "arc_blend_handle_holdouts",
    "arc_blend_balance",
    "arc_blend_chord_handles",
];
pub fn check(doc: &viboceros_document::Document, target: &Value, archive: bool) -> f64 {
    // Rhino replacement keeps object IDs but assigns new runtime serials.
    // Compare this owned scene by its unique names, without claiming parity
    // for the table's enumeration order after replacements and Undo.
    let rows = target.as_array().unwrap();
    assert_eq!(doc.objects().len(), rows.len());
    let mut by_name = std::collections::BTreeMap::new();
    for row in rows {
        assert!(
            by_name
                .insert(row["attributes"]["name"].as_str().unwrap(), row.clone())
                .is_none()
        );
    }
    let ordered = doc
        .objects()
        .map(|o| by_name.remove(o.attributes().name().unwrap_or("")).unwrap())
        .collect::<Vec<_>>();
    assert!(by_name.is_empty());
    super::replay::check(doc, &Value::Array(ordered), archive)
}
pub fn diagnostic(v: &Value) -> Option<&'static str> {
    let trace = &v["input_trace"][0];
    if trace["errors"] != serde_json::json!([]) {
        return Some("incomplete getter driver");
    }
    let actions = v["prescribed_case"]["actions"].as_array().unwrap();
    if actions.iter().any(|a| {
        a.as_f64() == Some(0.)
            || a.as_str()
                .is_some_and(|s| s.starts_with("RadiusDifference"))
    }) || v["history"]
        .as_str()
        .unwrap()
        .contains("Unable to achieve requested radius difference")
    {
        return Some("automatic radius solving remains unsupported");
    }
    if v["history"].as_str().unwrap().contains("Unknown command") {
        return Some("native rejected the input");
    }
    for (index, a) in actions.iter().enumerate().skip(2) {
        if a.is_array() || a.is_number() {
            let prompt = trace["prompts"][index].as_str().unwrap();
            if ![
                "Select start point location",
                "Select end point location",
                "Set arc control polygon",
            ]
            .iter()
            .any(|s| prompt.starts_with(s))
            {
                return Some("intended adjustment handle was not selected");
            }
        }
        if a.get("click").is_some()
            && index > 2
            && actions[index - 1].get("click").is_some()
            && !trace["prompts"][index]
                .as_str()
                .unwrap()
                .starts_with("Set arc control polygon")
        {
            return Some("intended adjustment handle was not selected");
        }
    }
    None
}
pub fn handle(input: &Input, point: Point3) -> Handle {
    input
        .handles()
        .unwrap()
        .into_iter()
        .zip([Handle::Start, Handle::Ratio, Handle::End])
        .min_by(|a, b| {
            a.0.distance_to(point)
                .unwrap()
                .total_cmp(&b.0.distance_to(point).unwrap())
        })
        .unwrap()
        .1
}
pub fn parameter(v: &Value, index: usize, input: &Input) -> f64 {
    let click = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    let curve = input.adjustment_curve().unwrap();

    // Free chord movement uses the public viewing line, including in
    // Perspective. The constructed output never seeds this query.
    let p0 = curve.evaluate(*curve.domain().start()).unwrap();
    let ray0 = native::point(&click["ray"][0]);
    let ray1 = native::point(&click["ray"][1]);
    let t = input
        .ratio_parameter_in_view(p0.vector_to(ray0).unwrap(), ray0.vector_to(ray1).unwrap())
        .unwrap();
    let screen_difference = click.get("pick_transform").map(|matrix| {
        let matrix: [[f64; 4]; 4] = serde_json::from_value(matrix.clone()).unwrap();
        let projected = curve
            .closest_parameter_in_projection([0., 0.], |p| {
                let [x, y, z] = p.to_array();
                let h = [x, y, z, 1.];
                let q: [f64; 4] =
                    std::array::from_fn(|i| matrix[i].iter().zip(h).map(|(a, b)| a * b).sum());
                (q[3] > 0.).then(|| [q[0] / q[3], q[1] / q[3]])
            })
            .unwrap();
        curve
            .evaluate(projected)
            .unwrap()
            .distance_to(curve.evaluate(t).unwrap())
            .unwrap()
    });
    println!(
        "ARC_BLEND_HANDLE_VIEW_LINE {}",
        serde_json::json!({"marker":click["marker"],"parameter":t,"screen_metric_station_difference":screen_difference})
    );
    t
}
pub fn replay(input: &mut Input, doc: &viboceros_document::Document, v: &Value) {
    for (index, action) in v["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if let Some(i) = action["curve"].as_u64() {
            input
                .pick_curve(
                    doc,
                    reference_inputs::guide(doc, i as usize),
                    None,
                    native::point(&action["pick"]),
                )
                .unwrap();
        } else if action.get("hover").is_some() {
            continue;
        } else if let Some(p) = action.get("click") {
            if input.active_handle().is_some() {
                input.adjust_parameter(parameter(v, index, input)).unwrap();
                input.finish_adjustment();
            } else {
                input
                    .select_handle(handle(input, native::point(p)))
                    .unwrap();
            }
        } else if action.is_array() {
            input.adjust_point(native::point(action)).unwrap();
            input.finish_adjustment();
        } else if action.is_number() {
            input.option(&format!("Ratio={action}")).unwrap();
            input.finish_adjustment();
        } else if action != "Enter" {
            assert!(input.option(action.as_str().unwrap()).unwrap());
        }
    }
}
