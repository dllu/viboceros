//! Complete public Weight value/getter, geometry, selection and history states.
use super::*;
use viboceros_geometry::CurveSegment3;

#[derive(Clone, Debug, Deserialize, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Input {
    Number(f64),
    Action(Action),
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, serde::Serialize)]
pub enum Action {
    Enter,
    Cancel,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    #[serde(flatten)]
    base: delete_grips::Fixture,
    pub inputs: Vec<Input>,
    pub dash: bool,
    pub pick: bool,
    repeat: bool,
}
impl Fixture {
    pub fn rebase(&mut self, folder: &Path) {
        self.base.rebase(folder);
    }
    fn validate(&self) -> Result<(), ProbeError> {
        if !(1..=4).contains(&self.inputs.len())
            || self
                .inputs
                .iter()
                .any(|v| matches!(v,Input::Number(n)if !n.is_finite()||n.abs()>1e6))
            || !self.dash
                && (self.inputs.len() > 2
                    || self.inputs.len() == 2
                        && !matches!(
                            self.inputs.as_slice(),
                            [Input::Number(_), Input::Action(Action::Cancel)]
                        ))
        {
            return Err(ProbeError::FixtureInvariant("invalid Weight input recipe"));
        }
        Ok(())
    }
}
pub fn prepare(f: &Fixture) -> Result<delete_grips::Prepared, ProbeError> {
    f.validate()?;
    delete_grips::prepare(&f.base)
}
pub fn snapshot(state: &delete_grips::Prepared) -> Result<Value, ProbeError> {
    snapshot_document(&state.document, state)
}
pub fn snapshot_document(
    doc: &Document,
    state: &delete_grips::Prepared,
) -> Result<Value, ProbeError> {
    let mut value = delete_grips::snapshot(doc, &state.ids, state.peer, state.marker, state.group)?;
    for row in value["objects"].as_array_mut().unwrap() {
        let obj = doc
            .objects()
            .find(|o| o.attributes().name() == row["name"].as_str())
            .unwrap();
        let weights = doc.control_point_weights(obj.id());
        for g in row["grips"].as_array_mut().unwrap() {
            g["weight"] = json!(
                weights
                    .as_ref()
                    .and_then(|w| w.get(g["index"].as_u64().unwrap() as usize))
                    .copied()
                    .unwrap_or(-1.234_321_012_343_21e308)
            );
        }
        if let Geometry::PolyCurve(c) = obj.geometry() {
            let segments = c
                .segments()
                .iter()
                .map(|s| {
                    let kind = match s {
                        CurveSegment3::Line(_) => "LineCurve",
                        CurveSegment3::Arc(_) => "ArcCurve",
                        CurveSegment3::Polyline(_) => "PolylineCurve",
                        CurveSegment3::NurbsCurve(_) => "NurbsCurve",
                    };
                    let mut curve = nurbs_curve_definition_value(&s.to_nurbs()?);
                    let knots = curve["knots"].as_array_mut().unwrap();
                    let last = knots.len() - 1;
                    knots[0] = knots[1].clone();
                    knots[last] = knots[last - 1].clone();
                    Ok(json!({"kind":kind,"curve":curve}))
                })
                .collect::<Result<Vec<_>, ProbeError>>()?;
            row["segments"] = json!(segments);
        }
    }
    Ok(value)
}

