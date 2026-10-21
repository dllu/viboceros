use viboceros_command::CommandRegistry;
#[path = "support/mesh_patch.rs"]
mod support;
#[test]
fn native_crossings_preserve_metadata_sources_and_history_with_visible_geometry_gaps() {
    for (group, rows) in support::crossing_groups() {
        let mut failures = Vec::new();
        for (op, v) in rows {
            let mut scene = support::setup(&v);
            let command = support::command(&scene, &v);
            CommandRegistry::with_builtins()
                .execute(&mut scene.document, &command)
                .unwrap_or_else(|e| panic!("{} {e}", op["id"]));
            let mut comparison =
                support::compare(&scene.document, &scene.parents, &v, "after_script");
            assert_eq!(scene.document.undo_label(), Some("MeshPatch"));
            scene.document.undo().unwrap();
            comparison.merge(support::compare(
                &scene.document,
                &scene.parents,
                &v,
                "undo",
            ));
            scene.document.redo().unwrap();
            comparison.merge(support::compare(
                &scene.document,
                &scene.parents,
                &v,
                "redo",
            ));
            println!(
                "MESH_PATCH_{group}_COMMAND {}",
                serde_json::json!({"id":op["id"],"qualified":comparison.qualified(),"maximum_bidirectional_sample_error":comparison.error,"counts_match":comparison.counts_match,"source_metadata_preserved":true,"grouped_history":true})
            );
            if !comparison.qualified() {
                failures.push(op["id"].as_str().unwrap().to_owned());
            }
        }
        support::assert_known_gaps(&failures);
    }
}
#[test]
fn native_mesh_patch_construction_metadata_sources_and_history() {
    for (op, v) in support::rows() {
        let mut scene = support::setup(&v);
        let command = support::command(&scene, &v);
        let result = CommandRegistry::with_builtins().execute(&mut scene.document, &command);
        if v["input"]["expect_failure"] == true {
            assert!(result.is_err());
            assert!(scene.document.undo_label().is_none());
            let error = support::check(&scene.document, &scene.parents, &v, "after_script");
            println!(
                "MESH_PATCH_COMMAND {}",
                serde_json::json!({"id":op["id"],"qualified":true,"native_failure":true,"maximum_bidirectional_sample_error":error})
            );
            continue;
        }
        result.unwrap_or_else(|e| panic!("{} {e}", op["id"]));
        let mut error = support::check(&scene.document, &scene.parents, &v, "after_script");
        assert_eq!(scene.document.undo_label(), Some("MeshPatch"));
        scene.document.undo().unwrap();
        error = error.max(support::check(&scene.document, &scene.parents, &v, "undo"));
        scene.document.redo().unwrap();
        error = error.max(support::check(&scene.document, &scene.parents, &v, "redo"));
        println!(
            "MESH_PATCH_COMMAND {}",
            serde_json::json!({"id":op["id"],"qualified":error<1e-6,"maximum_bidirectional_sample_error":error})
        );
        assert!(error < 1e-6, "{} {error:e}", op["id"]);
    }
}
