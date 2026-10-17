//! Public Helix/Spiral point workflows and exact model/history witnesses.
use super::*;
use crate::{curve_primitive::snapshot as curve_snapshot, object_source::ObjectSource};
use viboceros_command::spiral_input::{Input, Kind, Numeric};
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([f64; 3]),
    Number(f64),
    Option(String),
    Relative {
        relative: [f64; 3],
    },
    Pick {
        curve: usize,
        #[serde(default)]
        edge: Option<usize>,
        #[serde(default)]
        pick: Option<[f64; 3]>,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[f64; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
    #[serde(default)]
    pub curves: Vec<ObjectSource>,
    #[serde(default)]
    pub preselect: bool,
    #[serde(default)]
    pub guide_selection: bool,
    #[serde(default)]
    pub prior_point: Option<[f64; 3]>,
}
impl Fixture {
    pub fn source_geometries(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
        self.curves
            .iter()
            .map(|source| source.geometry(tolerance))
            .collect()
    }
}
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    kind: Kind,
    plane: viboceros_geometry::Frame3,
    actions: &[Action],
    guides: &[(ObjectId, Geometry)],
    last_point: &mut Option<Point3>,
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
            Action::Pick { curve, edge, pick } => {
                let (id, geometry) = guides
                    .get(*curve)
                    .ok_or(ProbeError::FixtureInvariant("missing spiral rail"))?;
                if let (Some(edge), Some(point)) = (edge, pick) {
                    let resolved = viboceros_command::curve_reference::resolve(
                        geometry,
                        Some(*edge),
                        doc.tolerance(),
                    )
                    .ok_or(ProbeError::FixtureInvariant("missing spiral rail edge"))?;
                    let p = Point3::try_from(*point)?;
                    let c = resolved.curve();
                    let t = c.closest_parameter(p, doc.tolerance())?;
                    if c.evaluate(t)?.distance_to(p)? > doc.tolerance().absolute() {
                        return Err(ProbeError::FixtureInvariant(
                            "spiral edge pick must lie on the named edge",
                        ));
                    }
                }
                input.capture_rail(doc, *id, *edge)?;
                if edge.is_none() {
                    doc.select_object(*id, viboceros_document::SelectionMode::Add)?;
                }
            }
            Action::Relative {
                relative: [x, y, z],
            } => {
                let token = format!("r{x},{y},{z}");
                let point_input = viboceros_drafting::PointInput::parse(&token)
                    .ok_or(ProbeError::FixtureInvariant("invalid relative point"))?
                    .map_err(|_| ProbeError::FixtureInvariant("invalid relative point"))?;
                let p = point_input
                    .resolve(plane, input.relative_base().or(*last_point))
                    .map_err(|_| ProbeError::FixtureInvariant("relative point requires a base"))?;
                complete = input.point(p)?;
                *last_point = Some(p);
            }
            Action::Point(p) => {
                if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant("spiral option needs a value"));
                }
                let p = Point3::try_from(*p)?;
                complete = input.point(p)?;
                *last_point = Some(p);
            }
            Action::Number(n) => {
                if let Some(key) = pending.take() {
                    match key {
                        "Turns" => input.numeric(Numeric::Turns, *n)?,
                        "Pitch" => input.numeric(Numeric::Pitch, *n)?,
                        "NumPointsPerTurn" => input.numeric(Numeric::PointsPerTurn, *n)?,
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
                } else if ["Turns", "Pitch", "NumPointsPerTurn"].contains(&word.as_str()) {
                    pending = Some(match word.as_str() {
                        "NumPointsPerTurn" => "NumPointsPerTurn",
                        "Turns" => "Turns",
                        _ => "Pitch",
                    });
                } else if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unavailable spiral option"));
                }
            }
        }
        if input.slot() == viboceros_command::spiral_input::Slot::Rail {
            doc.clear_selection();
        }
    }
    if !complete {
        return Err(ProbeError::FixtureInvariant("incomplete spiral input"));
    }
    commands.execute(doc, &input.command()?)?;
    commands.remember_spiral_options(kind, input.options());
    Ok(())
}
fn snapshot(
    doc: &Document,
    guides: &[(ObjectId, Geometry)],
    selection: bool,
) -> Result<Value, ProbeError> {
    let mut value = curve_snapshot(doc, guides)?;
    if selection {
        value["guide_selected"] = json!(
            guides
                .iter()
                .map(|(id, _)| doc.is_selected(*id))
                .collect::<Vec<_>>()
        );
    }
    Ok(value)
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
        let guides = f
            .source_geometries(tolerance)?
            .into_iter()
            .map(|g| {
                let id = doc.add_geometry(g.clone())?;
                Ok((id, g))
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        if f.preselect
            && let Some((id, _)) = guides.first()
        {
            doc.select_object(*id, viboceros_document::SelectionMode::Replace)?;
        }
        let mut last_point = f.prior_point.map(Point3::try_from).transpose()?;
        if let Some(setup) = &f.setup {
            enter(
                &mut doc,
                &commands,
                kind,
                plane,
                setup,
                &guides,
                &mut last_point,
            )?;
            doc.undo()?;
        }
        doc.clear_history()?;
        let before = snapshot(&doc, &guides, f.guide_selection)?;
        enter(
            &mut doc,
            &commands,
            kind,
            plane,
            &f.actions,
            &guides,
            &mut last_point,
        )?;
        let after = snapshot(&doc, &guides, f.guide_selection)?;
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc, &guides, f.guide_selection)?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc, &guides, f.guide_selection)?;
        Ok(
            json!({"command_returned":true,"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: &Value, b: &Value, path: &str) {
        close_at(a, b, path, 1e-6);
    }
    fn close_at(a: &Value, b: &Value, path: &str, epsilon: f64) {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= epsilon,
                "{path}: {a} != {b}"
            ),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    close_at(a, b, &format!("{path}/{i}"), epsilon);
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (k, a) in a {
                    close_at(a, b.get(k).unwrap(), &format!("{path}/{k}"), epsilon);
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
    fn around_curve_native_geometry_source_selection_and_actual_history() {
        for (request, native, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/spiral_around_curve.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/spiral_around_curve_rhino_20261017.json"
                ),
                23,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/spiral_around_topology.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/spiral_around_topology_rhino_20261017.json"
                ),
                3,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/spiral_relative_points.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/spiral_relative_points_rhino_20261017.json"
                ),
                6,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(request).unwrap();
            let n: Value = serde_json::from_str(native).unwrap();
            let a = run_request(&q).unwrap();
            assert_eq!(a.results.len(), count);
            for (a, n) in a.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, n["id"]);
                // Keep the closed spatial rail as an explicit accuracy diagnostic.
                // Its native swept controls differ by about 1.97e-6; it is not
                // included in the 1e-6 qualifications for the other workflows.
                let diagnostic = a.id == "around-closed-spatial";
                if diagnostic {
                    let actual = a.value["after"]["outputs"][0]["curve"]["control_points"]
                        .as_array()
                        .unwrap();
                    let expected = n["value"]["after"]["outputs"][0]["curve"]["control_points"]
                        .as_array()
                        .unwrap();
                    let maximum = actual
                        .iter()
                        .zip(expected)
                        .map(|(p, q)| {
                            (0..3)
                                .map(|i| {
                                    (p["point"][i].as_f64().unwrap()
                                        - q["point"][i].as_f64().unwrap())
                                    .powi(2)
                                })
                                .sum::<f64>()
                                .sqrt()
                        })
                        .fold(0_f64, f64::max);
                    assert!(maximum > 1e-6 && maximum <= 3e-6, "{maximum}");
                }
                for stage in ["before", "after", "undo", "redo"] {
                    close_at(
                        &a.value[stage],
                        &n["value"][stage],
                        &format!("{}/{stage}", a.id),
                        if diagnostic { 3e-6 } else { 1e-6 },
                    );
                }
            }
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
