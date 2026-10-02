//! Actual ExtractSrf edits replayed from independently generated B-reps.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ExtractFixture {
    sources: Vec<Source>,
    components: Vec<(usize, usize)>,
    copy: bool,
    output_current: bool,
    source_layer: bool,
    undo_redo: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    brep: crate::brep_source::BrepSourceFixture,
}

pub(super) fn run(f: &ExtractFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if !(1..=8).contains(&f.sources.len()) || !(1..=64).contains(&f.components.len()) {
        return Err(ProbeError::FixtureInvariant(
            "invalid ExtractSrf sources or face targets",
        ));
    }
    let mut doc = Document::new(tolerance);
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut constructed = Vec::new();
    let layer = if f.source_layer {
        doc.add_layer("sources", ColorRgb::new(0, 0, 0))?
    } else {
        doc.current_layer_id()
    };
    for (i, source) in f.sources.iter().enumerate() {
        let geometry = Geometry::Brep(source.brep.build(tolerance)?);
        if let Some(path) = &source.brep.artifact_path {
            crate::brep_source::write_shared_artifact(&geometry, path, tolerance)?;
        }
        constructed.push(untrim::geometry_record(&geometry, tolerance)?);
        let id = doc.add_geometry_with_attributes(
            geometry,
            ObjectAttributes::on_layer(layer)
                .with_name(format!("source-{i}"))
                .with_object_color(ColorRgb::new(10 + i as u8, 30, 50)),
        )?;
        ids.push(id);
        groups.push(doc.add_group(Some(format!("source-{i}")), [id])?);
    }
    let mut faces = BTreeSet::new();
    for &(source, face) in &f.components {
        if source >= ids.len() {
            return Err(ProbeError::FixtureInvariant(
                "ExtractSrf source outside fixture",
            ));
        }
        faces.insert((source, face));
    }
    let before = untrim::snapshot(&doc, &ids, &groups)?;
    doc.clear_history()?;
    let stage = viboceros_command::ExtractSurfaceSelection::prepare(
        &doc,
        faces.iter().map(|&(s, f)| (ids[s], f)),
        f.copy,
        f.output_current,
    )?;
    let succeeded = stage.commit(&mut doc).is_ok();
    let components = faces
        .iter()
        .map(|&(s, f)| json!([s, "face", f]))
        .collect::<Vec<_>>();
    let mut result = json!({"constructed":constructed,"before":before,"after":untrim::snapshot(&doc,&ids,&groups)?,
        "succeeded":succeeded,"component_selection":{"before":components,"after":[]}});
    if f.undo_redo {
        doc.undo()?;
        result["undo"] = json!(untrim::snapshot(&doc, &ids, &groups)?);
        doc.redo()?;
        result["redo"] = json!(untrim::snapshot(&doc, &ids, &groups)?);
        result["component_selection"]["undo"] = json!([]);
        result["component_selection"]["redo"] = json!([]);
    }
    Ok((result, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracted_faces_remainders_metadata_order_and_history_match_native() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/extract_srf_faces.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/extract_srf_faces.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 60);
        assert_eq!(observed["results"].as_array().unwrap().len(), 60);
        for (row, native) in actual
            .results
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            assert_eq!(row.id, native["id"]);
            let mut expected = native["value"].clone();
            for key in ["events", "history", "undo_events", "redo_events"] {
                expected.as_object_mut().unwrap().remove(key);
            }
            crate::test_json::close(&row.value, &expected, &row.id, 1e-9, 0.);
        }
    }
}
