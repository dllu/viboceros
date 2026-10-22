//! Native selected-component geometry, raw ordering and stored-channel checks.
use serde_json::{Value, json};
#[path = "support/mesh_equivalence.rs"]
mod equivalence;
#[path = "support/mesh_channels.rs"]
mod native;

fn run(v: &Value) -> viboceros_geometry::TriangleMesh {
    assert_eq!(v["before"]["valid"], true);
    let source = native::mesh(&v["before"]);
    let original = source.clone();
    let indices = serde_json::from_value::<Vec<usize>>(v["input"]["indices"].clone()).unwrap();
    let modify = v["input"]["modify_normals"].as_bool().unwrap();
    let (result, _) = match v["input"]["kind"].as_str().unwrap() {
        "edge" => source.unwelded_topology_edges_with_normal_update(&indices, modify),
        "vertex" => source.unwelded_topology_vertices_with_normal_update(&indices, modify),
        _ => unreachable!(),
    }
    .unwrap();
    assert_eq!(source, original);
    result
}

#[test]
fn component_unwelding_matches_native_stored_normal_policies() {
    let known: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_component_unweld_order_gaps.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    for text in [
        include_str!(
            "../../../tools/rhino_oracle/observations/mesh_component_unweld_verified.json"
        ),
        include_str!("../../../tools/rhino_oracle/observations/mesh_component_unweld_holdout.json"),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            let result = run(v);
            let differences = native::differences(&result, &v["after"]);
            let qualified = equivalence::equivalent(&result, &native::mesh(&v["after"]));
            println!(
                "COMPONENT_UNWELD_NATIVE {}",
                json!({"id":row["id"],"differences":differences,"qualified":qualified,"raw_order_qualified":differences.is_empty(),"vertices":result.vertices().len(),"faces":result.face_count()})
            );
            if !qualified
                || json!(differences)
                    != known
                        .get(row["id"].as_str().unwrap())
                        .cloned()
                        .unwrap_or(json!([]))
            {
                println!(
                    "COMPONENT_UNWELD_DETAIL {}",
                    json!({"id":row["id"],"faces":result.faces().iter().map(|f| f.indices()).collect::<Vec<_>>(),"vertices":result.vertices().iter().map(|p| p.to_array()).collect::<Vec<_>>(),"uv":result.texture_coordinates(),"normals":result.vertex_normals().unwrap_or_default().iter().map(|n| n.to_array()).collect::<Vec<_>>()})
                );
                failures.push((row["id"].clone(), differences));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn direct_sdk_differences_remain_explicit() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_component_unweld_channels.json"
    ))
    .unwrap();
    let known: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_component_unweld_sdk_gaps.json"
    ))
    .unwrap();
    let mut differences_changed = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for row in data["results"].as_array().unwrap() {
        let result = run(&row["value"]);
        let differences = native::differences(&result, &row["value"]["after"]);
        println!(
            "COMPONENT_UNWELD_SDK {}",
            json!({"id":row["id"],"differences":differences,"qualified":differences.is_empty(),"vertices":result.vertices().len(),"faces":result.face_count()})
        );
        if json!(differences)
            != known
                .get(row["id"].as_str().unwrap())
                .cloned()
                .unwrap_or(json!([]))
        {
            differences_changed.push(row["id"].clone());
        }
        if !differences.is_empty() {
            seen.insert(row["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(
        differences_changed.is_empty(),
        "SDK differences changed: {differences_changed:?}"
    );
    assert_eq!(seen, known.as_object().unwrap().keys().cloned().collect());
}

#[test]
fn vertex_renumbering_checks_channels_winding_and_sharing() {
    use viboceros_geometry::{Point3, Tolerance, TriangleMesh};
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_component_unweld_verified.json"
    ))
    .unwrap();
    let snapshot = &data["results"][0]["value"]["before"];
    let original = native::mesh(snapshot);
    assert!(equivalence::equivalent(&original, &original));
    for field in ["uv", "normals", "colors"] {
        let mut changed = snapshot.clone();
        match field {
            "uv" => changed[field][0][0] = json!(0.25),
            "normals" => changed[field][0] = json!([0, 1, 0]),
            "colors" => changed[field][0][3] = json!(1),
            _ => unreachable!(),
        }
        assert!(
            !equivalence::equivalent(&original, &native::mesh(&changed)),
            "{field}"
        );
    }
    let make = |labels: [u32; 4]| {
        let points = [
            [0., 0., 0.],
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [-1., 0., 0.],
            [-1., 1., 0.],
            [0., -1., 0.],
            [1., -1., 0.],
            [0., 1., 0.],
            [1., 1., 0.],
        ];
        TriangleMesh::try_new(
            points
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            labels
                .into_iter()
                .enumerate()
                .map(|(i, label)| [label, 2 + 2 * i as u32, 3 + 2 * i as u32])
                .collect(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let first = make([0, 0, 1, 1]);
    let second = make([0, 1, 0, 1]);
    assert!(!equivalence::equivalent(&first, &second));
    assert!(!equivalence::equivalent(&first, &first.reversed()));
}
