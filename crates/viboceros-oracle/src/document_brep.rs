//! Document admission on identical source geometry, independent of kernel queries.
use super::*;
use crate::solid_orientation::{SolidOrientationFixture, geometry_record};
use viboceros_document::ReplacementHistory;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DocumentBrepFixture {
    #[serde(flatten)]
    source: SolidOrientationFixture,
    // Rhino has two public insertion overloads. Native insertion has no kink
    // splitting option; retaining the field ensures both adapters use one recipe.
    #[serde(default)]
    insertion: Insertion,
    #[serde(default)]
    selected: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Insertion {
    #[default]
    Generic,
    NoKink,
}

pub(super) fn run(
    fixture: &DocumentBrepFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "document B-rep admission requires one iteration",
        ));
    }
    let source = fixture.source.build(tolerance)?;
    fixture.source.write_artifact(&source, tolerance)?;
    let input = geometry_record(&source)?;
    // Reverse the caller-owned source, NOT the inserted object: insertion may
    // differ between engines, but both replacement calls must get the same input.
    let replacement = source.reversed();
    let replacement_input = geometry_record(&replacement)?;
    let mut document = Document::new(tolerance);
    let id = document.add_geometry_with_attributes(
        Geometry::Brep(source.clone()),
        ObjectAttributes::on_layer(document.current_layer_id()).with_name("Source"),
    )?;
    document.add_group(Some("Source group".into()), [id])?;
    if fixture.selected {
        document.select_objects_direct([id], SelectionMode::Replace)?;
    }
    let inserted = snapshot(&document, id)?;
    let replaced = document.replace_object_geometries_with_history(
        [(id, Geometry::Brep(replacement.clone()))],
        ReplacementHistory::EveryReplacement,
    )? == 1;
    Ok((
        json!({"input":input,"inserted":inserted,
            "replacement_input":replacement_input,"replacement":snapshot(&document,id)?,
            "replaced":replaced,"source_unchanged":input==geometry_record(&source)?,
            "replacement_unchanged":replacement_input==geometry_record(&replacement)?}),
        0,
    ))
}

fn snapshot(document: &Document, id: ObjectId) -> Result<Value, ProbeError> {
    let object = document.object(id).ok_or(ProbeError::FixtureInvariant(
        "document admission lost object identity",
    ))?;
    let Geometry::Brep(brep) = object.geometry() else {
        return Err(ProbeError::FixtureInvariant(
            "document admission lost B-rep",
        ));
    };
    Ok(
        json!({"geometry":geometry_record(brep)?, "name":object.attributes().name(),
        "group_count":object.group_ids().len(),"selected":document.is_selected(id),
        "current_layer":object.attributes().layer_id()==document.current_layer_id(),
        "object_count":document.objects().len(),"identity_preserved":object.id()==id}),
    )
}

/// Import the actual shared file through the command, not through AddBrep.
pub(super) fn run_import(
    fixture: &SolidOrientationFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "B-rep import requires one iteration",
        ));
    }
    let source = fixture.build(tolerance)?;
    let input = geometry_record(&source)?;
    let temporary = OracleTemporaryFile::new("document-brep-import");
    let path = fixture
        .artifact_path
        .as_deref()
        .map(Path::new)
        .unwrap_or(&temporary.path);
    let path = path
        .to_str()
        .ok_or(ProbeError::FixtureInvariant("import path must be UTF-8"))?;
    if path.contains(['"', '\n', '\r']) {
        return Err(ProbeError::FixtureInvariant(
            "unsupported import path quoting",
        ));
    }
    crate::brep_source::write_shared_artifact(&Geometry::Brep(source.clone()), path, tolerance)?;
    let mut document = Document::new(tolerance);
    CommandRegistry::with_builtins().execute(&mut document, &format!("Import3dm \"{path}\""))?;
    if document.objects().len() != 1 {
        return Err(ProbeError::FixtureInvariant(
            "import must retain exactly one object",
        ));
    }
    let Geometry::Brep(imported) = document.objects().next().unwrap().geometry() else {
        return Err(ProbeError::FixtureInvariant("import lost B-rep"));
    };
    Ok((
        json!({"input":input,"imported":geometry_record(imported)?,
        "object_count":1,"succeeded":true,"source_unchanged":input==geometry_record(&source)?}),
        0,
    ))
}
