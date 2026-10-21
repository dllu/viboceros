use viboceros_command::CommandRegistry;
#[path = "support/mesh_from_lines.rs"]
mod native;
#[test]
fn line_network_commands_preserve_sources_attributes_and_grouped_history() {
    for (prefix, rows) in native::groups() {
        for row in rows {
            let value = &row["value"];
            let mut scene = native::setup(value);
            assert_eq!(
                scene.sources.len(),
                value["before"].as_array().unwrap().len()
            );
            CommandRegistry::with_builtins()
                .execute(&mut scene.document, &native::command(value))
                .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
            let mut error = native::check(&scene.document, &scene.parents, value, "after_script");
            if native::has_output(value) {
                assert_eq!(scene.document.undo_label(), Some("MeshFromLines"));
                scene.document.undo().unwrap();
                error = error.max(native::check(
                    &scene.document,
                    &scene.parents,
                    value,
                    "undo",
                ));
                scene.document.redo().unwrap();
                error = error.max(native::check(
                    &scene.document,
                    &scene.parents,
                    value,
                    "redo",
                ));
            } else {
                assert!(scene.document.undo_label().is_none());
            }
            let supported = native::native_options_supported(value);
            println!(
                "{prefix}_COMMAND {}",
                serde_json::json!({"id":row["id"],"qualified":error<1e-6&&supported,"geometry_qualified":error<1e-6,"native_options_supported":supported,"maximum_bidirectional_sample_error":error,"native_empty":!native::has_output(value),"sources_and_attributes_preserved":true,"grouped_history":true})
            );
            assert!(error < 1e-6, "{} {error:e}", row["id"]);
        }
    }
}

#[test]
fn no_regions_and_invalid_options_preserve_redo_and_sources() {
    let (_, rows) = native::groups().remove(0);
    let value = &rows[1]["value"];
    let mut scene = native::setup(value);
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut scene.document, "Point 9,9,9")
        .unwrap();
    scene.document.undo().unwrap();
    let redo = scene.document.redo_label().map(str::to_owned);
    let count = scene.document.objects().count();
    for input in [
        "MeshFromLines MaxSidesPerFace=2",
        "MeshFromLines MaxSidesPerFace=4 MaxSidesPerFace=5",
        "MeshFromLines OutputLayer=missing",
    ] {
        assert!(registry.execute(&mut scene.document, input).is_err());
        assert_eq!(scene.document.objects().count(), count);
        assert_eq!(scene.document.redo_label(), redo.as_deref());
    }
    registry
        .execute(&mut scene.document, "MeshFromLines MaxSidesPerFace=3")
        .unwrap();
    assert_eq!(scene.document.objects().count(), count);
    assert_eq!(scene.document.redo_label(), redo.as_deref());
    assert!(scene.document.undo_label().is_none());
}
#[test]
fn explicit_layer_ids_do_not_change_the_current_layer() {
    let (_, rows) = native::groups().remove(0);
    let mut scene = native::setup(&rows[0]["value"]);
    let current = scene.document.current_layer_id();
    let source = scene
        .document
        .layer_by_name("MeshFromLines source")
        .unwrap()
        .id();
    CommandRegistry::with_builtins()
        .execute(
            &mut scene.document,
            &format!("MeshFromLines OutputLayer={source}"),
        )
        .unwrap();
    let output = scene
        .document
        .objects()
        .find(|o| !scene.sources.contains(&o.id()))
        .unwrap();
    assert_eq!(output.attributes().layer_id(), source);
    assert!(output.attributes().name().is_none());
    assert_eq!(scene.document.current_layer_id(), current);
    scene.document.undo().unwrap();
    assert_eq!(scene.document.objects().count(), scene.parents.len());
}
