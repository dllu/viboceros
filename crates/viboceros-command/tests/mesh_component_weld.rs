//! Indexed and picked component welding retains channels and document history.
use serde_json::Value;
#[path = "support/mesh_seams.rs"]
#[allow(dead_code)]
mod native;

#[test]
fn component_welding_channels_survive_selectors_aliases_and_history() {
    let known: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_component_weld_gaps.json"
    ))
    .unwrap();
    let mut checks = 0;
    for text in [
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_weld_commands.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_weld_followup.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_weld_holdout.json"),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        for row in data["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let value = &row["value"];
            if value.get("invalid_source") == Some(&serde_json::json!(true))
                || value["input"]["kind"] == "whole"
            {
                continue;
            }
            let expected = serde_json::from_value::<Vec<String>>(
                known.get(name).cloned().unwrap_or(serde_json::json!([])),
            )
            .unwrap();
            let check = |command: &str, history: &str| {
                native::check_with_differences(name, value, command, history, &expected)
            };
            let indices = value["input"]["indices"].as_array().unwrap();
            if indices.is_empty() {
                continue;
            }
            let edge = value["input"]["kind"] == "edge";
            let command = if edge { "WeldEdge" } else { "WeldVertices" };
            let selector = if edge { "Edges" } else { "Vertices" };
            let indices = indices
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            check(&format!("{command} {selector}={indices}"), command);
            checks += 1;
            if name.ends_with("_cube_all") {
                check(&format!("{command} {selector}=All"), command);
                checks += 1;
            }
            if name.ends_with("_bent") {
                let picked = if edge {
                    "WeldMeshEdge 2,0,0"
                } else {
                    "WeldMeshVertex 0,0,0"
                };
                check(picked, command);
                let indexed = if edge {
                    "WeldEdge EdgeIndices 0"
                } else {
                    "WeldVertices VertexIndices 0"
                };
                check(indexed, command);
                checks += 2;
            }
            if let Some(recipe) = value.get("recipe_mesh")
                && recipe["normals"].as_array().unwrap().is_empty()
            {
                let mut from_recipe = value.clone();
                from_recipe["before"] = recipe.clone();
                native::check_with_differences(
                    &(name.to_owned() + "_from_recipe"),
                    &from_recipe,
                    &format!("{command} {selector}={indices}"),
                    command,
                    &[],
                );
                checks += 1;
            }
        }
    }
    assert_eq!(checks, 66);
}
