//! Authored Conic inputs, independent of the captured expected geometry.
use super::*;
use crate::{curve_primitive::snapshot, object_source::ObjectSource};
use viboceros_geometry::Real;
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([Real; 3]),
    Number(Real),
    Option(String),
    Hover {
        hover: [Real; 3],
    },
    Pick {
        curve: usize,
        #[serde(default)]
        edge: Option<usize>,
        #[serde(default)]
        pick: Option<[Real; 3]>,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub curves: Vec<ObjectSource>,
    #[serde(default)]
    pub construction_plane: Option<[[Real; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
}
fn command(
    actions: &[Action],
    guides: &[(ObjectId, Geometry)],
    tolerance: Tolerance,
) -> Result<String, ProbeError> {
    let mut words = vec!["Conic".to_owned()];
    for action in actions {
        words.push(match action {
            Action::Point([x, y, z]) => format!("{x},{y},{z}"),
            Action::Number(n) => n.to_string(),
            Action::Option(w) => w.clone(),
            Action::Hover { hover: [x, y, z] } => format!("Hover={x},{y},{z}"),
            Action::Pick { curve, edge, pick } => {
                let (id, geometry) = guides
                    .get(*curve)
                    .ok_or(ProbeError::FixtureInvariant("missing conic reference"))?;
                if let (Some(index), Some(pick)) = (edge, pick) {
                    let reference = viboceros_command::curve_reference::resolve(
                        geometry,
                        Some(*index),
                        tolerance,
                    )
                    .ok_or(ProbeError::FixtureInvariant("missing conic edge"))?;
                    let curve = reference.curve();
                    let point = Point3::try_from(*pick)?;
                    let t = curve.closest_parameter(point, Tolerance::NUMERICAL_VALIDATION)?;
                    if curve.evaluate(t)?.distance_to(point)? > tolerance.absolute() {
                        return Err(ProbeError::FixtureInvariant(
                            "Conic pick position must lie on the named edge",
                        ));
                    }
                }
                format!(
                    "{}={}{}",
                    if edge.is_some() { "Edge" } else { "Curve" },
                    id,
                    edge.map_or_else(String::new, |i| format!(",{i}"))
                )
            }
        });
    }
    Ok(words.join(" "))
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let construction_plane = if let Some([p, x, y]) = f.construction_plane {
            viboceros_geometry::Frame3::try_from_directions(
                Point3::try_from(p)?,
                Vector3::try_from(x)?,
                Vector3::try_from(y)?,
                tolerance,
            )?
        } else {
            viboceros_command::CommandContext::default().construction_plane
        };
        let context = viboceros_command::CommandContext { construction_plane };
        let mut guides = Vec::new();
        for source in &f.curves {
            let geometry = source.geometry(tolerance)?;
            let id = doc.add_geometry(geometry.clone())?;
            guides.push((id, geometry));
        }
        if let Some(setup) = &f.setup {
            commands.execute_in_context(&mut doc, &command(setup, &guides, tolerance)?, context)?;
            doc.undo()?;
        }
        doc.clear_history()?;
        let before = snapshot(&doc, &guides)?;
        commands.execute_in_context(
            &mut doc,
            &command(&f.actions, &guides, tolerance)?,
            context,
        )?;
        let after = snapshot(&doc, &guides)?;
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc, &guides)?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc, &guides)?;
        Ok(
            json!({"command_returned":true,"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6
            }
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|b| close(v, b)))
            }
            _ => a == b,
        }
    }
    #[test]
    fn conic_getters_match_native_direction_edges_and_history() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/conic_getters.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/conic_getters_rhino_20261017.json"
        ))
        .unwrap();
        assert_eq!(request.operations.len(), 28);
        assert_eq!(native["results"].as_array().unwrap().len(), 28);
        let mut failures = Vec::new();
        for (operation, n) in request
            .operations
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let mut one = request.clone();
            one.operations = vec![operation.clone()];
            let actual = match run_request(&one) {
                Ok(v) => v,
                Err(e) => {
                    failures.push(format!("{}: {e}", n["id"]));
                    continue;
                }
            };
            let a = &actual.results[0];
            assert_eq!(a.id, n["id"]);
            let x = &a.value;
            let y = &n["value"];
            assert_eq!(y["command_returned"], true);
            assert_eq!(
                y["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|e| e["name"] == "Conic")
                    .map(|e| e["result"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["Success"]
            );
            assert_eq!(y["after"], y["after_script"]);
            assert_eq!(y["undo_returned"], true);
            assert_eq!(y["redo_returned"], true);
            let trace = &y["input_trace"];
            assert_eq!(trace["completed"], trace["expected"]);
            assert_eq!(trace["errors"], json!([]));
            for token in ["_Cancel", "Unknown command", "Invalid"] {
                assert!(!y["history"].as_str().unwrap().contains(token));
            }
            assert_eq!(x["undo_label"], "Conic");
            assert_eq!(x["redo_label"], "Conic");
            for stage in ["before", "after", "undo", "redo"] {
                let ns = if stage == "after" {
                    "after_script"
                } else {
                    stage
                };
                if !close(&x[stage], &y[ns]) {
                    failures.push(format!("{} {stage}: {} != {}", a.id, x[stage], y[ns]));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
    #[test]
    fn conic_command_matches_native_curves_constraints_and_actual_undo_redo() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/conic_command.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/conic_command_rhino_20261017.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 42);
        assert_eq!(native["results"].as_array().unwrap().len(), 42);
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            let x = &a.value;
            let y = &n["value"];
            assert_eq!(y["command_returned"], true, "{}", a.id);
            assert_eq!(
                y["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|e| e["name"] == "Conic")
                    .map(|e| e["result"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["Success"],
                "{}",
                a.id
            );
            let history = y["history"].as_str().unwrap();
            for token in ["_Cancel", "Unknown command", "Invalid"] {
                assert!(!history.contains(token), "{} {history}", a.id);
            }
            assert_eq!(y["after"], y["after_script"]);
            assert_eq!(y["undo_returned"], true);
            assert_eq!(y["redo_returned"], true);
            assert_eq!(x["undo_label"], "Conic");
            assert_eq!(x["redo_label"], "Conic");
            for stage in ["before", "after", "undo", "redo"] {
                let nstage = if stage == "after" {
                    "after_script"
                } else {
                    stage
                };
                assert!(
                    close(&x[stage], &y[nstage]),
                    "{} {stage}: {} != {}",
                    a.id,
                    x[stage],
                    y[nstage]
                );
            }
        }
    }
}
