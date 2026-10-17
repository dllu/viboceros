//! Flow point maps from authored guides; native frames are never inputs.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_geometry::{CurveFlowOptions, CurveFlowPointMorph, PointMorph, Real};
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default = "yes")]
    pub stretch: bool,
    #[serde(default)]
    pub reverse_base: bool,
    #[serde(default)]
    pub reverse_target: bool,
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub rigid: bool,
    #[serde(default = "yes")]
    pub rigid_groups: bool,
    #[serde(default)]
    pub preserve_structure: bool,
    #[serde(default)]
    pub roadlike: bool,
    #[serde(default)]
    pub stairlike: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            stretch: true,
            reverse_base: false,
            reverse_target: false,
            copy: false,
            rigid: false,
            rigid_groups: true,
            preserve_structure: false,
            roadlike: false,
            stairlike: false,
        }
    }
}
impl Options {
    pub fn geometry(&self) -> CurveFlowOptions {
        CurveFlowOptions {
            stretch: self.stretch,
            reverse_base: self.reverse_base,
            reverse_target: self.reverse_target,
            preserve_structure: self.preserve_structure,
            ..Default::default()
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub base: ObjectSource,
    pub target: ObjectSource,
    pub points: Vec<[Real; 3]>,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub diagnostic_integrals: bool,
}
impl Fixture {
    pub fn guides(&self, tolerance: Tolerance) -> Result<[Geometry; 2], ProbeError> {
        Ok([
            self.base.geometry(tolerance)?,
            self.target.geometry(tolerance)?,
        ])
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let a = f.base.geometry(tolerance)?;
        let b = f.target.geometry(tolerance)?;
        let a = a
            .curve_ref()
            .ok_or(ProbeError::FixtureInvariant("Flow base must be a curve"))?
            .to_owned();
        let b = b
            .curve_ref()
            .ok_or(ProbeError::FixtureInvariant("Flow target must be a curve"))?
            .to_owned();
        let integration_traces = if f.diagnostic_integrals {
            let mut roles = serde_json::Map::new();
            let mut inverses = serde_json::Map::new();
            for (role, curve) in [("base", &a), ("target", &b)] {
                let sampler =
                    viboceros_geometry::ArcLengthSampler::try_new(curve.as_ref(), tolerance)?;
                let mut traces = Vec::new();
                for fractional in [1e-2, 1e-4, 1e-6, 1e-8, 1e-12] {
                    let trace = sampler.integration_checkpoints(fractional)?;
                    traces.push(json!({
                        "fractional_tolerance":fractional,
                        "length":trace.last().unwrap().distance(),
                        "checkpoints":trace.iter().map(|n| json!({"t":n.parameter(),"length":n.distance(),"estimated_step_error":n.estimated_step_error()})).collect::<Vec<_>>()
                    }));
                }
                roles.insert(role.into(), json!(traces));
                let mut batches = Vec::new();
                for divisions in [2, 16, 100, 840] {
                    let fractions = (0..=divisions)
                        .map(|i| i as f64 / divisions as f64)
                        .collect::<Vec<_>>();
                    let parameters =
                        sampler.parameters_at_incremental_normalized_lengths(&fractions, 1e-6)?;
                    batches.push(json!({"divisions":divisions,"parameters":parameters}));
                }
                inverses.insert(role.into(), json!(batches));
            }
            Some((roles, inverses))
        } else {
            None
        };
        let morph = CurveFlowPointMorph::try_new(a, b, f.options.geometry(), tolerance)?;
        let points = f
            .points
            .iter()
            .map(|p| Ok(morph.morph_point(Point3::try_from(*p)?)?.to_array()))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let command = if f.command {
            let mut doc = Document::new(tolerance);
            let ga = f.base.geometry(tolerance)?;
            let gb = f.target.geometry(tolerance)?;
            let base = doc.add_geometry(ga.clone())?;
            let target = doc.add_geometry(gb.clone())?;
            let cloud = PointCloud3::try_new(
                f.points
                    .iter()
                    .map(|p| Point3::try_from(*p))
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            let source = doc.add_geometry(Geometry::PointCloud(cloud))?;
            if f.group {
                doc.add_group(Some("Owned Flow source".into()), [source])?;
            }
            doc.select_objects_direct([source], SelectionMode::Replace)?;
            doc.clear_history()?;
            let options = viboceros_command::flow::Options {
                copy: f.options.copy,
                rigid: f.options.rigid,
                rigid_groups: f.options.rigid_groups,
                stretch: f.options.stretch,
                preserve_structure: f.options.preserve_structure,
                reverse_base: f.options.reverse_base,
                reverse_target: f.options.reverse_target,
                ..Default::default()
            };
            let registry = CommandRegistry::with_builtins();
            let error = registry
                .execute(
                    &mut doc,
                    &format!(
                        "Flow Base={base} Target={target} {}{}{}",
                        options.arguments(),
                        if f.options.roadlike {
                            " Roadlike=Yes"
                        } else {
                            ""
                        },
                        if f.options.stairlike {
                            " Stairlike=Yes"
                        } else {
                            ""
                        }
                    ),
                )
                .err()
                .map(|e| e.to_string());
            let outputs=doc.objects().filter_map(|o|{
                let Geometry::PointCloud(p)=o.geometry()else{return None};
                Some(json!({"original":o.id()==source,"selected":doc.is_selected(o.id()),"group_count":o.group_ids().len(),"points":p.points().iter().map(|p|p.to_array()).collect::<Vec<_>>()}))
            }).collect::<Vec<_>>();
            json!({"error":error,"returned":error.is_none(),"outputs":outputs,"guides_unchanged":doc.object(base).unwrap().geometry()==&ga&&doc.object(target).unwrap().geometry()==&gb,"guides_selected":[doc.is_selected(base),doc.is_selected(target)],"group_count":doc.groups().len(),"undo_label":doc.undo_label()})
        } else {
            Value::Null
        };
        let mut result = json!({"sdk_points":points,"base_length":morph.base_length(),"target_length":morph.target_length(),"command":command});
        if let Some((traces, inverses)) = integration_traces {
            result["integration_traces"] = Value::Object(traces);
            result["incremental_inverse_batches"] = Value::Object(inverses);
        }
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integration_traces_and_incremental_inverses_match_native_nurbs_batches() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/flow_integration.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/flow_integration_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 4);
        let mut compared = 0;
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            for role in ["base", "target"] {
                let native_fractional = if a.id == "polycurve-integration" && role == "target" {
                    1e-6
                } else {
                    1e-4
                };
                let traces = a.value["integration_traces"][role].as_array().unwrap();
                let integrals = n["value"]["diagnostics"][role]["integrals"]
                    .as_array()
                    .unwrap();
                assert_eq!(traces.len(), 5);
                assert_eq!(integrals.len(), 5);
                for (trace, integral) in traces.iter().zip(integrals) {
                    assert_eq!(
                        trace["fractional_tolerance"],
                        integral["fractional_tolerance"]
                    );
                    let expected = integral["length"].as_f64().unwrap();
                    let length = trace["length"].as_f64().unwrap();
                    assert!(
                        (length - expected).abs() <= 1e-11 * expected.max(1.),
                        "{} {role}: {length} != {expected}",
                        a.id
                    );
                    compared += 1;
                }
                let batches = a.value["incremental_inverse_batches"][role]
                    .as_array()
                    .unwrap();
                assert_eq!(batches.len(), 4);
                for batch in batches {
                    let native = n["value"]["diagnostics"][role]["length_batches"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|v| {
                            v["divisions"] == batch["divisions"]
                                && v["fractional_tolerance"] == native_fractional
                        })
                        .unwrap();
                    let aa = batch["parameters"].as_array().unwrap();
                    let nn = native["parameters"].as_array().unwrap();
                    assert_eq!(aa.len(), nn.len());
                    for (x, y) in aa.iter().zip(nn) {
                        assert!(
                            (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-8,
                            "{} {role}: {x} != {y}",
                            a.id
                        );
                    }
                }
            }
        }
        assert_eq!(compared, 40);
    }
    #[test]
    fn nurbs_source_target_and_composite_guides_match_seventy_two_native_maps() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/flow_nurbs.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/flow_nurbs_rhino_20261017.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 72);
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            near(&a.value["sdk_points"], &n["value"]["sdk_points"], &a.id);
        }
    }
    fn near(a: &Value, b: &Value, label: &str) {
        let a = a.as_array().unwrap();
        let b = b.as_array().unwrap();
        assert_eq!(a.len(), b.len(), "{label}");
        for (a, b) in a.iter().zip(b) {
            if a.is_array() {
                near(a, b, label);
            } else {
                assert!(
                    (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6,
                    "{label}: {a} != {b}"
                );
            }
        }
    }
    #[test]
    fn flow_authored_guides_match_seventy_six_native_maps_and_forty_three_commands() {
        let mut supported = 0;
        let mut spatial = 0;
        let mut commands = 0;
        for (fixture, observed) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/flow.json"),
                include_str!("../../../tools/rhino_oracle/observations/flow_rhino_20261016.json"),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/flow_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/flow_command_rhino_20261016.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/flow_planes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/flow_planes_rhino_20261016.json"
                ),
            ),
        ] {
            let input: Value = serde_json::from_str(fixture).unwrap();
            let native: Value = serde_json::from_str(observed).unwrap();
            let tolerance: Tolerance =
                serde_json::from_value::<ToleranceSpec>(input["tolerance"].clone())
                    .unwrap()
                    .geometry()
                    .unwrap();
            assert_eq!(
                input["operations"].as_array().unwrap().len(),
                native["results"].as_array().unwrap().len()
            );
            for (op, row) in input["operations"]
                .as_array()
                .unwrap()
                .iter()
                .zip(native["results"].as_array().unwrap())
            {
                assert_eq!(op["id"], row["id"]);
                let label = op["id"].as_str().unwrap();
                let f: Fixture = serde_json::from_value(op.clone()).unwrap();
                let a = f.base.geometry(tolerance).unwrap();
                let b = f.target.geometry(tolerance).unwrap();
                let morph = CurveFlowPointMorph::try_new(
                    a.curve_ref().unwrap().to_owned(),
                    b.curve_ref().unwrap().to_owned(),
                    f.options.geometry(),
                    tolerance,
                )
                .unwrap();
                let (actual, _) = run(&f, 1, tolerance).unwrap();
                if morph.uses_spatial_transport() {
                    spatial += 1;
                    if f.command {
                        assert_eq!(actual["command"]["returned"], false);
                        assert!(
                            actual["command"]["error"]
                                .as_str()
                                .unwrap()
                                .contains("spatial")
                        );
                    }
                    continue;
                }
                supported += 1;
                near(&actual["sdk_points"], &row["value"]["sdk_points"], label);
                if f.command {
                    commands += 1;
                    let a = &actual["command"];
                    let n = &row["value"]["command"];
                    for key in [
                        "returned",
                        "guides_unchanged",
                        "guides_selected",
                        "group_count",
                    ] {
                        assert_eq!(a[key], n[key], "{label}: {key}");
                    }
                    assert_eq!(a["undo_label"], "Flow");
                    let mut aa = a["outputs"].as_array().unwrap().iter().collect::<Vec<_>>();
                    aa.sort_by_key(|v| v["original"].as_bool());
                    let mut nn = n["outputs"].as_array().unwrap().iter().collect::<Vec<_>>();
                    nn.sort_by_key(|v| v["original"].as_bool());
                    assert_eq!(aa.len(), nn.len(), "{label}");
                    for (a, n) in aa.iter().zip(nn) {
                        for key in ["original", "selected", "group_count"] {
                            assert_eq!(a[key], n[key], "{label}: {key}");
                        }
                        near(&a["points"], &n["points"], label);
                    }
                }
            }
        }
        assert_eq!((supported, spatial, commands), (76, 8, 43));
    }
}
