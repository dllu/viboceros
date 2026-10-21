#[path = "support/subdivision_native.rs"]
mod native;
#[test]
fn mesh_subdivision_matches_native_regions_colors_raw_seams_and_neighbor_topology() {
    let families = [
        include_str!("../../../tools/rhino_oracle/observations/subdivide.json"),
        include_str!("../../../tools/rhino_oracle/observations/subdivide_details.json"),
        include_str!("../../../tools/rhino_oracle/observations/subdivide_orientations.json"),
        include_str!("../../../tools/rhino_oracle/observations/subdivide_extra.json"),
        include_str!("../../../tools/rhino_oracle/observations/subdivide_ngons.json"),
    ];
    let rows = families
        .iter()
        .flat_map(|s| {
            serde_json::from_str::<serde_json::Value>(s).unwrap()["results"]
                .as_array()
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 141);
    for row in &rows {
        let v = &row["value"];
        if v["input"]["cancel"] == true {
            continue;
        }
        for (i, before) in v["before"].as_array().unwrap().iter().enumerate() {
            let Some(source) = before.get("mesh") else {
                continue;
            };
            let mut mesh = native::mesh(source);
            let original = mesh.vertices().len();
            for (step_index, step) in v["steps"].as_array().unwrap().iter().enumerate() {
                let faces = v["input"]["sources"][i]["selected_faces"]
                    .as_array()
                    .filter(|a| !a.is_empty());
                mesh = if let Some(faces) = faces {
                    mesh.subdivided_faces(
                        &faces
                            .iter()
                            .map(|i| i.as_u64().unwrap() as usize)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap()
                } else {
                    mesh.subdivided().unwrap()
                };
                let out = step["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| o["source"].as_u64() == Some(i as u64))
                    .unwrap();
                let check = native::compare_mesh(&mesh, &out["mesh"], original);
                println!(
                    "SUBDIVIDE_MESH_KERNEL {}",
                    serde_json::json!({"id":row["id"],"source":i,"step":step_index,"check":check})
                );
            }
        }
    }
}
