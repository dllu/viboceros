pub const FAMILIES: [&str; 6] = [
    "blend_curve_closed_single",
    "blend_curve_closed_discovery",
    "blend_curve_closed_verification",
    "blend_curve_closed_holdouts",
    "blend_curve_closed_ownership",
    "blend_curve_closed_self_flips",
];
pub fn included(_family: &str, _name: &str) -> bool {
    true
}
pub fn domain_diagnostic(_family: &str, _name: &str) -> bool {
    false
}
pub fn rejected_option(value: &serde_json::Value, action: &serde_json::Value) -> bool {
    action.as_str().is_some_and(|word| {
        value["history"]
            .as_str()
            .unwrap()
            .contains(&format!("Unknown command: _{word}"))
    })
}
pub fn replay(
    input: &mut viboceros_command::blend_curve::Input,
    doc: &viboceros_document::Document,
    value: &serde_json::Value,
) -> usize {
    let mut getter = super::blend::Getter::default();
    let mut rejected = 0;
    for (i, a) in value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if rejected_option(value, a) {
            let preview = input.preview().cloned();
            let arguments = input.arguments().unwrap();
            let prefs = input.preferences();
            assert!(
                input
                    .option_with_units(a.as_str().unwrap(), doc.units())
                    .is_err()
            );
            assert_eq!(input.preview(), preview.as_ref());
            assert_eq!(input.arguments().unwrap(), arguments);
            assert_eq!(input.preferences(), prefs);
            rejected += 1;
        } else if !getter.action(input, doc, value, i, a) {
            break;
        }
    }
    rejected
}

/// RhinoCommon pads its short knot vector by duplication; OpenNURBS reconstructs
/// superfluous periodic knots by extrapolation when reading 3DM. Both unused
/// entries are outside the active basis. Retain their discrepancy separately,
/// and keep every control, weight and active knot in the ordinary strict gate.
pub fn check(
    doc: &viboceros_document::Document,
    target: &serde_json::Value,
    archive: bool,
) -> (f64, f64, f64) {
    let mut expected = target.clone();
    let mut padding = 0_f64;
    let objects = doc
        .objects()
        .map(|o| (o.attributes().name().unwrap_or(""), o))
        .collect::<std::collections::BTreeMap<_, _>>();
    for row in expected.as_array_mut().unwrap() {
        if row.get("curve").is_none() {
            continue;
        }
        let object = objects[row["attributes"]["name"].as_str().unwrap()];
        let actual = object.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        let native = super::native::curve(&row["curve"]);
        if !actual.is_periodic() || !native.is_periodic() {
            continue;
        }
        let a = *actual.domain().start();
        let width = actual.domain().end() - a;
        let [start, end]: [f64; 2] =
            serde_json::from_value(row["curve"]["domain"].clone()).unwrap();
        let knots = row["curve"]["knots"].as_array_mut().unwrap();
        for index in [0, knots.len() - 1] {
            let mapped = start + (end - start) * ((actual.knots()[index] - a) / width);
            padding = padding.max((mapped - knots[index].as_f64().unwrap()).abs());
            knots[index] = serde_json::json!(mapped);
        }
    }
    let (geometry, domain) = super::blend::check_high(doc, &expected, archive);
    (geometry, domain, padding)
}
