//! Catenary getters, remembered parameters, and real model/history snapshots.
use super::*;
use viboceros_command::catenary_input::Input;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([f64; 3]),
    Number(f64),
    Option(String),
    Relative { relative: [f64; 3] },
    Click { click: [f64; 3] },
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
    #[serde(default)]
    pub curves: Vec<crate::object_source::ObjectSource>,
    #[serde(default)]
    pub persistent_snaps: Vec<String>,
}
impl Fixture {
    pub fn source_geometries(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
        self.curves.iter().map(|s| s.geometry(tolerance)).collect()
    }
    pub fn snap_modes(&self) -> Result<viboceros_drafting::ObjectSnapModes, ProbeError> {
        use viboceros_drafting::{ObjectSnapKind, ObjectSnapModes};
        let mut modes = ObjectSnapModes::NONE;
        for word in &self.persistent_snaps {
            let kind = match word.as_str() {
                "End" => ObjectSnapKind::End,
                "Point" => ObjectSnapKind::Point,
                "Near" => ObjectSnapKind::Near,
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "unsupported Catenary snap mode",
                    ));
                }
            };
            modes = modes.with(kind, true);
        }
        Ok(modes)
    }
    /// Authored world targets use the existing XY feature picker. This is
    /// a command-input replay; physical viewport aperture tests are separate.
    pub fn resolve_click(
        &self,
        doc: &Document,
        point: Point3,
    ) -> Result<viboceros_drafting::ObjectSnap, ProbeError> {
        viboceros_drafting::ObjectSnapCache::default()
            .nearest_axis_aligned_with_modes(
                doc,
                viboceros_geometry::PointCloudProjection::Xy,
                point,
                [0., 0.],
                1.,
                self.snap_modes()?,
            )
            .map_err(|_| ProbeError::FixtureInvariant("invalid Catenary snap query"))?
            .ok_or(ProbeError::FixtureInvariant(
                "Catenary click needs a resolved feature",
            ))
    }
}
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last_point: &mut Option<Point3>,
    fixture: &Fixture,
) -> Result<(), ProbeError> {
    let mut input = Input::new(commands.catenary_options(), plane, doc.tolerance());
    let mut pending = None;
    let mut complete = false;
    for action in actions {
        if complete {
            return Err(ProbeError::FixtureInvariant(
                "catenary actions after completion",
            ));
        }
        match action {
            Action::Click { click } => {
                if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant(
                        "Catenary option requires a value",
                    ));
                }
                let snap = fixture.resolve_click(doc, Point3::try_from(*click)?)?;
                complete = input.snapped_point(snap.point(), Some(snap.kind()))?;
                *last_point = Some(snap.point());
            }
            Action::Point(p) => {
                if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant(
                        "catenary option requires a value",
                    ));
                }
                let p = Point3::try_from(*p)?;
                complete = input.point(p)?;
                *last_point = Some(p);
            }
            Action::Relative {
                relative: [x, y, z],
            } => {
                if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant(
                        "catenary option requires a value",
                    ));
                }
                let p = viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .ok_or(ProbeError::FixtureInvariant(
                        "invalid relative catenary point",
                    ))?
                    .map_err(|_| ProbeError::FixtureInvariant("invalid relative catenary point"))?
                    .resolve(plane, input.relative_base().or(*last_point))
                    .map_err(|_| {
                        ProbeError::FixtureInvariant("relative catenary point requires a base")
                    })?;
                complete = input.point(p)?;
                *last_point = Some(p);
            }
            Action::Number(n) => {
                if pending == Some("PointCount") {
                    if !n.is_finite()
                        || n.fract() != 0.
                        || *n < 0.
                        || *n > viboceros_geometry::MAX_CATENARY_POINT_COUNT as f64
                    {
                        return Err(ProbeError::FixtureInvariant("invalid catenary point count"));
                    }
                    input.option(&format!("PointCount={}", *n as usize))?;
                    pending = None;
                } else if pending.is_some() {
                    return Err(ProbeError::FixtureInvariant(
                        "catenary mode requires a name",
                    ));
                } else {
                    complete = input.number(*n)?;
                }
            }
            Action::Option(word) => {
                if word == "Enter" {
                    if pending.take().is_none() {
                        complete = input.enter()?;
                    }
                } else if let Some(key) = pending.take() {
                    if !input.option(&format!("{key}={word}"))? {
                        return Err(ProbeError::FixtureInvariant("invalid catenary option"));
                    }
                } else if ["Mode", "PointCount"].contains(&word.as_str()) {
                    pending = Some(if word == "Mode" { "Mode" } else { "PointCount" });
                } else if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unknown catenary option"));
                }
            }
        }
    }
    if !complete {
        return Err(ProbeError::FixtureInvariant("incomplete catenary input"));
    }
    commands.execute(doc, &input.command()?)?;
    commands.remember_catenary_options(input.options());
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
        let plane = if let Some([p, x, y]) = f.construction_plane {
            Frame3::try_from_directions(
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
        let mut last_point = f.prior_point.map(Point3::try_from).transpose()?;
        if let Some(setup) = &f.setup {
            enter(&mut doc, &commands, plane, setup, &mut last_point, f)?;
            let ids = doc
                .objects()
                .filter(|o| !guides.iter().any(|(id, _)| *id == o.id()))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            doc.delete_objects(ids)?;
        }
        doc.clear_history()?;
        let snapshot = |doc: &Document| crate::curve_primitive::snapshot(doc, &guides);
        let before = snapshot(&doc)?;
        enter(&mut doc, &commands, plane, &f.actions, &mut last_point, f)?;
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
    fn compare_snap_geometry(request: &str, native: &str, count: usize) {
        let q: ProbeRequest = serde_json::from_str(request).unwrap();
        let n: Value = serde_json::from_str(native).unwrap();
        let local = run_request(&q).unwrap();
        assert_eq!(local.results.len(), count);
        assert_eq!(n["results"].as_array().unwrap().len(), count);
        for (a, b) in local.results.iter().zip(n["results"].as_array().unwrap()) {
            assert_eq!(a.id, b["id"]);
            assert_eq!(b["value"]["input_trace"]["errors"], json!([]));
            for stage in ["before", "after", "undo", "redo"] {
                close(
                    &a.value[stage],
                    &b["value"][stage],
                    &format!("{}/{stage}", a.id),
                );
            }
            assert_eq!(a.value["undo_label"], "Catenary");
            assert_eq!(a.value["redo_label"], "Catenary");
        }
    }
    #[test]
    fn catenary_physical_interior_snaps_match_native_geometry() {
        compare_snap_geometry(
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_snap_inputs.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_snap_inputs_rhino_20261017.json"
            ),
            3,
        );
    }
    #[test]
    fn catenary_documented_end_snap_matches_native_enter_geometry() {
        // Rhino 8.32 fails actual endpoint End clicks. These are explicitly
        // Enter references for the documented behavior, not same-input parity.
        compare_snap_geometry(
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_end_apex_snap.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_end_apex_enter_rhino_20261017.json"
            ),
            6,
        );
    }
    #[test]
    fn catenary_minimum_count_and_two_point_sdk_interpolation_match_native_controls() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/catenary_low_count.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_low_count_rhino_20261017.json"
                ),
                6,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/catenary_tangent_confirm.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_tangent_confirm_rhino_20261017.json"
                ),
                4,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/catenary_negative_tangents.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_negative_tangents_rhino_20261017.json"
                ),
                2,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let local = run_request(&q).unwrap();
            assert_eq!(local.results.len(), count);
            for (a, b) in local.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                close(&a.value, &b["value"], &a.id);
            }
        }
    }
    #[test]
    fn catenary_native_getters_full_geometry_selection_preferences_and_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/catenary_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_command_rhino_20261017.json"
                ),
                9,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/catenary_preferences.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_preferences_rhino_20261017.json"
                ),
                8,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/catenary_directions.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_directions_rhino_20261017.json"
                ),
                11,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/catenary_final.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/catenary_final_rhino_20261017.json"
                ),
                8,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let native: Value = serde_json::from_str(n).unwrap();
            let local = run_request(&q).unwrap();
            assert_eq!(local.results.len(), count);
            assert_eq!(native["results"].as_array().unwrap().len(), count);
            for (a, b) in local
                .results
                .iter()
                .zip(native["results"].as_array().unwrap())
            {
                assert_eq!(a.id, b["id"]);
                let n = &b["value"];
                assert_eq!(n["input_trace"]["errors"], json!([]));
                assert_eq!(n["after"], n["after_script"]);
                assert_eq!(n["before"], n["undo"]);
                assert_eq!(n["after"], n["redo"]);
                for stage in ["before", "after", "undo", "redo"] {
                    close(&a.value[stage], &n[stage], &format!("{}/{stage}", a.id));
                }
                assert_eq!(a.value["undo_label"], "Catenary");
                assert_eq!(a.value["redo_label"], "Catenary");
            }
        }
    }
}