struct WeightResult {
    result: &'static str,
    getter: Value,
    cleanup: Vec<ObjectId>,
}
fn weight_step(
    f: &Fixture,
    doc: &mut Document,
    registry: &CommandRegistry,
) -> Result<WeightResult, ProbeError> {
    let mut selection = false;
    if doc.selected_control_points().next().is_none() {
        selection = true;
        doc.clear_selection();
        if !f.pick {
            return Ok(WeightResult {
                result: "Cancel",
                getter: json!({"opened":true,"selection":true,"default":null,"inputs":[]}),
                cleanup: Vec::new(),
            });
        }
        doc.select_all_control_points();
    }
    let controls = doc
        .selected_control_points()
        .map(|(p, _)| p)
        .collect::<Vec<_>>();
    let all_owners = controls.iter().map(|p| p.object).collect::<BTreeSet<_>>();
    let before = doc
        .objects()
        .filter(|o| all_owners.contains(&o.id()))
        .map(|o| (o.id(), o.geometry_snapshot().clone()))
        .collect::<Vec<_>>();
    let cleanup_all = || {
        if selection {
            all_owners.iter().copied().collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    };
    if doc.selected_weight_control_points().next().is_none() {
        return Ok(WeightResult {
            result: "Failure",
            getter: json!({"opened":selection,"selection":selection,"default":null,"inputs":[]}),
            cleanup: cleanup_all(),
        });
    }
    let prepared = viboceros_command::weight::Prepared::capture(doc)?;
    let shown = (!prepared.weights_vary()).then(|| {
        // The native numeric getter displays six significant decimal digits;
        // Enter still uses the original full-precision controller weight.
        let w = prepared.default_weight();
        format!("{w:.5e}").parse::<f64>().unwrap()
    });
    let consumed = if f.dash {
        &f.inputs[..1]
    } else {
        f.inputs.as_slice()
    };
    let getter = json!({"opened":true,"selection":selection,"default":shown,"inputs":consumed});
    if consumed.last() == Some(&Input::Action(Action::Cancel)) {
        return Ok(WeightResult {
            result: "Cancel",
            getter,
            cleanup: cleanup_all(),
        });
    }
    let weight = match consumed.first().unwrap() {
        Input::Number(v) => *v,
        Input::Action(Action::Enter) => prepared.default_weight(),
        Input::Action(Action::Cancel) => unreachable!(),
    };
    if weight <= 0. {
        return Ok(WeightResult {
            result: "Failure",
            getter,
            cleanup: cleanup_all(),
        });
    }
    let mut groups = BTreeMap::<ObjectId, Vec<Option<f64>>>::new();
    for p in controls {
        groups
            .entry(p.object)
            .or_default()
            .push(doc.control_point_weight(p));
    }
    let failed = groups
        .values()
        .any(|w| w.iter().any(Option::is_none) || w.iter().all(|w| *w == Some(weight)));
    registry.execute(doc, &format!("Weight {weight}"))?;
    let cleanup = if selection {
        before
            .iter()
            .filter(|(id, g)| {
                doc.object(*id)
                    .is_some_and(|o| o.geometry_snapshot().shares_storage_with(g))
            })
            .map(|(id, _)| *id)
            .collect()
    } else {
        Vec::new()
    };
    Ok(WeightResult {
        result: if failed { "Failure" } else { "Success" },
        getter,
        cleanup,
    })
}
pub(super) fn run(f: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    f.validate()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut state = prepare(f)?;
        let registry = CommandRegistry::with_builtins();
        let before = snapshot(&state)?;
        let mut steps = Vec::new();
        for command in std::iter::once("Weight")
            .chain(f.repeat.then_some("Weight"))
            .chain(f.base.points_off().then_some("PointsOff"))
            .chain(["Undo", "Redo"])
        {
            let initial = snapshot(&state)?;
            let outcome = if command == "Weight" {
                weight_step(f, &mut state.document, &registry)?
            } else {
                registry.execute(&mut state.document, command)?;
                WeightResult {
                    result: "Success",
                    getter: Value::Null,
                    cleanup: Vec::new(),
                }
            };
            let after = snapshot(&state)?;
            if !outcome.cleanup.is_empty() {
                let kept = state
                    .document
                    .selected_control_points()
                    .filter(|(p, _)| !outcome.cleanup.contains(&p.object))
                    .map(|(p, _)| p)
                    .collect::<Vec<_>>();
                state.document.clear_control_point_selection();
                state
                    .document
                    .select_control_points(kept, SelectionMode::Add)?;
            }
            let scripted = snapshot(&state)?;
            steps.push(json!({"command":command,"before":initial,"after":after,"after_script":scripted,"result":outcome.result,"getter":outcome.getter}));
        }
        Ok(json!({"before":before,"steps":steps}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn native_curve(value: &Value) -> NurbsCurve {
        nurbs_curve_from_definition(&serde_json::from_value(value.clone()).unwrap()).unwrap()
    }
    fn native_segment(value: &Value) -> CurveSegment3 {
        let c = native_curve(&value["curve"]);
        if value["kind"] == "LineCurve" {
            CurveSegment3::Line(
                LineSegment::try_new(
                    c.control_points()[0].point(),
                    c.control_points()[1].point(),
                    Tolerance::default(),
                )
                .unwrap()
                .try_reparameterized(c.domain())
                .unwrap(),
            )
        } else {
            CurveSegment3::NurbsCurve(c)
        }
    }
    fn short_outer_slots(c: &NurbsCurve) -> Value {
        let mut v = nurbs_curve_definition_value(c);
        let k = v["knots"].as_array_mut().unwrap();
        let n = k.len();
        k[0] = k[1].clone();
        k[n - 1] = k[n - 2].clone();
        v
    }
    fn lifecycle_snapshot(doc: &Document, id: ObjectId) -> Value {
        let g = doc.object(id).unwrap().geometry();
        let kind = match g {
            Geometry::Line(_) => "LineCurve",
            Geometry::Polyline(_) => "PolylineCurve",
            Geometry::NurbsCurve(_) => "NurbsCurve",
            Geometry::PolyCurve(_) => "PolyCurve",
            _ => unreachable!(),
        };
        let mut value = json!({"kind":kind,"curve":short_outer_slots(&g.curve_ref().unwrap().to_nurbs().unwrap()),"selected":doc.is_selected(id),
            "grips":doc.all_control_points().filter(|(p,_,_)|p.object==id).map(|(p,point,selected)|json!({"index":p.index,"point":point.to_array(),"weight":doc.control_point_weight(p).unwrap(),"selected":selected})).collect::<Vec<_>>()});
        if let Geometry::PolyCurve(c) = g {
            value["segments"]=json!(c.segments().iter().map(|s|json!({"kind":match s {CurveSegment3::Line(_)=>"LineCurve",CurveSegment3::NurbsCurve(_)=>"NurbsCurve",_=>unreachable!()},"curve":short_outer_slots(&s.to_nurbs().unwrap())})).collect::<Vec<_>>());
        }
        value
    }
    #[test]
    fn native_weight_grip_lifecycle_reopens_normalized_and_composite_controllers() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let r: Value = serde_json::from_str(
            &fs::read_to_string(root.join("observations/grip_weight_lifecycle.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(r["results"].as_array().unwrap().len(), 14);
        for row in r["results"].as_array().unwrap() {
            let before = &row["value"]["before"];
            let geometry = if before["kind"] == "PolyCurve" {
                let pieces = before["segments"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(native_segment)
                    .collect::<Vec<_>>();
                Geometry::PolyCurve(viboceros_geometry::PolyCurve3::try_new(pieces).unwrap())
            } else if before["kind"] == "LineCurve" {
                let CurveSegment3::Line(c) = native_segment(before) else {
                    unreachable!()
                };
                Geometry::Line(c)
            } else {
                Geometry::NurbsCurve(native_curve(&before["curve"]))
            };
            let mut doc = Document::default();
            let id = doc.add_geometry(geometry).unwrap();
            doc.enable_control_points([id]).unwrap();
            doc.clear_history().unwrap();
            let select = |doc: &mut Document, v: &Value| {
                doc.select_control_points(
                    v["grips"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|g| g["selected"] == true)
                        .map(|g| viboceros_document::ControlPointId {
                            object: id,
                            index: g["index"].as_u64().unwrap() as usize,
                        }),
                    SelectionMode::Add,
                )
                .unwrap();
            };
            select(&mut doc, before);
            let registry = CommandRegistry::with_builtins();
            crate::test_json::close(
                &lifecycle_snapshot(&doc, id),
                before,
                row["id"].as_str().unwrap(),
                1e-8,
                1e-12,
            );
            for step in row["value"]["steps"].as_array().unwrap() {
                if step["command"] == "PointsOn" {
                    doc.select_objects_direct([id], SelectionMode::Add).unwrap();
                }
                if step["command"] == "Weight" && doc.selected_control_points().next().is_none() {
                    select(&mut doc, &step["before"]);
                }
                crate::test_json::close(
                    &lifecycle_snapshot(&doc, id),
                    &step["before"],
                    "lifecycle before",
                    1e-8,
                    1e-12,
                );
                let command = if step["command"] == "Weight" {
                    "Weight 2"
                } else {
                    step["command"].as_str().unwrap()
                };
                registry.execute(&mut doc, command).unwrap();
                crate::test_json::close(
                    &lifecycle_snapshot(&doc, id),
                    &step["after_script"],
                    &format!("{}/{}", row["id"], command),
                    1e-8,
                    1e-12,
                );
            }
        }
    }
    #[test]
    fn complete_native_weight_command_and_history() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let mut failed = Vec::new();
        let mut count = 0;
        for (name, size) in [
            ("grip_weight", 237),
            ("grip_weight_selection", 5),
            ("grip_weight_representations", 20),
        ] {
            let q: Value = serde_json::from_str(
                &fs::read_to_string(root.join(format!("fixtures/{name}.json"))).unwrap(),
            )
            .unwrap();
            let expected: ProbeResponse = serde_json::from_str(
                &fs::read_to_string(root.join(format!("observations/{name}.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(expected.results.len(), size);
            for (op, e) in q["operations"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected.results)
            {
                let mut fixture: Fixture = serde_json::from_value(op.clone()).unwrap();
                fixture.rebase(&root.join(format!("fixtures/{name}")));
                let result = std::panic::catch_unwind(|| {
                    let actual = run(&fixture, 1).unwrap().0;
                    crate::test_json::close(&actual, &e.value, &e.id, 1e-8, 1e-12);
                });
                if result.is_err() {
                    failed.push(e.id);
                }
                count += 1;
            }
        }
        assert_eq!(count, 262);
        assert!(failed.is_empty(), "Weight differences: {failed:?}");
    }
}
