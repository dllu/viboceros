//! Compare actual Rhino-saved custom face data with independent kernel edits.
use super::support;
use serde_json::{Value, json};
use viboceros_geometry::{SubD, SubDMergeComponent};

pub(super) fn value(s: &SubD) -> Value {
    json!({"face_ids":s.face_ids(),"texture_type":s.texture_coordinate_type() as u32,
        "properties":s.face_properties().iter().zip(s.face_texture_points()).map(|(p,t)|json!({
            "color":p.color,"material_channel":p.material_channel,"uvw":t})).collect::<Vec<_>>()})
}
fn definition(rows: &Value) -> &Value {
    &rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap()["subd"]
}
pub(super) fn compare(
    actual: &SubD,
    expected: &SubD,
    rows: &Value,
    name: &str,
    differences: &mut Vec<String>,
) {
    let control = support::actual_value(actual);
    for source in [
        &support::actual_value(expected),
        &support::native_value(definition(rows)),
    ] {
        support::differences_between(&control, source, name, differences);
    }
    if value(actual) != value(expected) {
        differences.push(format!(
            "{name}: custom face data differs\nactual {}\nexpected {}",
            value(actual),
            value(expected)
        ));
    }
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap();
    assert_eq!(
        actual.texture_coordinate_type() as u32,
        row["texture_coordinate_type"].as_u64().unwrap() as u32,
        "{name}"
    );
}
pub(super) fn audit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (family, recipe, assets, count) in [
        (
            "subd_face_custom",
            "subd_face_custom_recipes",
            "subd_face_custom_assets",
            38,
        ),
        (
            "subd_face_custom_holdouts",
            "subd_face_custom_holdout_recipes",
            "subd_face_custom_holdout_assets",
            14,
        ),
    ] {
        let record: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let recipes: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{recipe}.json"))).unwrap(),
        )
        .unwrap();
        let assets: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{assets}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(record["results"].as_array().unwrap().len(), count);
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let mut source = support::read(&v["source_3dm"]);
            let p = source
                .face_properties()
                .iter()
                .map(|p| json!({"material_channel":p.material_channel,"color":p.color}))
                .collect::<Vec<_>>();
            assert_eq!(
                json!(p),
                recipes[name]["source"]["properties"],
                "{name}: Native seed import changed properties"
            );
            assert_eq!(
                json!(source.face_texture_points()),
                recipes[name]["source"]["textures"],
                "{name}: Native seed import changed UVWs"
            );
            let mut authored = support::read(&assets[name]["data"]);
            if let Some(rows) = recipes[name].get("transform") {
                let m: [[f64; 4]; 4] = serde_json::from_value(rows.clone()).unwrap();
                let transform = viboceros_geometry::AffineTransform3::try_new(
                    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j])),
                    viboceros_geometry::Vector3::try_new(m[0][3], m[1][3], m[2][3]).unwrap(),
                )
                .unwrap();
                authored = authored.transformed(transform).unwrap();
            }
            let mut differences = Vec::new();
            support::differences_between(
                &support::actual_value(&authored),
                &support::actual_value(&source),
                &format!("{name}/seed/transform"),
                &mut differences,
            );
            assert_eq!(
                authored.face_properties(),
                source.face_properties(),
                "{name} seed properties"
            );
            assert_eq!(
                authored.face_texture_points(),
                source.face_texture_points(),
                "{name} seed UVWs"
            );
            // Rhino's document/evaluation preparation activates Packed mapping
            // on an Unset untextured seed. No operation-result comparison uses
            // this import-only allowance; custom UVW modes stay exact.
            use viboceros_geometry::SubDTextureCoordinateType as Mapping;
            let default_mapping_activated = authored.texture_coordinate_type() == Mapping::Unset
                && source.texture_coordinate_type() == Mapping::Packed
                && authored.face_texture_points().iter().all(Option::is_none);
            assert!(
                authored.texture_coordinate_type() == source.texture_coordinate_type()
                    || default_mapping_activated,
                "{name}: unexpected seed mapping change"
            );
            let before = source.clone();
            let mut stages = Vec::new();
            for (i, step) in v["steps"].as_array().unwrap().iter().enumerate() {
                let picks = step["selected"]
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
                source = source.merge_face_components(&picks).unwrap().subd;
                let native = support::read(&step["after_3dm"]);
                compare(
                    &source,
                    &native,
                    &step["after"],
                    &format!("{name}/{i}"),
                    &mut differences,
                );
                let restored = SubD::try_from_archive(source.archive()).unwrap();
                assert_eq!(value(&restored), value(&source));
                stages.push(json!({"native":value(&native),"actual":value(&source)}));
            }
            if !differences.is_empty() {
                failures.push((name.to_owned(), differences.clone()));
            }
            report.push(
            json!({"family":family,"id":name,"before":value(&before),"seed_default_mapping_activated":default_mapping_activated,"steps":stages,"differences":differences}),
        );
        }
    }
    assert_eq!(report.len(), 52);
    if let Ok(path) = std::env::var("VIBOCEROS_SUBD_FACE_CUSTOM_AUDIT_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    assert!(
        failures.is_empty(),
        "Native custom face data differs: {failures:?}"
    );
}
