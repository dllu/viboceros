//! Native command admission counts independently check the public chain sorter.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_document::Geometry;

#[test]
fn selected_control_chains_match_native_counts_and_source_topology() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let mut workflows = 0;
    let mut sources = 0;
    for (family, count) in [
        ("soft_crease_chains", 26),
        ("soft_crease_followup", 18),
        ("soft_crease_anchors", 4),
    ] {
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        for row in native["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let value = &row["value"];
            let history = value["history"].as_str().unwrap();
            let line = history
                .lines()
                .find(|s| s.starts_with("Selected ") && s.contains("chain"))
                .unwrap();
            let expected = line
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let path = root.join(format!(
                "tools/rhino_oracle/fixtures/{family}/source/{id}.3dm"
            ));
            let (document, _, _, _) =
                viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
            let mut total = 0;
            let mut selected_count = 0;
            for object in document.objects() {
                let Geometry::SubD(source) = object.geometry() else {
                    continue;
                };
                let name = object.attributes().name().unwrap();
                let before = value["before"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| o["name"] == name)
                    .unwrap();
                let native_edges: Vec<[u32; 2]> =
                    serde_json::from_value(before["subd"]["edges"].clone()).unwrap();
                assert_eq!(source.edges(), native_edges, "{family}/{id}/{name}");
                let selected = before["selected_edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .filter(|&i| source.edge_face_counts()[i] == 2)
                    .collect::<Vec<_>>();
                let original = source.clone();
                let chains = source.try_edge_chains(&selected).unwrap();
                assert_eq!(source, &original);
                assert_eq!(
                    chains.iter().map(|c| c.edges.len()).sum::<usize>(),
                    selected.len()
                );
                total += chains.len();
                selected_count += selected.len();
                sources += 1;
            }
            assert_eq!(total, expected, "{family}/{id}");
            workflows += 1;
            println!(
                "SUBD_CHAINS_NATIVE {}",
                json!({"family":family,"id":id,"chains":total,
                "selected_edges":selected_count,"source_unchanged":true,"topology":true})
            );
        }
    }
    assert_eq!((workflows, sources), (48, 50));
}
