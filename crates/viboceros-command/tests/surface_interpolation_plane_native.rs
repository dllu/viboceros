#[path = "support/interpolation_plane.rs"]
mod native;
use viboceros_command::CommandRegistry;
#[test]
fn infinite_plane_scripts_preserve_native_sources_metadata_history_and_curve_image() {
    let captured = native::read("surface_interpolate_plane_workflows");
    let audit = native::read("surface_interpolate_plane_audit");
    assert_eq!(captured["results"].as_array().unwrap().len(), 20);
    assert_eq!(audit["results"].as_array().unwrap().len(), 3);
    let mut completed = 0;
    for row in captured["results"]
        .as_array()
        .unwrap()
        .iter()
        .chain(audit["results"].as_array().unwrap())
    {
        let v = &row["value"];
        let control = v["control"].as_bool().unwrap();
        assert_eq!(v["success"], !control);
        assert_eq!(v["returned_idle"], true);
        if control {
            continue;
        } // Cancellation belongs to the interactive replay.
        completed += 1;
        assert_eq!(v["input_trace"][0]["errors"], serde_json::json!([]));
        let (mut doc, source) = native::setup(v);
        let original = doc.objects().cloned().collect::<Vec<_>>();
        let definition = native::definition(&doc, v);
        let command = native::command(v, &definition);
        CommandRegistry::with_builtins()
            .execute(&mut doc, &command)
            .unwrap();
        let mut error = native::check(&doc, source, v, "after");
        doc.undo().unwrap();
        error = error.max(native::check(&doc, source, v, "undo"));
        doc.redo().unwrap();
        error = error.max(native::check(&doc, source, v, "redo"));
        for original in original {
            assert_eq!(doc.object(original.id()), Some(&original));
            assert!(
                original
                    .geometry_snapshot()
                    .shares_storage_with(doc.object(original.id()).unwrap().geometry_snapshot())
            );
        }
        println!(
            "INTERPOLATION_PLANE_SCRIPT {}",
            serde_json::json!({"id":row["id"],"maximum_bidirectional_sample_error":error,"geometry_qualified":error<=1e-6})
        );
        assert!(error <= 1e-6, "{} {error:e}", row["id"]);
    }
    assert_eq!(completed, 18);
}
