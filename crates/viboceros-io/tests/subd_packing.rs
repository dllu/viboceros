//! Retained native pack charts survive 3DM import and immutable metadata queries.
use serde_json::Value;
use std::{io::Read, sync::OnceLock};
use viboceros_geometry::{SubDNurbsContinuity, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

fn captures() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut results = Vec::new();
        for bytes in [
            include_bytes!("../../../tools/rhino_oracle/observations/subd_packing_modes.json.gz")
                .as_slice(),
            include_bytes!("../../../tools/rhino_oracle/observations/subd_packing.json.gz")
                .as_slice(),
            include_bytes!(
                "../../../tools/rhino_oracle/observations/subd_packing_holdouts.json.gz"
            )
            .as_slice(),
        ] {
            let mut text = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .read_to_end(&mut text)
                .unwrap();
            let mut data: Value = serde_json::from_slice(&text).unwrap();
            results.append(data["results"].as_array_mut().unwrap());
        }
        serde_json::json!({"results":results})
    })
}

#[test]
fn imported_native_face_pack_ids_charts_and_sources_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    for row in captures()["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if !id.ends_with("_0") {
            continue;
        }
        let value = &row["value"];
        let archive = value["archive"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap() as u8)
            .collect::<Vec<_>>();
        let path = temp.path().join(format!("{id}.3dm"));
        std::fs::write(&path, archive).unwrap();
        let model = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
        assert_eq!(model.objects.len(), 1);
        let ThreeDmGeometry::SubD(subd) = &model.objects[0].geometry else {
            panic!("{id}: expected SubD");
        };
        let before = subd.archive().to_vec();
        let records = viboceros_opennurbs::subd_face_packs(subd.archive()).unwrap();
        assert_eq!(records.len(), subd.faces().len());
        let mut offset = 0;
        for (record, face) in records.iter().zip(subd.faces()) {
            let record = record.unwrap();
            assert_eq!(
                record.id as u64,
                value["unpacked"]["pack_ids"][offset].as_u64().unwrap(),
                "{id}"
            );
            assert!(record.quarter_turns < 4);
            offset += if face.len() == 4 { 1 } else { face.len() };
        }
        let brep = subd
            .to_packed_nurbs_brep(SubDNurbsContinuity::G0, Tolerance::DEFAULT)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(
            brep.faces().len(),
            value["output"]["geometry"]["faces"]
                .as_array()
                .unwrap()
                .len(),
            "{id}"
        );
        assert_eq!(
            brep.edges().len(),
            value["output"]["geometry"]["edges"]
                .as_array()
                .unwrap()
                .len(),
            "{id}"
        );
        assert_eq!(
            brep.vertices().len(),
            value["output"]["geometry"]["vertices"]
                .as_array()
                .unwrap()
                .len(),
            "{id}"
        );
        assert_eq!(
            brep.is_solid(),
            value["output"]["solid"].as_bool().unwrap(),
            "{id}"
        );
        if id == "grid_0" {
            let edge = subd
                .edges()
                .iter()
                .position(|&[a, b]| [a.min(b), a.max(b)] == [7, 12])
                .unwrap();
            let changed = subd.try_with_edge_tags(&[(edge, 2)]).unwrap();
            assert_eq!(
                changed
                    .to_packed_nurbs_patches(SubDNurbsContinuity::G0)
                    .unwrap()
                    .len(),
                6,
                "a crease edit must invalidate the retained rectangular pack"
            );
        }
        assert_eq!(before, subd.archive());
    }
}
