//! Frozen diagnostic membership across deletion and document history.
use super::edge_analysis_mixed as mixed;
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub flat: bool,
    pub analyzed: Vec<usize>,
    pub removed: Vec<usize>,
    #[serde(alias = "deleted")]
    pub targets: Vec<usize>,
    #[serde(default)]
    pub runtime: bool,
    #[serde(default = "default_edit")]
    pub edit: String,
    #[serde(default)]
    pub panel_mode: Option<String>,
    #[serde(default)]
    pub initial_mode: Option<String>,
    #[serde(default)]
    pub mode_sequence: Vec<String>,
    #[serde(default)]
    pub skip_zoom: bool,
}
fn default_edit() -> String {
    "Delete".into()
}
impl Fixture {
    pub fn validate(&self) -> Result<(), ProbeError> {
        if !matches!(self.edit.as_str(), "Delete" | "Move" | "Flip" | "Select")
            || (self.edit == "Select"
                && !matches!(
                    self.panel_mode.as_deref(),
                    Some("Zoom" | "Naked" | "All" | "NonManifold")
                ))
            || (self.edit != "Select" && self.panel_mode.is_some())
            || self
                .initial_mode
                .as_deref()
                .is_some_and(|m| !matches!(m, "All" | "Naked" | "NonManifold"))
            || self.mode_sequence.len() > 8
            || self
                .mode_sequence
                .iter()
                .any(|m| !matches!(m.as_str(), "All" | "Naked" | "NonManifold"))
            || self.analyzed.is_empty()
            || self.targets.is_empty()
            || [&self.analyzed, &self.removed, &self.targets]
                .iter()
                .any(|ids| {
                    ids.iter().any(|&i| i >= 3)
                        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                })
        {
            return Err(ProbeError::FixtureInvariant(
                "owned diagnostic lifetime operands required",
            ));
        }
        Ok(())
    }
}

pub fn setup(f: &Fixture, tolerance: Tolerance) -> Result<(Document, Vec<ObjectId>), ProbeError> {
    f.validate()?;
    mixed::setup(
        &mixed::Fixture {
            order: vec!["brep".into(), "mesh".into(), "subd".into()],
            preselect: true,
            flat: f.flat,
            grouped: false,
            extra_point: false,
            command: "ShowEdges".into(),
            actions: vec![],
            membership: false,
            panel_only: false,
            sequential: false,
            mode_trace: false,
        },
        tolerance,
    )
}

