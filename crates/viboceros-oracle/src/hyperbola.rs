//! Authored getter sequences, exact output and actual undo/redo comparisons.
use super::*;
use crate::curve_primitive::snapshot_with_failed_lengths as snapshot;
use viboceros_command::hyperbola::{Coefficient, Construction, Input};
use viboceros_geometry::Real;
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([Real; 3]),
    Number(Real),
    Option(String),
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[Real; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
}
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: viboceros_geometry::Frame3,
    actions: &[Action],
) -> Result<bool, ProbeError> {
    let mut input = Input::new(commands.hyperbola_options(), plane, doc.tolerance());
    let mut pending = None;
    let mut complete = false;
    for (index, action) in actions.iter().enumerate() {
        if complete {
            return Err(ProbeError::FixtureInvariant(
                "hyperbola actions after completion",
            ));
        }
        match action {
            Action::Point(point) => {
                if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant("coefficient needs a number"));
                }
                complete = input.point(Point3::try_from(*point)?)?.is_some();
            }
            Action::Number(value) => {
                let key = pending
                    .take()
                    .ok_or(ProbeError::FixtureInvariant("coefficient getter required"))?;
                input.coefficient(key, *value)?;
            }
            Action::Option(word) => {
                if input.cancels(word) {
                    if index + 1 != actions.len() {
                        return Err(ProbeError::FixtureInvariant(
                            "inputs after cancelled hyperbola",
                        ));
                    }
                    return Ok(false);
                }
                if pending.is_some() && word == "Enter" {
                    pending = None;
                    continue;
                }
                if input.construction() == Construction::Coefficient && (word == "A" || word == "B")
                {
                    pending = Some(if word == "A" {
                        Coefficient::A
                    } else {
                        Coefficient::B
                    });
                    continue;
                }
                if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unavailable hyperbola option"));
                }
            }
        }
    }
    if !complete {
        return Err(ProbeError::FixtureInvariant("incomplete hyperbola input"));
    }
    commands.execute_in_context(
        doc,
        &input.command(),
        viboceros_command::CommandContext {
            construction_plane: plane,
        },
    )?;
    Ok(true)
}
pub(super) fn run(
    f: &Fixture,
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
            enter(&mut doc, &commands, plane, setup)?;
            doc.undo()?;
        }
        doc.clear_history()?;
        let before = snapshot(&doc, &[])?;
        let completed = enter(&mut doc, &commands, plane, &f.actions)?;
        let after = snapshot(&doc, &[])?;
        let curve_diagnostics=doc.objects().filter_map(|o|if let Geometry::NurbsCurve(c)=o.geometry(){Some(json!({"valid":true,"midpoint_valid":c.evaluate(0.5*(*c.domain().start()+*c.domain().end())).is_ok()}))}else{None}).collect::<Vec<_>>();
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc, &[])?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc, &[])?;
        Ok(
            json!({"command_returned":completed,"curve_diagnostics":curve_diagnostics,"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
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
    fn hyperbola_commands_match_native_geometry_focus_history_and_projective_poles() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/hyperbola_command.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/hyperbola_command_rhino_20261017.json"
        ))
        .unwrap();
        assert_eq!(request.operations.len(), 25);
        assert_eq!(native["results"].as_array().unwrap().len(), 25);
        let actual = run_request(&request).unwrap();
        for ((operation, a), n) in request
            .operations
            .iter()
            .zip(&actual.results)
            .zip(native["results"].as_array().unwrap())
        {
            let Operation::HyperbolaCommand { .. } = operation else {
                panic!()
            };
            assert_eq!(a.id, n["id"]);
            let x = &a.value;
            let y = &n["value"];
            assert_eq!(y["command_returned"], true, "{}", a.id);
            let events = y["events"].as_array().unwrap();
            assert_eq!(events.len(), 1, "{}", a.id);
            assert_eq!(events[0]["name"], "Hyperbola");
            assert_eq!(events[0]["result"], "Success");
            assert_eq!(y["after"], y["after_script"], "{}", a.id);
            assert!(
                !y["after"]["outputs"].as_array().unwrap().is_empty(),
                "{}",
                a.id
            );
            for token in ["_Cancel", "Unknown command", "Invalid"] {
                assert!(!y["history"].as_str().unwrap().contains(token), "{}", a.id);
            }
            assert_eq!(y["undo_returned"], true);
            assert_eq!(y["redo_returned"], true);
            assert_eq!(x["undo_label"], "Hyperbola");
            assert_eq!(x["redo_label"], "Hyperbola");
            for stage in ["before", "after", "undo", "redo"] {
                assert!(
                    close(&x[stage], &y[stage]),
                    "{} {stage}: {} != {}",
                    a.id,
                    x[stage],
                    y[stage]
                );
            }
        }
    }

    #[test]
    fn hyperbola_native_planes_signed_extents_focus_only_and_rejections() {
        for (request, native) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_planes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_planes_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_directions.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_directions_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_gates.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_gates_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_shared_driver.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_shared_driver_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_projective.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_projective_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_boundaries.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_boundaries_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/hyperbola_mode_cancel.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/hyperbola_mode_cancel_rhino_20261017.json"
                ),
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(request).unwrap();
            let native: Value = serde_json::from_str(native).unwrap();
            assert_eq!(
                request.operations.len(),
                native["results"].as_array().unwrap().len()
            );
            for (operation, native) in request
                .operations
                .iter()
                .zip(native["results"].as_array().unwrap())
            {
                let Operation::HyperbolaCommand { id, fixture } = operation else {
                    panic!()
                };
                assert_eq!(id.as_str(), native["id"].as_str().unwrap());
                let v = &native["value"];
                let mut one = request.clone();
                one.operations = vec![operation.clone()];
                let actual = run_request(&one);
                if v["command_returned"] == false {
                    assert!(v["after"]["outputs"].as_array().unwrap().is_empty());
                    assert_eq!(v["before"], v["after"]);
                    assert_eq!(v["after"], v["undo"]);
                    assert_eq!(v["undo"], v["redo"]);
                    if let Ok(actual) = actual {
                        let a = &actual.results[0].value;
                        assert_eq!(a["command_returned"], false, "{id}");
                        assert_eq!(a["undo_label"], Value::Null);
                        assert_eq!(a["redo_label"], Value::Null);
                        for stage in ["before", "after", "undo", "redo"] {
                            assert!(close(&a[stage], &v[stage]), "{id} {stage}");
                        }
                    } else {
                        // Execute rejected authored inputs against an existing model
                        // too, proving public command transactions leave no helpers.
                        let mut doc = Document::new(request.tolerance.geometry().unwrap());
                        let source = doc
                            .add_geometry(Geometry::Point(Point3::try_new(20., 30., 40.).unwrap()))
                            .unwrap();
                        doc.clear_history().unwrap();
                        let mut words = vec!["Hyperbola".to_owned()];
                        for action in &fixture.actions {
                            words.push(match action {
                                Action::Point([x, y, z]) => format!("{x},{y},{z}"),
                                Action::Number(n) => n.to_string(),
                                Action::Option(w) => w.clone(),
                            });
                        }
                        let plane = if let Some([p, x, y]) = fixture.construction_plane {
                            viboceros_geometry::Frame3::try_from_directions(
                                Point3::try_from(p).unwrap(),
                                Vector3::try_from(x).unwrap(),
                                Vector3::try_from(y).unwrap(),
                                doc.tolerance(),
                            )
                            .unwrap()
                        } else {
                            viboceros_command::CommandContext::default().construction_plane
                        };
                        let commands = CommandRegistry::with_builtins();
                        assert!(
                            commands
                                .execute_in_context(
                                    &mut doc,
                                    &words.join(" "),
                                    viboceros_command::CommandContext {
                                        construction_plane: plane
                                    }
                                )
                                .is_err(),
                            "{id}"
                        );
                        assert_eq!(doc.objects().count(), 1);
                        assert!(doc.object(source).is_some());
                        assert!(doc.undo_label().is_none());
                    }
                    continue;
                }
                let actual = actual.unwrap_or_else(|e| panic!("{id}: {e}"));
                let a = &actual.results[0].value;
                assert_eq!(a["command_returned"], true);
                if v.get("curve_diagnostics").is_some() {
                    assert_eq!(a["curve_diagnostics"], v["curve_diagnostics"], "{id}");
                }
                let label = if v["after"]["outputs"].as_array().unwrap().is_empty() {
                    Value::Null
                } else {
                    json!("Hyperbola")
                };
                assert_eq!(a["undo_label"], label, "{id}");
                assert_eq!(a["redo_label"], label, "{id}");
                for stage in ["before", "after", "undo", "redo"] {
                    assert!(
                        close(&a[stage], &v[stage]),
                        "{id} {stage}: {} != {}",
                        a[stage],
                        v[stage]
                    );
                }
            }
        }
    }

    #[test]
    fn conic_shared_driver_smoke_retains_real_hover_edge_receipts_and_geometry() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/conic_shared_driver.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/conic_shared_driver_rhino_20261017.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 2);
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            let v = &n["value"];
            assert_eq!(v["command_returned"], true);
            assert_eq!(v["input_trace"]["errors"], json!([]));
            assert_eq!(v["input_trace"]["completed"], v["input_trace"]["expected"]);
            for stage in ["before", "after", "undo", "redo"] {
                assert!(close(&a.value[stage], &v[stage]), "{} {stage}", a.id);
            }
        }
    }
}
