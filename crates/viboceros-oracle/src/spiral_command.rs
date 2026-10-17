//! Public Helix/Spiral point workflows and exact model/history witnesses.
use super::*;
use crate::curve_primitive::snapshot;
use viboceros_command::spiral_input::{Input, Kind, Numeric};
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([f64; 3]),
    Number(f64),
    Option(String),
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[f64; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
}
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    kind: Kind,
    plane: viboceros_geometry::Frame3,
    actions: &[Action],
) -> Result<(), ProbeError> {
    let mut input = Input::new(kind, commands.spiral_options(kind), plane, doc.tolerance());
    let mut pending = None;
    let mut complete = false;
    for action in actions {
        if complete {
            return Err(ProbeError::FixtureInvariant(
                "spiral actions after completion",
            ));
        }
        match action {
            Action::Point(p) => {
                if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant("spiral option needs a value"));
                }
                complete = input.point(Point3::try_from(*p)?)?;
            }
            Action::Number(n) => {
                if let Some(key) = pending.take() {
                    match key {
                        "Turns" => input.numeric(Numeric::Turns, *n)?,
                        "Pitch" => input.numeric(Numeric::Pitch, *n)?,
                        _ => return Err(ProbeError::FixtureInvariant("numeric mode")),
                    }
                } else {
                    complete = input.number(*n)?;
                }
            }
            Action::Option(word) => {
                if pending.is_some() && word == "Enter" {
                    pending = None;
                    continue;
                }
                if let Some(key) = pending.take() {
                    if !input.option(&format!("{key}={word}"))? {
                        return Err(ProbeError::FixtureInvariant("unavailable spiral option"));
                    }
                } else if word == "Enter" {
                    complete = input.enter()?;
                } else if ["Turns", "Pitch"].contains(&word.as_str()) {
                    pending = Some(match word.as_str() {
                        "Mode" => "Mode",
                        "Turns" => "Turns",
                        _ => "Pitch",
                    });
                } else if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unavailable spiral option"));
                }
            }
        }
    }
    if !complete {
        return Err(ProbeError::FixtureInvariant("incomplete spiral input"));
    }
    commands.execute(doc, &input.command()?)?;
    commands.remember_spiral_options(kind, input.options());
    Ok(())
}
pub(super) fn run(
    f: &Fixture,
    kind: Kind,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let plane = if let Some([p, x, y]) = f.construction_plane {
            viboceros_geometry::Frame3::try_from_directions(
                Point3::try_from(p)?,
                Vector3::try_from(x)?,
                Vector3::try_from(y)?,
                tolerance,
            )?
        } else {
            viboceros_command::CommandContext::default().construction_plane
        };
        if let Some(setup) = &f.setup {
            enter(&mut doc, &commands, kind, plane, setup)?;
            doc.undo()?;
        }
        doc.clear_history()?;
        let before = snapshot(&doc, &[])?;
        enter(&mut doc, &commands, kind, plane, &f.actions)?;
        let after = snapshot(&doc, &[])?;
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc, &[])?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc, &[])?;
        Ok(
            json!({"command_returned":true,"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: &Value, b: &Value, path: &str) {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6,
                "{path}: {a} != {b}"
            ),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    close(a, b, &format!("{path}/{i}"));
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (k, a) in a {
                    close(a, b.get(k).unwrap(), &format!("{path}/{k}"));
                }
            }
            _ => assert_eq!(a, b, "{path}"),
        }
    }
    #[test]
    fn spiral_sampling_matches_native_knots_domains_density_and_reports_endpoint_limits() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/spiral_sampling.json"
        ))
        .unwrap();
        let n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/spiral_sampling_rhino_20261017.json"
        ))
        .unwrap();
        let a = run_request(&q).unwrap();
        assert_eq!(a.results.len(), 45);
        let mut limits = Vec::new();
        for (a, n) in a.results.iter().zip(n["results"].as_array().unwrap()) {
            assert_eq!(a.id, n["id"]);
            let n = &n["value"];
            assert_eq!(a.value["degree"], n["degree"]);
            close(&a.value["knots"], &n["knots"], &a.id);
            close(&a.value["domain"], &n["domain"], &a.id);
            let actual = a.value["control_points"].as_array().unwrap();
            let expected = n["control_points"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{}", a.id);
            let mut maximum = 0_f64;
            for (p, q) in actual.iter().zip(expected) {
                assert_eq!(p["weight"], q["weight"]);
                let error = (0..3)
                    .map(|i| {
                        (p["point"][i].as_f64().unwrap() - q["point"][i].as_f64().unwrap()).powi(2)
                    })
                    .sum::<f64>()
                    .sqrt();
                maximum = maximum.max(error);
            }
            // These are diagnostics of the retained C2 analytic endpoint rule,
            // not claims of full 1e-6 control agreement for large radii/turn counts.
            assert!(maximum <= 3e-5, "{}: {maximum}", a.id);
            if maximum > 1e-6 {
                limits.push(a.id.as_str());
            }
        }
        assert_eq!(
            limits,
            ["dim-100-10", "t100", "density-99p99", "density-1000"]
        );
    }
    #[test]
    fn spiral_sampling_public_sdk_layout_boundaries() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/spiral_layout.json"
        ))
        .unwrap();
        let n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/spiral_layout_rhino_20261017.json"
        ))
        .unwrap();
        let a = run_request(&q).unwrap();
        assert_eq!(a.results.len(), 32);
        for (a, n) in a.results.iter().zip(n["results"].as_array().unwrap()) {
            assert_eq!(a.id, n["id"]);
            assert_eq!(a.value, n["value"], "{}", a.id);
        }
    }
    #[test]
    fn spiral_command_native_getters_geometry_selection_and_history() {
        for (request, native, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/spiral_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/spiral_command_rhino_20261017.json"
                ),
                22,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/spiral_preferences.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/spiral_preferences_rhino_20261017.json"
                ),
                3,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/spiral_defaults.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/spiral_defaults_rhino_20261017.json"
                ),
                1,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(request).unwrap();
            let n: Value = serde_json::from_str(native).unwrap();
            let a = run_request(&q).unwrap();
            assert_eq!(a.results.len(), count);
            assert_eq!(a.results.len(), n["results"].as_array().unwrap().len());
            for ((op, a), n) in q
                .operations
                .iter()
                .zip(a.results)
                .zip(n["results"].as_array().unwrap())
            {
                assert_eq!(a.id, n["id"]);
                let command = if matches!(op, Operation::HelixCommand { .. }) {
                    "Helix"
                } else {
                    "Spiral"
                };
                let n = &n["value"];
                assert_eq!(n["command_returned"], true, "{}", a.id);
                if let Some(trace) = n.get("input_trace") {
                    assert_eq!(trace["errors"], json!([]), "{}", a.id);
                }
                assert_eq!(n["before"], n["undo"]);
                assert_eq!(n["after"], n["after_script"]);
                assert_eq!(n["after"], n["redo"]);
                assert_eq!(a.value["undo_label"], command);
                assert_eq!(a.value["redo_label"], command);
                for stage in ["before", "after", "undo", "redo"] {
                    close(&a.value[stage], &n[stage], &format!("{}/{stage}", a.id));
                }
            }
        }
    }
}
