//! Actual selector dispatch over corresponding native representation scenes.
use super::*;
mod scene;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub command: String,
    pub scenario: String,
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1
        || !matches!(
            fixture.command.as_str(),
            "SelExtrusion" | "SelBlockInstance" | "SelMirroredBlocks"
        )
        || !matches!(
            fixture.scenario.as_str(),
            "base"
                | "preselected"
                | "grouped"
                | "hidden"
                | "locked"
                | "hidden-layer"
                | "locked-layer"
                | "no-matches"
                | "no-matches-preselected"
        )
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid typed selection recipe",
        ));
    }
    let mut doc = Document::new(tolerance);
    let ids = scene::build(&mut doc, &fixture.scenario)?;
    let sentinel = doc.add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.)?))?;
    doc.undo()?;
    if matches!(
        fixture.scenario.as_str(),
        "preselected" | "no-matches-preselected"
    ) {
        doc.select_objects_direct([ids[4]], SelectionMode::Replace)?;
    }
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let definitions = doc.block_definitions().cloned().collect::<Vec<_>>();
    let layers = doc.layers().cloned().collect::<Vec<_>>();
    let groups = doc.groups().cloned().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    CommandRegistry::with_builtins().execute(&mut doc, &fixture.command)?;
    if before != doc.objects().cloned().collect::<Vec<_>>()
        || definitions != doc.block_definitions().cloned().collect::<Vec<_>>()
        || layers != doc.layers().cloned().collect::<Vec<_>>()
        || groups != doc.groups().cloned().collect::<Vec<_>>()
        || doc.undo_label() != undo.as_deref()
        || doc.redo_label() != redo.as_deref()
    {
        return Err(ProbeError::FixtureInvariant(
            "typed selection changed sources or history",
        ));
    }
    let selected = ids
        .iter()
        .enumerate()
        .filter_map(|(i, id)| doc.is_selected(*id).then_some(i))
        .collect::<Vec<_>>();
    doc.redo()?;
    let redo_preserved = doc.object(sentinel).is_some();
    let types = ids
        .iter()
        .map(|id| match doc.object(*id).unwrap().geometry() {
            Geometry::Brep(b) if b.is_extrusion() => "Extrusion",
            Geometry::Brep(_) => "Brep",
            Geometry::NurbsSurface(_) => "NurbsSurface",
            Geometry::Line(_) => "LineCurve",
            Geometry::Point(_) => "Point",
            Geometry::BlockInstance(_) => "InstanceReferenceGeometry",
            _ => unreachable!("fixed typed selection scene"),
        })
        .collect::<Vec<_>>();
    Ok((
        json!({"selected":selected,"types":types,"unchanged":true,"redo_preserved":redo_preserved}),
        0,
    ))
}

#[cfg(test)]
mod tests;
