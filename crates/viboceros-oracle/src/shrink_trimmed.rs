//! Whole-object shrink command replay from independent shared B-rep sources.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ShrinkFixture {
    sources: Vec<Source>,
    preselect: bool,
    order: Vec<usize>,
    undo_redo: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    brep: crate::brep_source::BrepSourceFixture,
}

pub(super) fn run(
    f: &ShrinkFixture,
    tolerance: Tolerance,
    command: &str,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=8).contains(&f.sources.len())
        || f.order.len() != f.sources.len()
        || f.order.iter().copied().collect::<BTreeSet<_>>()
            != (0..f.sources.len()).collect::<BTreeSet<_>>()
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid shrink source selection order",
        ));
    }
    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut constructed = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        let geometry = Geometry::Brep(source.brep.build(tolerance)?);
        if let Some(path) = &source.brep.artifact_path {
            crate::brep_source::write_shared_artifact(&geometry, path, tolerance)?;
        }
        constructed.push(untrim::geometry_record(&geometry, tolerance)?);
        let id = document.add_geometry_with_attributes(
            geometry,
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_name(format!("source-{i}"))
                .with_object_color(ColorRgb::new(10 + i as u8, 30, 50)),
        )?;
        ids.push(id);
        groups.push(document.add_group(Some(format!("source-{i}")), [id])?);
    }
    if f.preselect {
        document.select_objects_direct(f.order.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
    }
    let before = untrim::snapshot(&document, &ids, &groups)?;
    document.clear_history()?;
    if !f.preselect {
        document.select_objects_direct(f.order.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
    }
    let succeeded = if f.preselect {
        registry.execute(&mut document, command)
    } else {
        registry.execute_postselected(&mut document, command, Default::default())
    }
    .is_ok();
    let after = untrim::snapshot(&document, &ids, &groups)?;
    let mut result =
        json!({"constructed":constructed,"before":before,"after":after,"succeeded":succeeded});
    if f.undo_redo {
        let tested = document.can_undo();
        result["history_tested"] = json!(tested);
        if tested {
            document.undo()?;
            result["undo"] = json!(untrim::snapshot(&document, &ids, &groups)?);
            document.redo()?;
            result["redo"] = json!(untrim::snapshot(&document, &ids, &groups)?);
        }
    }
    Ok((result, 0))
}
