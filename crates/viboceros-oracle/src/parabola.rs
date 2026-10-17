//! Authored getter sequences, exact output and actual undo/redo comparisons.
use super::*;
use crate::curve_primitive::snapshot;
use viboceros_command::parabola::{Input, Kind};
use viboceros_geometry::Real;
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([Real; 3]),
    Option(String),
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub command: String,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[Real; 3]; 3]>,
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
    let mut input = Input::new(
        kind,
        commands.parabola_options(kind),
        plane,
        doc.tolerance(),
    );
    let mut complete = false;
    for action in actions {
        if complete {
            return Err(ProbeError::FixtureInvariant(
                "parabola actions after completion",
            ));
        }
        match action {
            Action::Point(point) => {
                complete = input.point(Point3::try_from(*point)?)?.is_some();
            }
            Action::Option(word) => {
                if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unavailable parabola option"));
                }
            }
        }
    }
    if !complete {
        return Err(ProbeError::FixtureInvariant("incomplete parabola input"));
    }
    commands.execute(doc, &input.command())?;
    Ok(())
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let kind =
            Kind::from_name(&f.command).ok_or(ProbeError::FixtureInvariant("parabola command"))?;
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
            json!({"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
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
    fn parabola_command_getters_match_native_geometry_focus_and_history() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/parabola_command.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/parabola_command_rhino_20261017.json"
        ))
        .unwrap();
        assert_eq!(request.operations.len(), 24);
        assert_eq!(native["results"].as_array().unwrap().len(), 24);
        let actual = run_request(&request).unwrap();
        for ((operation, a), n) in request
            .operations
            .iter()
            .zip(&actual.results)
            .zip(native["results"].as_array().unwrap())
        {
            let Operation::ParabolaCommand { fixture, .. } = operation else {
                panic!()
            };
            assert_eq!(a.id, n["id"]);
            let x = &a.value;
            let y = &n["value"];
            assert_eq!(y["command_returned"], true, "{}", a.id);
            let events = y["events"].as_array().unwrap();
            assert_eq!(events.len(), 1, "{}", a.id);
            assert_eq!(events[0]["name"], fixture.command);
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
            assert_eq!(x["undo_label"], fixture.command);
            assert_eq!(x["redo_label"], fixture.command);
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
}
