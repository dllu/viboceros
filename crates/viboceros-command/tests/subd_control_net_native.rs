//! Public native control meshes include n-gons, crease seams and texture corners.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_document::Geometry;
use viboceros_geometry::SubDControlNetPriority;

#[test]
fn control_mesh_priorities_preserve_native_topology_normals_and_texture_coordinates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let mut count = 0;
    for family in [
        "control_polygon",
        "control_polygon_options",
        "control_polygon_priority",
    ] {
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in native["results"].as_array().unwrap() {
            let value = &row["value"];
            if value["success"] != true || value["history"].as_str().unwrap().contains("Unknown") {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            let path = root.join(format!(
                "tools/rhino_oracle/fixtures/{family}/source/{id}.3dm"
            ));
            let (document, _, _, _) =
                viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
            for source in document.objects() {
                let Geometry::SubD(subd) = source.geometry() else {
                    continue;
                };
                let before = subd.clone();
                let expected = value["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| {
                        o["source"].is_null() && o["name"].as_str() == source.attributes().name()
                    })
                    .unwrap();
                let priority = if value["input"]["priority"] == "TextureCoordinates" {
                    SubDControlNetPriority::TextureCoordinates
                } else {
                    SubDControlNetPriority::Geometry
                };
                let result = subd.control_net_mesh(priority).unwrap();
                let mesh = &result;
                let expected = &expected["mesh"];
                let points: Vec<[f64; 3]> =
                    serde_json::from_value(expected["vertices"].clone()).unwrap();
                assert_eq!(mesh.vertices().len(), points.len(), "{family}/{id}");
                let error = mesh
                    .vertices()
                    .iter()
                    .zip(&points)
                    .map(|(p, q)| {
                        p.to_array()
                            .into_iter()
                            .zip(q)
                            .map(|(a, b)| a - b)
                            .fold(0_f64, f64::hypot)
                    })
                    .fold(0_f64, f64::max);
                assert!(error <= 1e-6, "{family}/{id}: {error}");
                let faces: Vec<Vec<u32>> =
                    serde_json::from_value(expected["faces"].clone()).unwrap();
                assert_eq!(
                    mesh.faces()
                        .iter()
                        .map(|f| f.indices().to_vec())
                        .collect::<Vec<_>>(),
                    faces,
                    "{family}/{id}"
                );
                let ngons = expected["ngons"].as_array().unwrap();
                assert_eq!(mesh.ngons().len(), ngons.len(), "{family}/{id}");
                for (a, b) in mesh.ngons().iter().zip(ngons) {
                    assert_eq!(
                        a.vertices(),
                        serde_json::from_value::<Vec<u32>>(b["vertices"].clone()).unwrap()
                    );
                    assert_eq!(
                        a.faces(),
                        serde_json::from_value::<Vec<u32>>(b["faces"].clone()).unwrap()
                    );
                }
                let normals: Vec<[f64; 3]> =
                    serde_json::from_value(expected["normals"].clone()).unwrap();
                assert_eq!(
                    result
                        .vertex_normals()
                        .unwrap_or_default()
                        .iter()
                        .map(|v| v.to_array())
                        .collect::<Vec<_>>(),
                    normals,
                    "{family}/{id}"
                );
                if let Some(tex) = expected.get("texture_coordinates") {
                    assert_eq!(
                        result.texture_coordinates().unwrap_or_default(),
                        serde_json::from_value::<Vec<[f64; 2]>>(tex.clone()).unwrap(),
                        "{family}/{id}"
                    );
                } else {
                    assert!(result.texture_coordinates().is_none());
                }
                assert_eq!(subd, &before);
                if priority == SubDControlNetPriority::Geometry {
                    assert_eq!(subd.control_mesh().unwrap(), result);
                }
                println!(
                    "CONTROL_NET_NATIVE {}",
                    json!({"family":family,"id":id,"source":source.attributes().name(),"vertices":mesh.vertices().len(),"faces":mesh.face_count(),"ngons":mesh.ngons().len(),"textures":result.texture_coordinates().unwrap_or_default().len(),"normals":result.vertex_normals().unwrap_or_default().len(),"maximum_point_error":error,"source_unchanged":true,"vertex_attributes_exact":true,"topology_exact":true,"qualified":true})
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 21);
}
