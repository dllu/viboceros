use serde_json::Value;
use std::path::Path;
use viboceros_geometry::Tolerance;
use viboceros_io::{ThreeDmGeometry, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file};

#[test]
fn native_mesh_channels_survive_three_dm_round_trip() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_channel_assets.json"
    ))
    .unwrap();
    let observations: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_channels.json"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    for source in ["texture_cube", "ngon"] {
        let input = read_3dm_file(
            root.join(assets[source]["path"].as_str().unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let meshes = input
            .objects
            .iter()
            .filter_map(|o| match &o.geometry {
                ThreeDmGeometry::Mesh(m) => Some(m),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(meshes.len(), 1);
        let mesh = meshes[0];
        let expected = &observations["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == format!("{source}_identity"))
            .unwrap()["value"]["before"];
        assert_eq!(
            mesh.texture_coordinates().unwrap_or_default(),
            serde_json::from_value::<Vec<[f64; 2]>>(expected["uv"].clone()).unwrap()
        );
        assert_eq!(
            mesh.vertex_normals()
                .unwrap()
                .iter()
                .map(|n| n.to_array())
                .collect::<Vec<_>>(),
            serde_json::from_value::<Vec<[f64; 3]>>(expected["normals"].clone()).unwrap()
        );
        let model = ThreeDmModel::new(
            input.layers,
            vec![],
            vec![ThreeDmObject::new(ThreeDmGeometry::Mesh(mesh.clone()), 0)],
        );
        let path = dir.path().join(format!("{source}.3dm"));
        write_3dm_file(&path, &model).unwrap();
        let result = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.unsupported_object_count(), 0);
        assert_eq!(
            result.objects[0].geometry,
            ThreeDmGeometry::Mesh(mesh.clone())
        );
    }
}
