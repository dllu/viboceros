use serde_json::{Value, json};
use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn triangle_protocol_uses_sdk_unification_for_nonmanifold_disjoint_and_mobius_meshes() {
    for (family, id) in [
        ("sdk", "unify_normals_nonmanifold"),
        ("sdk", "unify_normals_disjoint"),
        ("extra", "unify_extra_mobius"),
    ] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
            .unwrap();
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "tools/rhino_oracle/observations/unify_normals_{family}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        let v = &data["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["value"];
        let request: ProbeRequest = serde_json::from_value(json!({
            "protocol_version": 1, "iterations": 1,
            "operations": [{"op":"mesh_unify_normals", "id":id,
                "vertices":v["before"]["vertices"], "triangles":v["before"]["faces"]}]
        }))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(
            response.results[0].value,
            json!({
                "flipped_faces":v["changed"], "triangles":v["after"]["faces"]
            })
        );
    }
}
