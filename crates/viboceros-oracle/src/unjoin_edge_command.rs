//! Command replay from independent sources and component input, never expected output.
use super::*;
use viboceros_command::UnjoinEdgeSelection;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UnjoinEdgeFixture {
    sources: Vec<Source>,
    components: Vec<(usize, usize)>,
    finish: Finish,
    undo_redo: bool,
    #[serde(default)]
    kind: Kind,
    #[serde(default)]
    object_preselect: bool,
    #[serde(default)]
    pick: Pick,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    brep: crate::brep_source::BrepSourceFixture,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
enum Finish {
    Enter,
    Cancel,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    #[default]
    Edge,
    Face,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Pick {
    #[default]
    Preselect,
    Mouse,
}
pub(super) fn run(f: &UnjoinEdgeFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if !(1..=8).contains(&f.sources.len())
        || f.components.len() > 100_000
        || f.components
            .iter()
            .any(|(source, _)| *source >= f.sources.len())
        || (f.pick == Pick::Mouse && f.kind != Kind::Edge)
    {
        return Err(ProbeError::FixtureInvariant("invalid UnjoinEdge selection"));
    }
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut constructed = Vec::new();
    for (index, source) in f.sources.iter().enumerate() {
        let geometry = Geometry::Brep(source.brep.build(tolerance)?);
        if let Some(path) = &source.brep.artifact_path {
            crate::brep_source::write_shared_artifact(&geometry, path, tolerance)?;
        }
        constructed.push(untrim::geometry_record(&geometry, tolerance)?);
        let id = document.add_geometry_with_attributes(
            geometry,
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_name(format!("source-{index}"))
                .with_object_color(ColorRgb::new(10 + index as u8, 30, 50)),
        )?;
        ids.push(id);
        groups.push(document.add_group(Some(format!("source-{index}")), [id])?);
    }
    for &(source, index) in &f.components {
        let Geometry::Brep(brep) = document.object(ids[source]).unwrap().geometry() else {
            unreachable!()
        };
        let count = if f.kind == Kind::Edge {
            brep.edges().len()
        } else {
            brep.faces().len()
        };
        if index >= count {
            return Err(ProbeError::FixtureInvariant(
                "UnjoinEdge component outside source",
            ));
        }
    }
    if f.object_preselect {
        document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    }
    let before = untrim::snapshot(&document, &ids, &groups)?;
    document.clear_selection();
    document.clear_history()?;
    let selected = if f.pick == Pick::Preselect {
        f.components
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|(source, index)| {
                json!([
                    source,
                    if f.kind == Kind::Edge { "edge" } else { "face" },
                    index
                ])
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let picks = if f.kind == Kind::Edge {
        // Native preselection enumerates component tables; mouse selection
        // retains click order. The public command processes document order.
        if f.pick == Pick::Preselect {
            f.components
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            f.components.clone()
        }
    } else {
        Vec::new()
    };
    let picks = picks.into_iter().map(|(source, edge)| (ids[source], edge));
    let prepared = if f.pick == Pick::Preselect {
        UnjoinEdgeSelection::prepare_preselected(&document, picks)?
    } else {
        UnjoinEdgeSelection::prepare(&document, picks)?
    };
    let succeeded =
        prepared.changes_geometry() && (f.pick == Pick::Preselect || f.finish == Finish::Enter);
    if succeeded {
        prepared.commit(&mut document)?;
    }
    let after = untrim::snapshot(&document, &ids, &groups)?;
    let mut result = json!({"constructed":constructed,"before":before,"after":after,
        "succeeded":succeeded,"component_selection":{"before":selected,"after":[]}});
    if f.undo_redo {
        result["history_tested"] = json!(succeeded);
        if succeeded {
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
