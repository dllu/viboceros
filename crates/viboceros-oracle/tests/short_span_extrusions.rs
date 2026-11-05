#[path = "support/short_span_extrusions.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/edge_analysis_mixed.rs"]
mod values;

#[test]
fn microscopic_rational_spans_match_native_creators_selection_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/short_span_extrusions.json")).unwrap(),
    )
    .unwrap();
    let actual = serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
    let native = observation::read(&base, "short_span_extrusions");
    assert_eq!(actual["results"].as_array().unwrap().len(), 16);
    for (a, b) in actual["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(a["id"], b["id"]);
        for field in ["before", "after", "undo", "redo"] {
            values::compare(
                &a["value"][field],
                &b["value"][field],
                &format!("{}/{field}", a["id"]),
            );
            comparison::exact_basis(
                &a["value"][field],
                &b["value"][field],
                &format!("{}/{field}", a["id"]),
            );
        }
    }
}

#[test]
fn short_span_extrusion_archives_roundtrip_through_3dm() {
    use viboceros_command::CommandRegistry;
    use viboceros_document::Geometry;
    use viboceros_geometry::Tolerance;
    use viboceros_oracle::short_span_extrusions::{Fixture, setup};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/short_span_extrusions.json")).unwrap(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for op in request["operations"].as_array().unwrap() {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let (mut doc, source) = setup(&f, Tolerance::DEFAULT).unwrap();
        let commands = CommandRegistry::with_builtins();
        commands.execute(&mut doc, &f.input()).unwrap();
        let output = doc.objects().find(|o| o.id() != source).unwrap();
        let id = output.id();
        let Geometry::Brep(original) = output.geometry() else {
            panic!("typed extrusion required")
        };
        let form = original.extrusion().unwrap().nurbs_form().unwrap();
        let path = temp.path().join("short-span.3dm");
        commands
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let model = viboceros_io::read_3dm_file(&path, doc.tolerance()).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        let output = model.objects.iter().find(|o| o.id == id.uuid()).unwrap();
        let viboceros_io::ThreeDmGeometry::Brep(restored) = &output.geometry else {
            panic!("extrusion lost in 3DM")
        };
        assert!(restored.is_extrusion());
        assert_eq!(
            restored.extrusion().unwrap().nurbs_form().unwrap().surface,
            form.surface
        );
        assert!(restored.tessellate_with_normals(8, doc.tolerance()).is_ok());
    }
}
