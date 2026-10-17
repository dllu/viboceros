//! Temporary Flow guides use authored endpoints and never become document objects.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_command::flow::{GuideInput, GuideReference};
use viboceros_document::{ControlPointId, GripMode};
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub base: ObjectSource,
    pub target: ObjectSource,
    pub source: ObjectSource,
    #[serde(default = "yes")]
    pub base_line: bool,
    #[serde(default)]
    pub target_line: bool,
    #[serde(default)]
    pub selected: Vec<usize>,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub options: super::flow_grips::Options,
}
impl Fixture {
    pub fn source_geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        self.source.geometry(tolerance)
    }
    pub fn guide_geometries(&self, tolerance: Tolerance) -> Result<[Geometry; 2], ProbeError> {
        Ok([
            self.base.geometry(tolerance)?,
            self.target.geometry(tolerance)?,
        ])
    }
    pub fn mode(&self) -> GripMode {
        if self.edit {
            GripMode::Edit
        } else {
            GripMode::Control
        }
    }
    pub fn observe(
        &self,
        doc: &Document,
        source: ObjectId,
        guides: [Option<ObjectId>; 2],
    ) -> Result<Value, ProbeError> {
        let ids = guides.into_iter().flatten().collect::<Vec<_>>();
        let mut value = super::orient_surface_grips::snapshot_excluding(doc, source, None, &ids)?;
        value["object_count"] = json!(doc.objects().count());
        value["guide_selected"] = json!(ids.iter().any(|id| doc.is_selected(*id)));
        let geometry = self.guide_geometries(doc.tolerance())?;
        value["guide_unchanged"] = json!(guides.into_iter().zip(geometry).all(|(id, g)| {
            id.is_none_or(|id| doc.object(id).is_some_and(|o| o.geometry() == &g))
        }));
        Ok(value)
    }
    pub fn input(geometry: &Geometry, id: Option<ObjectId>) -> Result<GuideInput, ProbeError> {
        if let Some(id) = id {
            return Ok(GuideReference { id, edge: None }.into());
        }
        let Geometry::Line(line) = geometry else {
            return Err(ProbeError::FixtureInvariant("Flow Line input needs a line"));
        };
        Ok(GuideInput::Line([line.start(), line.end()]))
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_with_options(f, iterations, tolerance, f.options.command())
}
pub(super) fn run_with_options(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    options: viboceros_command::flow::Options,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let geometry = f.guide_geometries(tolerance)?;
        let guides = [
            if f.base_line {
                None
            } else {
                Some(doc.add_geometry(geometry[0].clone())?)
            },
            if f.target_line {
                None
            } else {
                Some(doc.add_geometry(geometry[1].clone())?)
            },
        ];
        let inputs = [
            Fixture::input(&geometry[0], guides[0])?,
            Fixture::input(&geometry[1], guides[1])?,
        ];
        let original = f.source_geometry(tolerance)?;
        let source = doc.add_geometry(original.clone())?;
        if f.group {
            doc.add_group(Some("Owned Flow Line".into()), [source])?;
        }
        let grips = f
            .selected
            .iter()
            .map(|&index| ControlPointId {
                object: source,
                index,
            })
            .collect::<Vec<_>>();
        if grips.is_empty() {
            doc.select_objects_direct([source], SelectionMode::Replace)?;
        } else {
            match f.mode() {
                GripMode::Control => doc.enable_control_points([source])?,
                GripMode::Edit => doc.enable_edit_points([source])?,
            };
            doc.select_control_points(grips.iter().copied(), SelectionMode::Add)?;
        }
        doc.clear_history()?;
        let before = f.observe(&doc, source, guides)?;
        let picks = if grips.is_empty() {
            String::new()
        } else {
            format!(
                " Sources= GripSources={}",
                grips
                    .iter()
                    .map(|g| format!("{}:{}", g.object, g.index))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let registry = CommandRegistry::with_builtins();
        let error = registry
            .execute(
                &mut doc,
                &format!(
                    "Flow {} {} {}{picks}",
                    inputs[0].arguments("Base"),
                    inputs[1].arguments("Target"),
                    options.arguments()
                ),
            )
            .err()
            .map(|e| e.to_string());
        let after = f.observe(&doc, source, guides)?;
        let after_geometry = doc
            .objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>();
        let undo_label = doc.undo()?;
        let undo = f.observe(&doc, source, guides)?;
        let undo_restores_source = doc
            .object(source)
            .is_some_and(|o| o.geometry() == &original);
        let redo_label = doc.redo()?;
        let redo = f.observe(&doc, source, guides)?;
        let redo_restores_outputs = after_geometry
            .iter()
            .all(|(id, g)| doc.object(*id).is_some_and(|o| o.geometry() == g));
        Ok(
            json!({"before":before,"after":after,"undo":undo,"redo":redo,"command_returned":error.is_none(),"error":error,"undo_label":undo_label,"redo_label":redo_label,"undo_restores_source":undo_restores_source,"redo_restores_outputs":redo_restores_outputs}),
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
                a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| close(a, b)))
            }
            _ => a == b,
        }
    }
    #[test]
    fn flow_line_matches_native_geometry_guide_lifetime_and_history() {
        compare_native(
            include_str!("../../../tools/rhino_oracle/fixtures/flow_line.json"),
            include_str!("../../../tools/rhino_oracle/observations/flow_line_rhino_20261016.json"),
            128,
        );
        compare_native(
            include_str!("../../../tools/rhino_oracle/fixtures/flow_line_orientation.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/flow_line_orientation_rhino_20261016.json"
            ),
            24,
        );
    }
    #[test]
    fn flow_roadlike_matches_native_axes_owners_and_history() {
        compare_native(
            include_str!("../../../tools/rhino_oracle/fixtures/flow_roadlike.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/flow_roadlike_rhino_20261016.json"
            ),
            40,
        );
    }
    #[test]
    fn flow_roadlike_poles_match_supported_native_owners_and_history() {
        let request = include_str!("../../../tools/rhino_oracle/fixtures/flow_roadlike_poles.json");
        let native = include_str!(
            "../../../tools/rhino_oracle/observations/flow_roadlike_poles_rhino_20261016.json"
        );
        let mut q: Value = serde_json::from_str(request).unwrap();
        let mut n: Value = serde_json::from_str(native).unwrap();
        let keep = q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["target"]["type"] != "nurbs")
            .collect::<Vec<_>>();
        assert_eq!(keep.iter().filter(|b| !**b).count(), 4);
        let mut i = 0;
        q["operations"].as_array_mut().unwrap().retain(|_| {
            let b = keep[i];
            i += 1;
            b
        });
        i = 0;
        n["results"].as_array_mut().unwrap().retain(|_| {
            let b = keep[i];
            i += 1;
            b
        });
        compare_native(&q.to_string(), &n.to_string(), 36);
        let mut q: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/flow_roadlike_diagnostics.json"
        ))
        .unwrap();
        let mut n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/flow_roadlike_diagnostics_rhino_20261016.json"
        ))
        .unwrap();
        q["operations"].as_array_mut().unwrap().truncate(8);
        n["results"].as_array_mut().unwrap().truncate(8);
        compare_native(&q.to_string(), &n.to_string(), 8);
    }
    fn compare_native(request: &str, native: &str, cases: usize) {
        let request: ProbeRequest = serde_json::from_str(request).unwrap();
        let actual = run_request(&request).unwrap();
        let native: Value = serde_json::from_str(native).unwrap();
        assert_eq!(actual.results.len(), cases);
        assert_eq!(actual.results.len(), request.operations.len());
        assert_eq!(
            actual.results.len(),
            native["results"].as_array().unwrap().len()
        );
        for (a, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, n["id"]);
            if a.value.get("sdk_points").is_some() {
                assert!(
                    close(&a.value["sdk_points"], &n["value"]["sdk_points"]),
                    "{}",
                    a.id
                );
                continue;
            }
            for key in [
                "command_returned",
                "undo_restores_source",
                "redo_restores_outputs",
            ] {
                assert_eq!(a.value[key], true, "{} {key}: {}", a.id, a.value);
            }
            assert_eq!(a.value["undo_label"], "Flow");
            assert_eq!(a.value["redo_label"], "Flow");
            for stage in ["before", "after", "undo", "redo"] {
                let x = &a.value[stage];
                let y = &n["value"][if stage == "after" {
                    "after_script"
                } else {
                    stage
                }];
                for key in [
                    "object_count",
                    "group_count",
                    "guide_unchanged",
                    "guide_selected",
                ] {
                    assert_eq!(x[key], y[key], "{} {stage} {key}", a.id);
                }
                let xx = x["outputs"].as_array().unwrap();
                let yy = y["outputs"].as_array().unwrap();
                assert_eq!(xx.len(), yy.len(), "{} {stage}", a.id);
                let mut used = BTreeSet::new();
                for output in xx {
                    let (i, _) = yy
                        .iter()
                        .enumerate()
                        .find(|(i, n)| !used.contains(i) && close(output, n))
                        .unwrap_or_else(|| {
                            panic!("{} {stage} unmatched {output}; native {y}", a.id)
                        });
                    used.insert(i);
                }
            }
        }
    }
}
