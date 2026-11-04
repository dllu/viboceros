//! Actual PackSubDFaces archives: partitions and unchanged control geometry.
use serde_json::Value;
use std::io::Read;
use viboceros_geometry::{SubD, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};
fn source(row: &Value, path: &std::path::Path) -> SubD {
    let bytes = row["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    std::fs::write(path, bytes).unwrap();
    let model = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    let ThreeDmGeometry::SubD(s) = &model.objects[0].geometry else {
        panic!()
    };
    s.clone()
}
#[test]
fn native_pack_command_partitions_and_control_geometry() {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!("../../../tools/rhino_oracle/observations/subd_pack_command.json.gz")
            .as_slice(),
    )
    .read_to_end(&mut raw)
    .unwrap();
    let data: Value = serde_json::from_slice(&raw).unwrap();
    let temp = tempfile::tempdir().unwrap();
    for row in data["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if id == "cancel" {
            continue;
        }
        for step in row["value"]["steps"].as_array().unwrap() {
            for before in step["before"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "SubD")
            {
                let after = step["settled"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["source"] == before["source"])
                    .unwrap();
                let input = source(before, &temp.path().join("before.3dm"));
                let native = source(after, &temp.path().join("after.3dm"));
                let expected = viboceros_opennurbs::subd_face_packs(native.archive()).unwrap();
                let output = input.try_repacked_faces().unwrap();
                let packs = viboceros_opennurbs::subd_face_packs(output.archive()).unwrap();
                assert_eq!(
                    packs.iter().map(|p| p.unwrap().id).collect::<Vec<_>>(),
                    expected.iter().map(|p| p.unwrap().id).collect::<Vec<_>>(),
                    "{id}"
                );
                assert_eq!(input.control_points(), output.control_points());
                assert_eq!(input.faces(), output.faces());
                assert_eq!(input.edges(), output.edges());
                assert_eq!(input.vertex_tags(), output.vertex_tags());
                assert_eq!(input.edge_tags(), output.edge_tags());
                assert_eq!(input.edge_sharpness(), output.edge_sharpness());
                assert_eq!(native.control_points(), output.control_points());
                assert_eq!(input.surface_points(), output.surface_points());
                let mut model =
                    read_3dm_file(temp.path().join("after.3dm"), Tolerance::DEFAULT).unwrap();
                model.objects[0].geometry = ThreeDmGeometry::SubD(output.clone());
                let saved = temp.path().join("repacked.3dm");
                viboceros_io::write_3dm_file(&saved, &model).unwrap();
                let restored = read_3dm_file(saved, Tolerance::DEFAULT).unwrap();
                let ThreeDmGeometry::SubD(restored) = &restored.objects[0].geometry else {
                    panic!()
                };
                assert_eq!(restored.face_packs().unwrap(), packs);
                assert_eq!(restored.control_points(), output.control_points());
                assert_eq!(
                    packs,
                    viboceros_opennurbs::subd_face_packs(
                        output.try_repacked_faces().unwrap().archive()
                    )
                    .unwrap()
                );
            }
        }
    }
}

#[test]
fn repacking_matches_independent_sdk_partitions_and_repairs_stale_creases() {
    let temp = tempfile::tempdir().unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for bytes in [
        include_bytes!("../../../tools/rhino_oracle/observations/subd_packing.json.gz").as_slice(),
        include_bytes!("../../../tools/rhino_oracle/observations/subd_packing_holdouts.json.gz")
            .as_slice(),
        include_bytes!("../../../tools/rhino_oracle/observations/subd_packing_modes.json.gz")
            .as_slice(),
    ] {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .unwrap();
        let data: Value = serde_json::from_slice(&raw).unwrap();
        for row in data["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap().rsplit_once('_').unwrap().0;
            if !seen.insert(id.to_owned()) {
                continue;
            }
            let input = source(&row["value"], &temp.path().join("source.3dm"));
            let output = input.try_repacked_faces().unwrap();
            assert_eq!(
                input
                    .face_packs()
                    .unwrap()
                    .iter()
                    .map(|p| p.unwrap().id)
                    .collect::<Vec<_>>(),
                output
                    .face_packs()
                    .unwrap()
                    .iter()
                    .map(|p| p.unwrap().id)
                    .collect::<Vec<_>>(),
                "{id}"
            );
            if id == "grid" {
                let edits = input
                    .edges()
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        [[2, 7], [7, 12], [12, 17]].contains(&[e[0].min(e[1]), e[0].max(e[1])])
                    })
                    .map(|(i, _)| (i, 2))
                    .collect::<Vec<_>>();
                assert_eq!(edits.len(), 3);
                let stale = input.try_with_edge_tags(&edits).unwrap();
                assert!(
                    stale
                        .face_packs()
                        .unwrap()
                        .iter()
                        .all(|p| p.unwrap().id == 1)
                );
                let fixed = stale.try_repacked_faces().unwrap();
                assert_eq!(
                    fixed
                        .face_packs()
                        .unwrap()
                        .iter()
                        .map(|p| p.unwrap().id)
                        .collect::<Vec<_>>(),
                    vec![1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2]
                );
                assert_eq!(fixed.control_points(), stale.control_points());
                assert_eq!(fixed.surface_points(), stale.surface_points());
                assert_eq!(fixed.edge_tags(), stale.edge_tags());
                let refined = fixed.subdivided(1).unwrap();
                assert_eq!(
                    refined
                        .face_packs()
                        .unwrap()
                        .iter()
                        .map(|p| p.unwrap().id)
                        .collect::<Vec<_>>(),
                    fixed
                        .face_packs()
                        .unwrap()
                        .iter()
                        .flat_map(|p| [p.unwrap().id; 4])
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    assert_eq!(seen.len(), 36);
}
