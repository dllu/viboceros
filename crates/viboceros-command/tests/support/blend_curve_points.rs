//! Point endpoint captures, including the incomplete two-point selector probe.
pub const FAMILIES: [&str; 6] = [
    "blend_curve_point_pilot",
    "blend_curve_point_discovery",
    "blend_curve_point_holdouts",
    "blend_curve_point_ownership",
    "blend_curve_point_modes",
    "blend_curve_point_pick_context",
];
pub fn diagnostic(name: &str) -> bool {
    name == "point_two_points"
}

pub fn preferences(v: &serde_json::Value) -> viboceros_command::blend_curve::Preferences {
    // Public initial getter state is input evidence; output geometry never seeds picks.
    viboceros_command::blend_curve::Preferences {
        pick_point: v["input_trace"][0]["prompts"][0]
            .as_str()
            .unwrap()
            .contains("BlendStart=PickPoint"),
        continuity: initial_continuities(v),
        ..Default::default()
    }
}

pub fn initial_continuities(value: &serde_json::Value) -> [u8; 2] {
    let prompt = value["input_trace"][0]["prompts"][0].as_str().unwrap();
    std::array::from_fn(|i| {
        prompt
            .split(&format!("Continuity_{}=G", i + 1))
            .nth(1)
            .and_then(|s| s.chars().next())
            .and_then(|c| c.to_digit(10))
            .map_or(2, |c| c as u8)
    })
}

pub fn identities(
    doc: &viboceros_document::Document,
) -> std::collections::BTreeMap<String, viboceros_document::ObjectId> {
    doc.objects()
        .map(|o| (o.attributes().name().unwrap_or("").to_owned(), o.id()))
        .collect()
}
pub fn check_identities(
    doc: &viboceros_document::Document,
    before: &std::collections::BTreeMap<String, viboceros_document::ObjectId>,
    target: &serde_json::Value,
) {
    let after = identities(doc);
    for row in target.as_array().unwrap() {
        let name = row["attributes"]["name"].as_str().unwrap();
        let id = after[name];
        if let Some(expected) = before.get(name) {
            assert_eq!(id, *expected, "{name} identity");
        } else {
            assert!(!before.values().any(|old| *old == id), "new blend identity");
        }
    }
    for (name, id) in before {
        if !after.contains_key(name) {
            assert!(doc.object(*id).is_none(), "consumed {name}");
        }
    }
}
