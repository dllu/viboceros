//! Native selection sets use imported source topology and immutable archives.
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use viboceros_geometry::{SubDEdgeChainType, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

#[test]
fn native_loop_selection_matches_owned_source_topology() {
    let mut count = 0;
    for bytes in [
        include_bytes!("../../../tools/rhino_oracle/observations/edge_loop_discovery.json.gz")
            .as_slice(),
        include_bytes!("../../../tools/rhino_oracle/observations/edge_loop_commands.json.gz")
            .as_slice(),
    ] {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .unwrap();
        let data: Value = serde_json::from_slice(&raw).unwrap();
        for row in data["results"].as_array().unwrap() {
            count += 1;
            let v = &row["value"];
            assert_eq!(v["source_unchanged"], true);
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("source.3dm");
            let raw = v["archive"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b.as_u64().unwrap() as u8)
                .collect::<Vec<_>>();
            std::fs::write(&path, raw).unwrap();
            let model = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
            let seed = v["seed"].as_u64().unwrap() as usize;
            let selected = v["after"]
                .get("ordinals")
                .map(|s| {
                    s.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as usize)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| {
                    v["after"]["components"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| {
                            c["index"].as_u64().unwrap() as usize
                                - usize::from(matches!(
                                    model.objects[0].geometry,
                                    ThreeDmGeometry::SubD(_)
                                ))
                        })
                        .collect()
                });
            let (ids, vertex_count) = match &model.objects[0].geometry {
                ThreeDmGeometry::SubD(s) => {
                    let archive = s.archive().to_vec();
                    let chain = s
                        .try_edge_loop(seed, SubDEdgeChainType::EqualEdgeTagAndOrdinary)
                        .unwrap();
                    assert_eq!(s.archive(), archive);
                    assert!(
                        s.try_edge_loop(
                            s.edges().len(),
                            SubDEdgeChainType::EqualEdgeTagAndOrdinary
                        )
                        .is_err()
                    );
                    (
                        chain.edges.iter().map(|e| e.index).collect::<Vec<_>>(),
                        chain.vertices.len(),
                    )
                }
                ThreeDmGeometry::Mesh(m) => {
                    let chain = m.try_edge_loop(seed).unwrap();
                    assert!(m.try_edge_loop(m.topology().edge_count()).is_err());
                    (chain.edges, chain.vertices.len())
                }
                _ => panic!(),
            };
            assert_eq!(vertex_count, ids.len() + 1);
            assert_eq!(
                ids.iter().copied().collect::<BTreeSet<_>>().len(),
                ids.len()
            );
            let mut sorted = ids;
            sorted.sort_unstable();
            assert_eq!(sorted, selected, "{}", row["id"]);
        }
    }
    assert_eq!(count, 28);
}
