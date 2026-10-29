use super::support;
use serde_json::{Value, json};
use viboceros_geometry::{SubD, SubDMergeComponent};

fn definition(rows: &Value) -> &Value {
    &rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap()["subd"]
}
fn check(s: &SubD, rows: &Value, name: &str) {
    let d = definition(rows);
    support::compare(&support::actual_value(s), &support::native_value(d), name);
    let mut expected = d["face_ids"]
        .as_array()
        .unwrap()
        .iter()
        .zip(d["face_colors"].as_array().unwrap())
        .map(|(id, color)| {
            let color = if color.is_null() {
                Value::Null
            } else {
                json!([
                    color[0],
                    color[1],
                    color[2],
                    255 - color[3].as_u64().unwrap()
                ])
            };
            (id.as_u64().unwrap(), color)
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|(id, _)| *id);
    let actual = s
        .face_ids()
        .iter()
        .zip(s.face_properties())
        .map(|(id, p)| (*id as u64, json!(p.color)))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{name} face colors");
    assert!(s.face_properties().iter().all(|p| p.material_channel == 0));
}
pub(super) fn audit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/subd_merge_properties.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["results"].as_array().unwrap().len(), 20);
    for row in record["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let mut source = support::read(&v["source_3dm"]);
        check(&source, &v["before"], name);
        // The wrapper initializes SMLib first when enabled. Exercise nonzero
        // material channels through the same linked archive/setter path too.
        let property = viboceros_geometry::SubDFaceProperties {
            color: Some([11, 37, 191, 83]),
            material_channel: u16::MAX,
        };
        let painted = source.try_with_face_properties(&[(0, property)]).unwrap();
        assert_eq!(painted.face_properties()[0], property);
        let restored = SubD::try_from_archive(painted.archive()).unwrap();
        assert_eq!(restored.face_properties(), painted.face_properties());
        assert_eq!(restored.face_ids(), source.face_ids());
        assert_eq!(restored.control_points(), source.control_points());
        for step in v["steps"].as_array().unwrap() {
            assert_eq!(step["command_result"], "Success");
            let selected = step["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    let id = c["id"].as_u64().unwrap() as u32;
                    match c["kind"].as_str().unwrap() {
                        "face" => SubDMergeComponent::Face(
                            source.face_ids().iter().position(|&x| x == id).unwrap(),
                        ),
                        "vertex" => SubDMergeComponent::Vertex(
                            source.vertex_ids().iter().position(|&x| x == id).unwrap(),
                        ),
                        "edge" => SubDMergeComponent::Edge(
                            source.edges()
                                [source.edge_ids().iter().position(|&x| x == id).unwrap()]
                            .map(|i| i as usize),
                        ),
                        _ => unreachable!(),
                    }
                })
                .collect::<Vec<_>>();
            let before = source.clone();
            let out = source.merge_face_components(&selected).unwrap();
            assert_eq!(source, before);
            source = out.subd;
            check(&source, &step["after"], name);
            let native = support::read(&step["after_3dm"]);
            check(&native, &step["after"], name);
            assert_eq!(source.face_properties(), native.face_properties());
            let restored = SubD::try_from_archive(source.archive()).unwrap();
            check(&restored, &step["after"], name);
        }
    }
}
