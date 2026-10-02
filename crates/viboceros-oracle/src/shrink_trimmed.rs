//! Whole-object and face shrink replay from independent shared B-rep sources.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ShrinkFixture {
    sources: Vec<Source>,
    preselect: bool,
    order: Vec<usize>,
    undo_redo: bool,
    components: Option<Vec<(usize, usize)>>,
    #[serde(default)]
    objects: Vec<usize>,
    pick: Option<String>,
    finish: Option<String>,
    #[serde(default)]
    steps: Vec<FaceStep>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct FaceStep {
    kind: String,
    component: (usize, usize),
    modifiers: String,
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
    let mut components = BTreeSet::new();
    for &(source, face) in f.components.as_deref().unwrap_or(&[]) {
        let object = ids.get(source).and_then(|id| document.object(*id));
        if !matches!(object.map(|o| o.geometry()), Some(Geometry::Brep(b)) if face < b.faces().len())
        {
            return Err(ProbeError::FixtureInvariant("invalid shrink face target"));
        }
        components.insert((source, face));
    }
    let component_record = |picks: &BTreeSet<(usize, usize)>| -> Vec<Value> {
        picks.iter().map(|&(s, f)| json!([s, "face", f])).collect()
    };
    if f.preselect {
        let whole = if f.components.is_some() {
            &f.objects
        } else {
            &f.order
        };
        if whole.iter().any(|&i| i >= ids.len()) {
            return Err(ProbeError::FixtureInvariant("invalid shrink whole target"));
        }
        document.select_objects_direct(whole.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
    }
    let before = untrim::snapshot(&document, &ids, &groups)?;
    let selected_before = component_record(&components);
    document.clear_history()?;
    let mut trace = Vec::new();
    if f.pick.as_deref() == Some("sequence") {
        for step in &f.steps {
            let (source, face) = step.component;
            if step.kind != "click"
                || source >= ids.len()
                || !matches!(document.object(ids[source]).map(|o| o.geometry()), Some(Geometry::Brep(b)) if face < b.faces().len())
            {
                return Err(ProbeError::FixtureInvariant("invalid shrink face step"));
            }
            match step.modifiers.as_str() {
                "sub" => {
                    document.select_objects_direct([ids[source]], SelectionMode::Remove)?;
                    if !components.insert((source, face)) {
                        components.remove(&(source, face));
                    }
                }
                "plain" | "shift" => {
                    components.retain(|&(s, _)| s != source);
                    document.select_objects_direct([ids[source]], SelectionMode::Add)?;
                }
                "ctrl" => {
                    components.retain(|&(s, _)| s != source);
                    document.select_objects_direct([ids[source]], SelectionMode::Remove)?;
                }
                _ => return Err(ProbeError::FixtureInvariant("invalid shrink step modifier")),
            }
            trace.push(component_record(&components));
        }
    } else if !f.preselect {
        document.select_objects_direct(f.order.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
    }
    let succeeded = if f.finish.as_deref() == Some("Cancel") {
        document.clear_selection();
        false
    } else if f.components.is_some() && command == "ShrinkTrimmedSrf" {
        viboceros_command::ShrinkTrimmedSelection::prepare(
            &document,
            viboceros_geometry::BrepSurfaceShrinkMode::Standard,
            components.iter().map(|&(s, f)| (ids[s], f)),
        )
        .and_then(|p| p.commit(&mut document, !f.preselect))
        .is_ok()
    } else if f.preselect {
        registry.execute(&mut document, command).is_ok()
    } else {
        registry
            .execute_postselected(&mut document, command, Default::default())
            .is_ok()
    };
    let after = untrim::snapshot(&document, &ids, &groups)?;
    let mut result =
        json!({"constructed":constructed,"before":before,"after":after,"succeeded":succeeded});
    if f.components.is_some() {
        result["component_selection"] = json!({"before":selected_before,"after":[]});
    }
    if f.pick.as_deref() == Some("sequence") {
        result["selection_steps"] = json!(trace);
    }
    if f.undo_redo {
        let tested = document.can_undo();
        result["history_tested"] = json!(tested);
        if tested {
            document.undo()?;
            result["undo"] = json!(untrim::snapshot(&document, &ids, &groups)?);
            document.redo()?;
            result["redo"] = json!(untrim::snapshot(&document, &ids, &groups)?);
            if f.components.is_some() {
                result["component_selection"]["undo"] = json!([]);
                result["component_selection"]["redo"] = json!([]);
            }
        }
    }
    Ok((result, 0))
}
