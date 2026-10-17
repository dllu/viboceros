//! Authored public Rectangle command inputs, exact curve form and history.
use super::*;
use crate::{object_source::ObjectSource, surface_flow::RectangleAction};
use viboceros_geometry::Real;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<RectangleAction>,
    #[serde(default)]
    pub construction_plane: Option<[[Real; 3]; 3]>,
    #[serde(default)]
    pub curve: Option<ObjectSource>,
    #[serde(default)]
    pub setup: Option<Vec<RectangleAction>>,
}
fn command(actions: &[RectangleAction], guide: Option<ObjectId>) -> String {
    let mut words = vec!["Rectangle".to_owned()];
    for action in actions {
        match action {
            RectangleAction::Point([x, y, z]) => words.push(format!("{x},{y},{z}")),
            RectangleAction::Number(n) => words.push(n.to_string()),
            RectangleAction::Option(word) => {
                words.push(word.clone());
                if word == "AroundCurve"
                    && let Some(id) = guide
                {
                    words.push(format!("Curve={id}"));
                }
            }
        }
    }
    words.join(" ")
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let frame = if let Some([p, x, y]) = f.construction_plane {
            viboceros_geometry::Frame3::try_from_directions(
                Point3::try_from(p)?,
                Vector3::try_from(x)?,
                Vector3::try_from(y)?,
                tolerance,
            )?
        } else {
            viboceros_command::CommandContext::default().construction_plane
        };
        let context = viboceros_command::CommandContext {
            construction_plane: frame,
        };
        let original = f
            .curve
            .as_ref()
            .map(|s| s.geometry(tolerance))
            .transpose()?;
        let guide = original
            .as_ref()
            .map(|g| doc.add_geometry(g.clone()))
            .transpose()?;
        let guides = guide.zip(original).into_iter().collect::<Vec<_>>();
        if let Some(setup) = &f.setup {
            commands.execute_in_context(&mut doc, &command(setup, guide), context)?;
            doc.undo()?;
        }
        doc.clear_history()?;
        let before = crate::curve_primitive::snapshot(&doc, &guides)?;
        commands.execute_in_context(&mut doc, &command(&f.actions, guide), context)?;
        let after = crate::curve_primitive::snapshot(&doc, &guides)?;
        let undo_label = doc.undo()?;
        let undo = crate::curve_primitive::snapshot(&doc, &guides)?;
        let redo_label = doc.redo()?;
        let redo = crate::curve_primitive::snapshot(&doc, &guides)?;
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
    fn normalize_domains(value: &mut Value) {
        for piece in value["outputs"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .flat_map(|o| o["segments"].as_array_mut().unwrap())
        {
            let a = piece["domain"][0].as_f64().unwrap();
            let b = piece["domain"][1].as_f64().unwrap();
            for knot in piece["knots"].as_array_mut().unwrap() {
                *knot = json!((knot.as_f64().unwrap() - a) / (b - a));
            }
            piece["domain"] = json!([0., 1.]);
        }
    }
    #[test]
    fn rounded_rectangles_match_native_geometry_and_history() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/rectangle_rounded.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/rectangle_rounded_rhino_20261017.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 62);
        assert_eq!(native["results"].as_array().unwrap().len(), 62);
        let mut failures = Vec::new();
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let result = std::panic::catch_unwind(|| {
                assert_eq!(a.id, n["id"]);
                let x = &a.value;
                let y = &n["value"];
                assert_eq!(y["command_returned"], true);
                assert_eq!(
                    y["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|e| e["name"] == "Rectangle")
                        .map(|e| e["result"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    ["Success"]
                );
                let history = y["history"].as_str().unwrap();
                for token in ["_Cancel", "Unknown command", "Invalid"] {
                    assert!(!history.contains(token), "{} {history}", a.id);
                }
                assert_eq!(x["undo_label"], "Rectangle");
                assert_eq!(x["redo_label"], "Rectangle");
                if matches!(a.id.as_str(), "rounded-arc-6" | "tiny-9") {
                    // Rhino succeeds without producing an object for this tiny
                    // radius. Retain that observation as an unqualified case.
                    assert!(y["after_script"]["outputs"].as_array().unwrap().is_empty());
                    assert_eq!(x["after"]["outputs"].as_array().unwrap().len(), 1);
                    assert_eq!(x["after"]["outputs"][0]["closed"], true);
                    assert!(close(&x["before"], &x["undo"]));
                    assert!(close(&x["after"], &x["redo"]));
                    return;
                }
                assert_eq!(y["undo_returned"], true);
                assert_eq!(y["redo_returned"], true);
                for stage in ["before", "after", "undo", "redo"] {
                    let mut aa = x[stage].clone();
                    let mut nn = y[if stage == "after" {
                        "after_script"
                    } else {
                        stage
                    }]
                    .clone();
                    // Polycurve leaves keep independent local domains. Compare
                    // those leaf shapes on [0,1], and the entire curve's actual
                    // knots and domain without changing either record.
                    normalize_domains(&mut aa);
                    normalize_domains(&mut nn);
                    assert!(close(&aa, &nn), "{} {stage}: {aa} != {nn}", a.id);
                }
            });
            if result.is_err() {
                failures.push(a.id.clone());
            }
        }
        assert!(
            failures.is_empty(),
            "rounded rectangle witnesses differ: {failures:?}"
        );
    }
}
