use serde_json::json;
use viboceros_command::CommandRegistry;
use viboceros_geometry::TriangleMesh;
#[path = "support/closed_polyline.rs"]
mod native;

#[test]
fn closed_polyline_sdk_captures_keep_spatial_and_topology_gaps_visible() {
    let mut gaps = Vec::new();
    for row in native::rows() {
        let v = &row["value"];
        let mut comparisons = Vec::new();
        for (points, target) in v["input"]["polylines"]
            .as_array()
            .unwrap()
            .iter()
            .zip(v["sdk"].as_array().unwrap())
        {
            let mesh = TriangleMesh::try_from_closed_polyline(&native::polyline(points)).unwrap();
            comparisons.push(native::compare(mesh.as_ref(), target));
        }
        let qualified = comparisons.iter().all(|c| c["qualified"] == true);
        if !qualified {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        println!(
            "CLOSED_POLYLINE_GEOMETRY {}",
            json!({"id":row["id"],"qualified":qualified,"comparisons":comparisons})
        );
    }
    assert_eq!(gaps, vec!["self_crossing"]);
}

#[test]
fn closed_polyline_commands_copy_attributes_and_group_multiple_outputs() {
    let registry = CommandRegistry::with_builtins();
    let mut gaps = Vec::new();
    for row in native::rows() {
        let v = &row["value"];
        if native::cancelled(v) {
            println!(
                "CLOSED_POLYLINE_COMMAND {}",
                json!({"id":row["id"],"qualified":false,"native_cancelled":true})
            );
            continue;
        }
        let mut scene = native::setup(v);
        registry
            .execute(&mut scene.document, "MeshPolyline")
            .unwrap();
        let check = native::check(
            &scene.document,
            &scene.sources,
            &scene.parents,
            v,
            "after_script",
        );
        let qualified = check["qualified"] == true;
        if !qualified {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        if qualified && native::output_count(v) > 0 {
            assert_eq!(scene.document.undo_label(), Some("MeshPolyline"));
            scene.document.undo().unwrap();
            assert_eq!(
                native::check(&scene.document, &scene.sources, &scene.parents, v, "undo")["qualified"],
                true
            );
            scene.document.redo().unwrap();
            assert_eq!(
                native::check(&scene.document, &scene.sources, &scene.parents, v, "redo")["qualified"],
                true
            );
        } else if native::output_count(v) == 0 {
            assert!(scene.document.undo_label().is_none());
        }
        println!(
            "CLOSED_POLYLINE_COMMAND {}",
            json!({"id":row["id"],"qualified":qualified,"check":check,"grouped_history":qualified})
        );
    }
    assert_eq!(gaps, vec!["self_crossing"]);
}

#[test]
fn invalid_sources_are_atomic_and_empty_geometry_preserves_redo() {
    let rows = native::rows();
    let mut scene = native::setup(&rows[0]["value"]);
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut scene.document, "Point 9,9,9")
        .unwrap();
    scene.document.undo().unwrap();
    let redo = scene.document.redo_label().map(str::to_owned);
    for command in [
        "MeshPolyline Unknown=Yes".to_owned(),
        format!(
            "MeshPolyline Sources={},{}",
            scene.sources[0], scene.sources[0]
        ),
        "MeshPolyline Sources=999999999".to_owned(),
    ] {
        assert!(registry.execute(&mut scene.document, &command).is_err());
        assert_eq!(scene.document.objects().count(), scene.parents.len());
        assert!(scene.document.is_selected(scene.sources[0]));
        assert_eq!(scene.document.redo_label(), redo.as_deref());
    }
    let collinear = rows.iter().find(|r| r["id"] == "collinear").unwrap();
    let mut scene = native::setup(&collinear["value"]);
    registry
        .execute(&mut scene.document, "Point 9,9,9")
        .unwrap();
    scene.document.undo().unwrap();
    let redo = scene.document.redo_label().map(str::to_owned);
    registry
        .execute(&mut scene.document, "MeshPolyline")
        .unwrap();
    assert_eq!(scene.document.objects().count(), scene.parents.len());
    assert_eq!(scene.document.selected_object_count(), 0);
    assert_eq!(scene.document.redo_label(), redo.as_deref());
    assert!(scene.document.undo_label().is_none());
}
