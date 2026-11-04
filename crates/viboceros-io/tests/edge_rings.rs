//! Raw native topology separates traversal checks from independent construction.
use serde_json::Value;
use std::io::Read;
use viboceros_geometry::Tolerance;
use viboceros_io::{ThreeDmGeometry, read_3dm_file};
#[test]
fn native_single_seed_ring_sets_match_imported_mesh_and_subd_sources() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "edge_ring_discovery",
        "edge_ring_commands",
        "edge_ring_ngons",
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let bytes = std::fs::read(base.join(format!("observations/{family}.json.gz"))).unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut raw)
            .unwrap();
        let data: Value = serde_json::from_slice(&raw).unwrap();
        for (row, op) in data["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(request["operations"].as_array().unwrap())
        {
            if !op["initial"].as_array().unwrap().is_empty()
                || op["steps"].as_array().unwrap().len() != 1
                || op["steps"][0]["modifiers"] != "plain"
                || op["finish"] != "Enter"
                || op["whole"] == true
                || !op["component"].is_null()
            {
                continue;
            }
            count += 1;
            let v = &row["value"];
            let temp = tempfile::tempdir().unwrap();
            let file = temp.path().join("source.3dm");
            std::fs::write(
                &file,
                v["archive"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|b| b.as_u64().unwrap() as u8)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            let source = read_3dm_file(file, Tolerance::DEFAULT).unwrap();
            let seed = v["edges"]
                .as_array()
                .unwrap()
                .iter()
                .position(|e| e == &op["pair"] || (e[0] == op["pair"][1] && e[1] == op["pair"][0]))
                .unwrap();
            let ring = match &source.objects[0].geometry {
                ThreeDmGeometry::Mesh(m) => m.try_edge_ring(seed).unwrap(),
                ThreeDmGeometry::SubD(s) => {
                    let archive = s.archive().to_vec();
                    assert_eq!(
                        serde_json::json!(s.component_ids().unwrap().edges),
                        v["native"]
                    );
                    assert_eq!(s.archive(), archive);
                    s.try_edge_ring(seed).unwrap()
                }
                _ => panic!(),
            };
            assert_eq!(
                ring.faces.len() + usize::from(!ring.is_closed() && !ring.edges.is_empty()),
                ring.edges.len()
            );
            let mut ids = ring.edges;
            ids.sort_unstable();
            assert_eq!(
                serde_json::json!(ids),
                v["after"]["ordinals"],
                "{}",
                row["id"]
            );
        }
    }
    assert_eq!(count, 43);
}
