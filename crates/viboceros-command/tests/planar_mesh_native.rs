use viboceros_command::CommandRegistry;
#[path = "support/planar_mesh.rs"]
mod native;

#[test]
fn native_postselection_controls_preserve_selection_history_and_failed_geometry() {
    let registry = CommandRegistry::with_builtins();
    for row in native::post_rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        scene
            .document
            .select_objects_direct(
                scene.sources.clone(),
                viboceros_document::SelectionMode::Replace,
            )
            .unwrap();
        let result =
            registry.execute_postselected(&mut scene.document, "PlanarMesh", Default::default());
        assert_eq!(result.is_ok(), native::native_success(value));
        assert_eq!(
            native::check(
                &scene.document,
                &scene.sources,
                &scene.parents,
                value,
                "after_script"
            )["qualified"],
            true
        );
        if native::output_count(value) > 0 {
            scene.document.undo().unwrap();
            assert_eq!(
                native::check(
                    &scene.document,
                    &scene.sources,
                    &scene.parents,
                    value,
                    "undo"
                )["qualified"],
                true
            );
            scene.document.redo().unwrap();
            assert_eq!(
                native::check(
                    &scene.document,
                    &scene.sources,
                    &scene.parents,
                    value,
                    "redo"
                )["qualified"],
                true
            );
        } else {
            assert!(scene.document.undo_label().is_none());
        }
    }
}

#[test]
fn planar_mesh_regions_preserve_sources_and_native_document_history() {
    let registry = CommandRegistry::with_builtins();
    let mut geometry_gaps = Vec::new();
    let mut topology_gaps = Vec::new();
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let result = registry.execute(&mut scene.document, "PlanarMesh");
        let status_match = result.is_ok() == native::native_success(value);
        let mut check = native::check(
            &scene.document,
            &scene.sources,
            &scene.parents,
            value,
            "after_script",
        );
        let geometry = check["geometry_qualified"] == true && status_match;
        if !geometry {
            geometry_gaps.push(row["id"].clone());
        }
        if check["qualified"] != true {
            topology_gaps.push(row["id"].clone());
        }
        if native::output_count(value) > 0 && result.is_ok() {
            assert_eq!(scene.document.undo_label(), Some("PlanarMesh"));
            scene.document.undo().unwrap();
            let undo = native::check(
                &scene.document,
                &scene.sources,
                &scene.parents,
                value,
                "undo",
            );
            assert_eq!(undo["geometry_qualified"], true);
            scene.document.redo().unwrap();
            check = native::check(
                &scene.document,
                &scene.sources,
                &scene.parents,
                value,
                "redo",
            );
        } else {
            assert!(scene.document.undo_label().is_none());
        }
        println!(
            "PLANAR_MESH_COMMAND {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"]==true&&status_match,"geometry_qualified":geometry,
            "check":check,"native_status_match":status_match,"grouped_history":true,"local_error":result.err().map(|e|e.to_string())})
        );
    }
    println!("geometry gaps {geometry_gaps:?}; topology gaps {topology_gaps:?}");
    assert!(geometry_gaps.is_empty(), "{geometry_gaps:?}");
}

#[test]
fn invalid_sources_and_failed_construction_preserve_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let rows = native::rows();
    for row in [
        &rows[0],
        rows.iter().find(|r| r["id"] == "nonplanar").unwrap(),
    ] {
        let mut scene = native::setup(&row["value"]);
        registry
            .execute(&mut scene.document, "Point 9,9,9")
            .unwrap();
        scene.document.undo().unwrap();
        let redo = scene.document.redo_label().map(str::to_owned);
        let mut commands = vec![
            "PlanarMesh Unknown=Yes".to_owned(),
            "PlanarMesh Sources=999999999".to_owned(),
            format!(
                "PlanarMesh Sources={},{}",
                scene.sources[0], scene.sources[0]
            ),
        ];
        if row["id"] == "nonplanar" {
            commands.push("PlanarMesh".into());
        }
        for command in commands {
            assert!(registry.execute(&mut scene.document, &command).is_err());
            assert_eq!(scene.document.objects().count(), scene.parents.len());
            assert!(scene.document.is_selected(scene.sources[0]));
            assert_eq!(scene.document.redo_label(), redo.as_deref());
        }
    }
}