pub fn snapshot(doc: &Document, ids: &[ObjectId], runtime: bool) -> Result<Value, ProbeError> {
    let sources = ids
        .iter()
        .zip(["brep", "mesh", "subd"])
        .map(|(&id, kind)| {
            let object = doc.object(id);
            Ok(
                json!({"kind":kind,"present":object.is_some(),"selected":doc.is_selected(id),
            "geometry":object.map(|o|mixed::geometry(o.geometry())).transpose()?}),
            )
        })
        .collect::<Result<Vec<Value>, ProbeError>>()?;
    let points = doc
        .objects()
        .filter(|o| !ids.contains(&o.id()))
        .filter_map(|o| match o.geometry() {
            Geometry::Point(p) => Some(p.to_array()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut state = json!({"sources":sources,"points":points,"objects":doc.objects().len()});
    if runtime {
        let mut order = ids
            .iter()
            .zip(["brep", "mesh", "subd"])
            .filter_map(|(&id, kind)| doc.object(id).map(|o| (o.runtime_serial_number(), kind)))
            .collect::<Vec<_>>();
        order.sort_unstable_by_key(|&(serial, _)| serial);
        state["runtime_order"] = json!(order.into_iter().map(|(_, kind)| kind).collect::<Vec<_>>());
    }
    Ok(state)
}

fn stage(
    commands: &CommandRegistry,
    doc: &Document,
    ids: &[ObjectId],
    action: &str,
    runtime: bool,
) -> Result<Value, ProbeError> {
    // Redraws resolve the session while a source is absent, before Undo restores it.
    commands.edge_analysis_view(doc)?;
    Ok(
        json!({"action":action,"state":snapshot(doc,ids,runtime)?,"selection_states":ids.iter().map(|id|doc.object_selection_state(*id) as u8).collect::<Vec<_>>()}),
    )
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "one diagnostic lifetime iteration required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, ids) = setup(f, tolerance)?;
        let commands = CommandRegistry::with_builtins();
        doc.select_objects_direct(f.analyzed.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
        let before = snapshot(&doc, &ids, f.runtime)?;
        commands.execute(&mut doc, "ShowEdges")?;
        commands.execute(
            &mut doc,
            &format!(
                "ShowEdges Show={}",
                f.initial_mode.as_deref().unwrap_or("Naked")
            ),
        )?;
        let mut stages = vec![stage(&commands, &doc, &ids, "Start", f.runtime)?];
        if !f.removed.is_empty() {
            doc.select_objects_direct(f.removed.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
            commands.execute(&mut doc, "ShowEdges Remove")?;
            stages.push(stage(&commands, &doc, &ids, "Remove", f.runtime)?);
        }
        doc.select_objects_direct(f.targets.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
        if f.edit == "Select" {
            stages.push(stage(&commands, &doc, &ids, "Select", f.runtime)?);
            let modes = if f.mode_sequence.is_empty() {
                vec![f.panel_mode.as_deref().unwrap()]
            } else {
                f.mode_sequence.iter().map(String::as_str).collect()
            };
            for (i, mode) in modes.into_iter().enumerate() {
                if i > 0 {
                    doc.select_objects_direct(
                        f.targets.iter().map(|&i| ids[i]),
                        SelectionMode::Replace,
                    )?;
                    stages.push(stage(
                        &commands,
                        &doc,
                        &ids,
                        &format!("Select{i}"),
                        f.runtime,
                    )?);
                }
                if mode != "Zoom" {
                    commands.execute(&mut doc, &format!("ShowEdges Show={mode}"))?;
                    let action = if f.mode_sequence.is_empty() {
                        "Mode".into()
                    } else {
                        format!("Mode{i}")
                    };
                    stages.push(stage(&commands, &doc, &ids, &action, f.runtime)?);
                }
            }
        } else {
            let input = if f.edit == "Move" {
                "Move 0,0,0 7,2,1"
            } else {
                f.edit.as_str()
            };
            let undo = format!("Undo{}", f.edit);
            let redo = format!("Redo{}", f.edit);
            for (action, input) in [
                (f.edit.as_str(), input),
                (undo.as_str(), "Undo"),
                (redo.as_str(), "Redo"),
                ("Restore", "Undo"),
            ] {
                commands.execute(&mut doc, input)?;
                stages.push(stage(&commands, &doc, &ids, action, f.runtime)?);
            }
        }
        if let Some(name) = if f.skip_zoom {
            None
        } else {
            commands.begin_edge_analysis_zoom(&mut doc)?
        } {
            let mut group = doc.begin_history_group(name)?;
            commands.execute_edge_zoom_actions(&mut doc, "All", &mut group)?;
            commands.execute_edge_zoom_actions(&mut doc, "Mark", &mut group)?;
            commands.finish_edge_zoom(&mut doc);
        }
        stages.push(stage(&commands, &doc, &ids, "Zoom", f.runtime)?);
        let after = snapshot(&doc, &ids, f.runtime)?;
        if !after["points"].as_array().unwrap().is_empty() {
            commands.execute(&mut doc, "Undo")?;
            stages.push(stage(&commands, &doc, &ids, "UndoMark", f.runtime)?);
            commands.execute(&mut doc, "Redo")?;
            stages.push(stage(&commands, &doc, &ids, "RedoMark", f.runtime)?);
        }
        Ok(json!({"before":before,"after":after,"stages":stages}))
    })
}
