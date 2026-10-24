//! Frozen mesh split command and getter workflows.
use super::frozen_scene;
use serde_json::Value;
fn root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
}
pub fn rows() -> Vec<Value> {
    rows_for(&["discovery", "controls", "policy", "selection"])
}
pub fn rows_for(families: &[&str]) -> Vec<Value> {
    let mut rows = Vec::new();
    for &family in families {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(root().join(format!(
                "tools/rhino_oracle/observations/mesh_split_{family}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        for mut row in data["results"].as_array().unwrap().iter().cloned() {
            row["family"] = family.into();
            rows.push(row);
        }
    }
    rows
}
pub fn setup(row: &Value) -> frozen_scene::Scene {
    frozen_scene::setup(
        &root().join(format!(
            "tools/rhino_oracle/fixtures/mesh_split_{}/{}.3dm",
            row["family"].as_str().unwrap(),
            row["id"].as_str().unwrap()
        )),
        &row["value"],
        "Boolean output",
    )
}
pub fn name(row: &Value) -> &str {
    row["value"]["input"]["command_name"]
        .as_str()
        .unwrap_or("MeshSplit")
}
pub fn command(row: &Value, ids: &[viboceros_document::ObjectId]) -> String {
    let input = &row["value"]["input"];
    let mut result = format!(
        "{} {}",
        name(row),
        input["arguments"].as_str().unwrap_or("").replace('_', "")
    );
    if name(row) == "MeshSplit" {
        let set = |name: &str| {
            input[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ids[i.as_u64().unwrap() as usize].to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        result.push_str(&format!(" FirstSet={}", set("first")));
        if !input["second"].as_array().unwrap().is_empty() {
            result.push_str(&format!(" SecondSet={}", set("second")));
        }
        if input["cross_inputs"] == true {
            result.push_str(" PermitSplittingInputsWithEachOther");
        }
        if let Some(plane) = input["infinite_plane"].as_str() {
            result.push_str(&format!(" InfinitePlane={plane}"));
        }
    }
    result
}
pub fn context(row: &Value) -> viboceros_command::CommandContext {
    let frame = &row["value"]["input"]["cplane"];
    let construction_plane = if frame.is_array() {
        viboceros_geometry::Frame3::try_from_directions(
            viboceros_geometry::Point3::try_from(std::array::from_fn(|i| {
                frame[0][i].as_f64().unwrap()
            }))
            .unwrap(),
            viboceros_geometry::Vector3::try_from(std::array::from_fn(|i| {
                frame[1][i].as_f64().unwrap()
            }))
            .unwrap(),
            viboceros_geometry::Vector3::try_from(std::array::from_fn(|i| {
                frame[2][i].as_f64().unwrap()
            }))
            .unwrap(),
            viboceros_geometry::Tolerance::DEFAULT,
        )
        .unwrap()
    } else {
        viboceros_command::CommandContext::default().construction_plane
    };
    viboceros_command::CommandContext { construction_plane }
}
pub fn cancelled(row: &Value) -> bool {
    let input = &row["value"]["input"];
    input["cancel"] == true
        || input["cancel_second"] == true
        || input["menu"].is_string()
        || input["cross_inputs"] == true && input["second"].as_array().unwrap().is_empty()
}

/// Compare logical face boundaries and their covered surfaces independently of triangulation.
pub fn check(scene: &frozen_scene::Scene, value: &Value, stage: &str) -> Value {
    use serde_json::json;
    use viboceros_document::Geometry;
    let mut actual = super::boolean::snapshot(scene);
    for (row, object) in actual
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(scene.doc.objects())
    {
        if let Geometry::Mesh(mesh) = object.geometry() {
            row["mesh"]["ngons"] = json!(
                mesh.ngons()
                    .iter()
                    .map(|n| json!({"vertices":n.vertices(),"faces":n.faces()}))
                    .collect::<Vec<_>>()
            );
        } else if let Geometry::EmptyMesh = object.geometry() {
            row["mesh"]["ngons"] = json!([]);
        }
    }
    let ngons = actual.as_array().unwrap().len() == value[stage].as_array().unwrap().len()
        && actual
            .as_array()
            .unwrap()
            .iter()
            .zip(value[stage].as_array().unwrap())
            .all(|(a, n)| {
                let Some(expected) = n["mesh"]["ngons"].as_array() else {
                    return true;
                };
                let Some(actual) = a["mesh"]["ngons"].as_array() else {
                    return false;
                };
                if actual.len() != expected.len() {
                    return false;
                }
                let mut used = std::collections::BTreeSet::new();
                actual.iter().all(|ngon| {
                    let found = expected.iter().enumerate().find(|(i, other)| {
                        !used.contains(i) && same_ngon(&a["mesh"], ngon, &n["mesh"], other)
                    });
                    if let Some((i, _)) = found {
                        used.insert(i);
                        true
                    } else {
                        false
                    }
                })
            });
    let mut result = super::boolean::check_snapshot(actual, &value[stage]);
    result["ngons"] = ngons.into();
    result
}
pub fn check_with_channels(scene: &frozen_scene::Scene, value: &Value, stage: &str) -> Value {
    use serde_json::json;
    let mut result = check(scene, value, stage);
    for (row, object) in result["objects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(scene.doc.objects())
    {
        if let viboceros_document::Geometry::Mesh(mesh) = object.geometry() {
            row["mesh"]["uv"] = json!(mesh.texture_coordinates());
            row["mesh"]["vertex_normals"] = json!(
                mesh.vertex_normals()
                    .map(|ns| ns.iter().map(|n| n.to_array()).collect::<Vec<_>>())
            );
            // RhinoCommon uses opacity; the Rust/OpenNURBS channel stores transparency.
            row["mesh"]["colors"] = json!(mesh.vertex_colors().map(|cs| {
                cs.iter()
                    .map(|c| [c[0], c[1], c[2], 255 - c[3]])
                    .collect::<Vec<_>>()
            }));
        }
    }
    result
}
fn same_ngon(a: &Value, an: &Value, b: &Value, bn: &Value) -> bool {
    use serde_json::json;
    let objects = |mesh: &Value, ngon: &Value| {
        let mut points = ngon["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| mesh["vertices"][v.as_u64().unwrap() as usize].clone())
            .collect::<Vec<_>>();
        points.push(points[0].clone());
        let faces = ngon["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| mesh["faces"][i.as_u64().unwrap() as usize].clone())
            .collect::<Vec<_>>();
        json!([{"mesh":{"vertices":mesh["vertices"],"faces":faces}}, {"polyline":points}])
    };
    super::boolean::check_snapshot(objects(a, an), &objects(b, bn))["geometry"] == true
}

pub const GAPS: [&str; 12] = [
    "mesh_split_controls_face_contact_drop",
    "mesh_split_controls_mixed_cutter",
    "mesh_split_controls_three_cutters",
    "mesh_split_controls_two_cutters",
    "mesh_split_policy_brep_target_post",
    "mesh_split_policy_brep_target_pre",
    "mesh_split_policy_plane_three_point",
    "mesh_split_policy_plane_through_point",
    "mesh_split_policy_plane_world_yz",
    "mesh_split_selection_cross_both",
    "mesh_split_selection_curve_xz",
    "mesh_split_two_cutters",
];

pub const CHANNEL_GAPS: [&str; 3] = [
    "mesh_split_channels_seam_coplanar_post",
    "mesh_split_channels_seam_coplanar_pre",
    "mesh_split_channels_seam_folded",
];
