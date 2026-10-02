//! Native Untrim replay from independently exported B-rep inputs.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UntrimEdgeFixture {
    sources: Vec<Source>,
    components: Vec<(usize, usize)>,
    all_similar: bool,
    keep_trim_objects: bool,
    pick: Pick,
    finish: Finish,
    undo_redo: bool,
    #[serde(default)]
    undo_after: Vec<usize>,
    view: Option<View>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    brep: crate::brep_source::BrepSourceFixture,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Pick {
    Preselect,
    Mouse,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
enum Finish {
    Enter,
    Cancel,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum View {
    Top,
    Oblique,
}

pub(super) fn run(f: &UntrimEdgeFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if !(1..=8).contains(&f.sources.len())
        || f.components.len() > 64
        || f.components
            .iter()
            .any(|(source, _)| *source >= f.sources.len())
        || f.view.is_some() && f.pick != Pick::Mouse
        || f.undo_after
            .iter()
            .any(|&i| i == 0 || i > f.components.len())
        || !f.undo_after.windows(2).all(|pair| pair[0] < pair[1])
        || !f.undo_after.is_empty() && f.pick != Pick::Mouse
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid Untrim component sequence",
        ));
    }
    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let options = viboceros_command::UntrimOptions {
        all_similar: f.all_similar,
        keep_trim_objects: f.keep_trim_objects,
    };
    registry.accept_object_selection_input(&options.command_line())?;
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut constructed = Vec::new();
    let mut original = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        let brep = source.brep.build(tolerance)?;
        let geometry = Geometry::Brep(brep.clone());
        if let Some(path) = &source.brep.artifact_path {
            crate::brep_source::write_shared_artifact(&geometry, path, tolerance)?;
        }
        constructed.push(untrim::geometry_record(&geometry, tolerance)?);
        original.push(brep);
        let id = document.add_geometry_with_attributes(
            geometry,
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_name(format!("source-{i}"))
                .with_object_color(ColorRgb::new(10 + i as u8, 30, 50)),
        )?;
        ids.push(id);
        groups.push(document.add_group(Some(format!("source-{i}")), [id])?);
    }
    for &(source, edge) in &f.components {
        if edge >= original[source].edges().len() {
            return Err(ProbeError::FixtureInvariant("Untrim edge outside source"));
        }
    }
    let before = untrim::snapshot(&document, &ids, &groups)?;
    document.clear_history()?;
    let mut group = document.begin_history_group("Untrim")?;
    let mut states = Vec::new();
    let mut undo_states = Vec::new();
    if f.pick == Pick::Mouse {
        for (pick, &(source, edge)) in f.components.iter().enumerate() {
            let Geometry::Brep(current) = document.object(ids[source]).unwrap().geometry() else {
                unreachable!()
            };
            let candidates = if current == &original[source] {
                vec![edge]
            } else {
                current
                    .edges()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| {
                        (record.curve() == original[source].edges()[edge].curve()).then_some(index)
                    })
                    .collect::<Vec<_>>()
            };
            let [edge] = candidates.as_slice() else {
                return Err(ProbeError::FixtureInvariant(
                    "Untrim pick has no unique surviving source curve",
                ));
            };
            viboceros_command::UntrimSelection::prepare(&document, ids[source], *edge, options)?
                .commit_in_group(&mut document, &mut group)?;
            states.push(untrim::snapshot(&document, &ids, &groups)?);
            if f.undo_after.contains(&(pick + 1)) {
                if !document.undo_history_group(&mut group)? {
                    return Err(ProbeError::FixtureInvariant(
                        "Untrim Undo has no accepted edit",
                    ));
                }
                undo_states.push(untrim::snapshot(&document, &ids, &groups)?);
            }
        }
    }
    let after = untrim::snapshot(&document, &ids, &groups)?;
    let selected = if f.pick == Pick::Preselect {
        f.components
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|(source, edge)| json!([source, "edge", edge]))
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    let mut result = json!({"constructed":constructed,"before":before,"after":after,"succeeded":f.finish==Finish::Enter,
        "component_selection":{"before":selected,"after":[]}});
    if f.pick == Pick::Mouse {
        result["input_states"] = json!(states);
    }
    if !f.undo_after.is_empty() {
        result["undo_states"] = json!(undo_states);
    }
    if f.undo_redo {
        result["history_tested"] = json!(before != after);
        if before != after {
            document.undo()?;
            result["undo"] = json!(untrim::snapshot(&document, &ids, &groups)?);
            document.redo()?;
            result["redo"] = json!(untrim::snapshot(&document, &ids, &groups)?);
            result["component_selection"]["undo"] = json!([]);
            result["component_selection"]["redo"] = json!([]);
        }
    }
    Ok((result, 0))
}
