//! Native ShowDir command/control recipes replayed through the shared registry.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: String,
    pub preselect: bool,
    pub steps: Vec<String>,
    #[serde(default)]
    pub companion: bool,
    #[serde(default)]
    pub picks: std::collections::BTreeMap<String, usize>,
}

impl Fixture {
    pub fn source(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        Ok(match self.source.as_str() {
            "line" => Geometry::Line(LineSegment::try_new(
                Point3::try_new(0., 0., 0.)?,
                Point3::try_new(4., 2., 1.)?,
                tolerance,
            )?),
            "point" => Geometry::Point(Point3::try_new(1., 2., 3.)?),
            "mesh" => Geometry::Mesh(TriangleMesh::try_new_faces(
                vec![
                    Point3::try_new(0., 0., 0.)?,
                    Point3::try_new(4., 0., 0.)?,
                    Point3::try_new(4., 3., 1.)?,
                    Point3::try_new(0., 3., 0.)?,
                ],
                vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
                tolerance,
            )?),
            "warped" | "holed" | "box" | "sphere" => Geometry::Brep(
                brep_direction::Fixture {
                    source: self.source.clone(),
                    edit: "flip".into(),
                    preselect: true,
                    faces: None,
                    sdk: false,
                    sequence: None,
                    cancel: false,
                    pair: false,
                }
                .source(tolerance)?,
            ),
            _ => return Err(ProbeError::FixtureInvariant("unknown ShowDir source")),
        })
    }
    pub fn command(step: &str) -> String {
        if matches!(
            step,
            "Flip" | "UReverse" | "VReverse" | "SwapUV" | "Add" | "Remove"
        ) || step.contains('=')
        {
            format!("ShowDir {step}")
        } else {
            step.into()
        }
    }
}

fn object_snapshot(
    doc: &Document,
    registry: &CommandRegistry,
    id: ObjectId,
) -> Result<Value, ProbeError> {
    let Some(object) = doc.object(id) else {
        return Ok(
            json!({"exists":false,"selected":false,"hidden":false,"analysis":false,"geometry":null}),
        );
    };
    let geometry = match object.geometry() {
        Geometry::Brep(b) => brep_interchange::geometry_record(b)?,
        Geometry::Mesh(m) => polygon_mesh_value(m),
        Geometry::Point(p) => json!(p.to_array()),
        g if g.curve_ref().is_some() => curve_interchange::curve_record(g.curve_ref().unwrap())?,
        _ => return Err(ProbeError::FixtureInvariant("unsupported ShowDir snapshot")),
    };
    Ok(
        json!({"exists":true,"selected":doc.is_selected(id),"hidden":!object.attributes().is_visible(),"analysis":registry.object_has_direction_analysis(id),"geometry":geometry}),
    )
}

pub fn snapshot(
    doc: &Document,
    registry: &CommandRegistry,
    ids: &[ObjectId],
    companion: bool,
) -> Result<Value, ProbeError> {
    if ids.len() != if companion { 2 } else { 1 } {
        return Err(ProbeError::FixtureInvariant(
            "ShowDir source identities missing",
        ));
    }
    let rows = ids
        .iter()
        .map(|id| object_snapshot(doc, registry, *id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if companion {
        json!(rows)
    } else {
        rows.into_iter().next().unwrap()
    })
}

pub fn settings(doc: &Document, registry: &CommandRegistry) -> Value {
    registry
        .direction_analysis_view(doc)
        .map(|v| json!({"direction":v.options.direction,"u":v.options.u,"v":v.options.v}))
        .unwrap_or(Value::Null)
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    const STEPS: &[&str] = &[
        "ShowDir",
        "ShowDirOff",
        "Hide",
        "Show",
        "Delete",
        "Undo",
        "Redo",
        "Flip",
        "UReverse",
        "VReverse",
        "SwapUV",
        "Direction=No",
        "Direction=Yes",
        "U=No",
        "U=Yes",
        "V=No",
        "V=Yes",
        "SelectPrimary",
        "SelectCompanion",
        "SelNone",
        "SelAll",
        "Add",
        "Remove",
    ];
    if iterations != 1
        || f.steps.is_empty()
        || f.steps.len() > 24
        || f.steps.iter().any(|s| !STEPS.contains(&s.as_str()))
        || f.picks.iter().any(|(k, v)| {
            k.parse::<usize>().ok().is_none_or(|i| i >= f.steps.len())
                || *v >= if f.companion { 2 } else { 1 }
        })
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid ShowDir iteration/recipe length",
        ));
    }
    let source = f.source(tolerance)?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let mut ids = vec![doc.add_geometry(source.clone())?];
        if f.companion {
            ids.push(doc.add_geometry(source.clone())?);
        }
        if f.preselect {
            doc.select_objects_direct([ids[0]], SelectionMode::Replace)?;
        }
        doc.clear_history()?;
        let mut states = vec![snapshot(&doc, &registry, &ids, f.companion)?];
        let mut options = vec![settings(&doc, &registry)];
        for (step_index, step) in f.steps.iter().enumerate() {
            if matches!(
                step.as_str(),
                "SelNone" | "SelectPrimary" | "SelectCompanion"
            ) {
                if step == "SelNone" {
                    doc.clear_selection();
                } else {
                    let index = usize::from(step == "SelectCompanion");
                    doc.select_objects_direct(
                        [*ids
                            .get(index)
                            .ok_or(ProbeError::FixtureInvariant("missing ShowDir companion"))?],
                        SelectionMode::Replace,
                    )?;
                }
            } else {
                let postselected = doc.selected_object_count() == 0;
                if (matches!(
                    step.as_str(),
                    "ShowDir" | "Flip" | "UReverse" | "VReverse" | "SwapUV" | "Add" | "Remove"
                ) && doc.selected_object_count() == 0)
                    || step == "Delete"
                {
                    let pick = *f.picks.get(&step_index.to_string()).unwrap_or(&0);
                    doc.select_objects_direct(
                        [*ids.get(pick).ok_or(ProbeError::FixtureInvariant(
                            "invalid ShowDir getter target",
                        ))?],
                        SelectionMode::Replace,
                    )?;
                }
                let command = Fixture::command(step);
                // Nothing-to-undo is a completed observation, not a geometry edit.
                let execute = match step.as_str() {
                    "Undo" => doc.can_undo(),
                    "Redo" => doc.can_redo(),
                    _ => true,
                };
                if execute {
                    if postselected {
                        registry.execute_postselected(
                            &mut doc,
                            &command,
                            viboceros_command::CommandContext::default(),
                        )?;
                    } else {
                        registry.execute(&mut doc, &command)?;
                    }
                }
            }
            states.push(snapshot(&doc, &registry, &ids, f.companion)?);
            options.push(settings(&doc, &registry));
        }
        Ok(json!({"states":states,"settings":options}))
    })
}
