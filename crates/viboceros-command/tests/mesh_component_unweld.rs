//! Component normal policies survive indexed selection, picking and history.
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
#[path = "support/mesh_seams.rs"]
mod native;

#[test]
fn selected_component_normal_policies_match_native_commands_and_history() {
    let mut checks = 0;
    for (text, strict) in [
        (
            include_str!(
                "../../../tools/rhino_oracle/observations/mesh_component_unweld_verified.json"
            ),
            true,
        ),
        (
            include_str!(
                "../../../tools/rhino_oracle/observations/mesh_component_unweld_holdout.json"
            ),
            false,
        ),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        for row in data["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let indices = v["input"]["indices"].as_array().unwrap();
            if indices.is_empty() {
                continue;
            }
            let edge = v["input"]["kind"] == "edge";
            let command = if edge { "UnweldEdge" } else { "UnweldVertex" };
            let selector = if edge { "Edges" } else { "Vertices" };
            // Native component preselection is a set. The text grammar rejects
            // duplicate indices, while the geometry API accepts them directly.
            let indices = indices
                .iter()
                .map(|i| i.as_u64().unwrap())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let update = v["input"]["modify_normals"].as_bool().unwrap();
            let check = |text: &str| {
                if strict {
                    native::check(name, v, text, command);
                } else {
                    native::check_with_geometry(name, v, text, command, false);
                }
            };
            check(&format!(
                "{command} {selector}={indices} ModifyNormals={}",
                if update { "Yes" } else { "No" }
            ));
            checks += 1;
            if update {
                check(&format!("{command} {selector}={indices}"));
                checks += 1;
            }
            if name.contains("_bent_") {
                let pick = if edge {
                    "UnweldMeshEdge 2,0,0"
                } else {
                    "UnweldVertices 0,0,0"
                };
                check(&format!(
                    "{pick} ModifyNormals={}",
                    if update { "Yes" } else { "No" }
                ));
                checks += 1;
            }
        }
    }
    assert_eq!(checks, 106);
}

#[test]
fn malformed_normal_policies_and_components_preserve_redo() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, "MeshBox 0,0,0 3,4,0 5")
        .unwrap();
    let source = document.objects().next().unwrap().id();
    document
        .select_objects_direct([source], viboceros_document::SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Point 9,9,9").unwrap();
    document.undo().unwrap();
    assert!(document.is_selected(source));
    let original = document.object(source).unwrap().geometry_snapshot().clone();
    for command in [
        "UnweldEdge Edges=0 ModifyNormals=Maybe",
        "UnweldEdge Edges=0,0 ModifyNormals=No",
        "UnweldEdge Edges=0 ModifyNormals=No ModifyNormals=Yes",
        "UnweldVertex Vertices=0 ModifyNormals=No ModifyNormals=No",
        "UnweldEdge Edges=999 ModifyNormals=No",
        "UnweldVertex Vertices=999 ModifyNormals=No",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(
            document.object(source).unwrap().geometry_snapshot(),
            &original
        );
        assert_eq!(document.redo_label(), Some("Point"));
        assert!(matches!(
            document.object(source).unwrap().geometry(),
            Geometry::Mesh(_)
        ));
    }
}
