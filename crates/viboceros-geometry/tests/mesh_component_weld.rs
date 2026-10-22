//! Native selected-component welding compares every stored raw vertex channel.
use serde_json::{Value, json};
#[path = "support/mesh_equivalence.rs"]
mod equivalence;
#[path = "support/mesh_channels.rs"]
mod native;

#[test]
fn selected_component_welding_matches_native_channels() {
    let known: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_component_weld_gaps.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    let mut compared = 0;
    for text in [
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_weld_commands.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_weld_followup.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_weld_holdout.json"),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        for row in data["results"].as_array().unwrap() {
            let value = &row["value"];
            if value.get("invalid_source") == Some(&json!(true))
                || value["input"]["kind"] == "whole"
            {
                continue;
            }
            assert_eq!(value["before"]["valid"], true);
            let source = native::mesh(&value["before"]);
            let original = source.clone();
            let indices =
                serde_json::from_value::<Vec<usize>>(value["input"]["indices"].clone()).unwrap();
            let (result, _) = if value["input"]["kind"] == "edge" {
                source.welded_topology_edges(&indices)
            } else {
                source.welded_topology_vertices(&indices)
            }
            .unwrap();
            assert_eq!(source, original);
            let differences = native::differences(&result, &value["after"]);
            let topology_only = |mesh: viboceros_geometry::TriangleMesh| {
                mesh.try_with_vertex_colors(None)
                    .unwrap()
                    .try_with_texture_coordinates(None)
                    .unwrap()
                    .try_with_vertex_normals(None)
                    .unwrap()
            };
            let topology_qualified = equivalence::equivalent(
                &topology_only(result.clone()),
                &topology_only(native::mesh(&value["after"])),
            );
            println!(
                "MESH_COMPONENT_WELD_NATIVE {}",
                json!({"id":row["id"],"differences":differences,"qualified":differences.is_empty(),"topology_qualified":topology_qualified,"vertices":result.vertices().len(),"faces":result.face_count()})
            );
            if !differences.is_empty() {
                println!(
                    "MESH_COMPONENT_WELD_DETAIL {}",
                    json!({"id":row["id"],"faces":result.faces().iter().map(|f| f.indices()).collect::<Vec<_>>(),"uv":result.texture_coordinates(),"normals":result.vertex_normals().unwrap_or_default().iter().map(|n| n.to_array()).collect::<Vec<_>>(),"colors":result.vertex_colors()})
                );
            }
            if !topology_qualified
                || json!(differences)
                    != known
                        .get(row["id"].as_str().unwrap())
                        .cloned()
                        .unwrap_or(json!([]))
            {
                failures.push((row["id"].clone(), differences));
            }
            compared += 1;
            if let Some(recipe) = value.get("recipe_mesh")
                && recipe["normals"].as_array().unwrap().is_empty()
            {
                let source = native::mesh(recipe);
                let (result, _) = if value["input"]["kind"] == "edge" {
                    source.welded_topology_edges_with_normal_generation(&indices, true)
                } else {
                    source.welded_topology_vertices_with_normal_generation(&indices, true)
                }
                .unwrap();
                let differences = native::differences(&result, &value["after"]);
                println!(
                    "MESH_COMPONENT_WELD_RECIPE {}",
                    json!({"id":row["id"],"differences":differences,"qualified":differences.is_empty(),"vertices":result.vertices().len(),"faces":result.face_count()})
                );
                assert!(
                    differences.is_empty(),
                    "{} from recipe: {differences:?}",
                    row["id"]
                );
            }
        }
    }
    assert_eq!(compared, 56);
    assert!(failures.is_empty(), "{failures:?}");
}
