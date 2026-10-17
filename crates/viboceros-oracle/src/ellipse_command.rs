//! Ellipse point/numeric getters and complete document/history snapshots.
use super::*;
use viboceros_command::ellipse_input::Input;
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([f64; 3]),
    Number(f64),
    Option(String),
    Relative { relative: [f64; 3] },
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[f64; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
    #[serde(default)]
    pub prior_point: Option<[f64; 3]>,
}
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last_point: &mut Option<Point3>,
) -> Result<(), ProbeError> {
    let mut input = Input::new(commands.ellipse_options(), plane, doc.tolerance());
    for action in actions {
        match action {
            Action::Point(p) => {
                let point = Point3::try_from(*p)?;
                input.point(point)?;
                *last_point = Some(point);
            }
            Action::Number(value) => {
                input.number(*value)?;
            }
            Action::Option(word) => {
                if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unsupported Ellipse option"));
                }
            }
            Action::Relative {
                relative: [x, y, z],
            } => {
                let point = viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .unwrap()
                    .map_err(|_| ProbeError::FixtureInvariant("invalid relative ellipse point"))?
                    .resolve(plane, *last_point)
                    .map_err(|_| {
                        ProbeError::FixtureInvariant("relative ellipse point requires a base")
                    })?;
                input.point(point)?;
                *last_point = Some(point);
            }
        }
    }
    commands.execute(doc, &input.command()?)?;
    Ok(())
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let plane = if let Some([o, x, y]) = f.construction_plane {
            Frame3::try_from_directions(
                Point3::try_from(o)?,
                Vector3::try_from(x)?,
                Vector3::try_from(y)?,
                tolerance,
            )?
        } else {
            viboceros_command::CommandContext::default().construction_plane
        };
        let mut last = f.prior_point.map(Point3::try_from).transpose()?;
        if let Some(actions) = &f.setup {
            enter(&mut doc, &commands, plane, actions, &mut last)?;
            let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
            doc.delete_objects(ids)?;
        }
        doc.clear_history()?;
        let snapshot = |d: &Document| crate::curve_primitive::snapshot(d, &[]);
        let before = snapshot(&doc)?;
        enter(&mut doc, &commands, plane, &f.actions, &mut last)?;
        let after = snapshot(&doc)?;
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc)?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc)?;
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
    fn ellipse_native_construction_axes_focus_markers_and_actual_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_command_rhino_20261017.json"
                ),
                16,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_directions.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_directions_rhino_20261017.json"
                ),
                18,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_final.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_final_rhino_20261017.json"
                ),
                11,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_locks.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_locks_rhino_20261017.json"
                ),
                5,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_planes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_planes_rhino_20261017.json"
                ),
                5,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let local = run_request(&q).unwrap();
            assert_eq!(local.results.len(), count);
            for (a, b) in local.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    close(
                        &a.value[stage],
                        &b["value"][stage],
                        &format!("{}/{stage}", a.id),
                    );
                }
                assert_eq!(a.value["undo_label"], "Ellipse");
                assert_eq!(a.value["redo_label"], "Ellipse");
            }
        }
    }
}
