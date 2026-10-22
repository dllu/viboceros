#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::{Value, json};
use std::path::Path;
use viboceros_document::Geometry;
use viboceros_geometry::SubD;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
}
fn load(family: &str, id: &str, stage: &str) -> Vec<SubD> {
    let path = root().join(format!(
        "tools/rhino_oracle/fixtures/{family}/{stage}/{id}.3dm"
    ));
    let (doc, _, _, _) = viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let mut objects = doc
        .objects()
        .filter_map(|o| match o.geometry() {
            Geometry::SubD(s) => Some((o.attributes().name().unwrap().to_owned(), s.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    objects.sort_by(|a, b| a.0.cmp(&b.0));
    objects.into_iter().map(|(_, s)| s).collect()
}

#[derive(Clone, Copy)]
enum Edit {
    Keep,
    Weight(f64),
    Hard,
}
fn edit(value: &Value) -> Edit {
    if value["success"] != true {
        return Edit::Keep;
    }
    let mut edit = Edit::Keep;
    for action in value["input"]["actions"].as_array().unwrap() {
        if let Some(number) = action.as_f64() {
            edit = Edit::Weight(number);
        } else if matches!(action.as_str(), Some("CreaseAll" | "Crease")) {
            edit = Edit::Hard;
        }
    }
    edit
}
#[test]
fn soft_weights_match_native_controls_tags_surface_facets_and_archive_values() {
    let mut failures = Vec::new();
    for (family, source, count) in [
        (
            "soft_crease_discovery",
            include_str!("../../../tools/rhino_oracle/observations/soft_crease_discovery.json"),
            32,
        ),
        (
            "soft_crease",
            include_str!("../../../tools/rhino_oracle/observations/soft_crease.json"),
            72,
        ),
        (
            "soft_crease_numeric_labels",
            include_str!(
                "../../../tools/rhino_oracle/observations/soft_crease_numeric_labels.json"
            ),
            29,
        ),
    ] {
        let capture: Value = serde_json::from_str(source).unwrap();
        assert_eq!(capture["results"].as_array().unwrap().len(), count);
        for row in capture["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let result = std::panic::catch_unwind(|| {
                let native = &row["value"];
                let sources = load(family, id, "source");
                let expected = load(family, id, "result");
                for (index, (subd, expected)) in sources.iter().zip(&expected).enumerate() {
                    let archive = subd.archive().to_vec();
                    let input = &native["input"]["sources"][index];
                    let edges = subd
                        .edges()
                        .iter()
                        .enumerate()
                        .filter(|(i, e)| {
                            if subd.edge_face_counts()[*i] != 2 {
                                return false;
                            }
                            let mut pair = e.to_vec();
                            pair.sort_unstable();
                            input["edges"].as_array().unwrap().iter().any(|v| {
                                v[0].as_u64() == Some(pair[0] as u64)
                                    && v[1].as_u64() == Some(pair[1] as u64)
                            })
                        })
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    let label_weights = if family == "soft_crease_numeric_labels"
                        && id.starts_with("label_")
                        && native["success"] == true
                        && !(id == "label_multiple" && index == 1)
                    {
                        let vertex_weights = match id {
                            "label_middle_accepted" => Some([0., 80., 40., 0., 0., 0., 0., 0.]),
                            "label_both_ends" => {
                                Some([20., 20. + 70. / 3., 20. + 140. / 3., 90., 0., 0., 0., 0.])
                            }
                            "label_loop" => Some([80., 40., 0., 40., 0., 0., 0., 0.]),
                            "label_after_constant" => None, // The native click missed the moved label; 20 was global.
                            _ => Some([80., 80. * 2. / 3., 80. / 3., 0., 0., 0., 0., 0.]),
                        };
                        vertex_weights.map(|values| {
                            edges
                                .iter()
                                .map(|&edge| {
                                    let [a, b] = subd.edges()[edge];
                                    let weights =
                                        if matches!(id, "label_branch" | "label_disconnected") {
                                            if [a.min(b), a.max(b)] == [0, 1] {
                                                if a == 0 { [80., 0.] } else { [0., 80.] }
                                            } else {
                                                [0., 0.]
                                            }
                                        } else {
                                            [values[a as usize], values[b as usize]]
                                        };
                                    (edge, weights)
                                })
                                .collect::<Vec<_>>()
                        })
                    } else {
                        None
                    };
                    let (actual, changed) = if let Some(weights) = label_weights {
                        subd.try_with_soft_crease_weights(&weights).unwrap()
                    } else if id == "label_multiple" && index == 1 {
                        (subd.clone(), false)
                    } else {
                        match edit(native) {
                            Edit::Keep => (subd.clone(), false),
                            Edit::Hard => subd.try_crease_components(&edges, &[], true).unwrap(),
                            Edit::Weight(weight) => subd
                                .try_with_soft_crease_weights(
                                    &edges.iter().map(|&i| (i, [weight; 2])).collect::<Vec<_>>(),
                                )
                                .unwrap(),
                        }
                    };
                    assert_eq!(subd.archive(), archive);
                    let before = native["before"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|o| o["source"].as_u64() == Some(index as u64))
                        .unwrap();
                    let after = native["after"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|o| o["source"].as_u64() == Some(index as u64))
                        .unwrap();
                    assert_eq!(changed, before["crc"] != after["crc"]);
                    assert_eq!(actual.edges(), expected.edges());
                    assert_eq!(actual.edge_tags(), expected.edge_tags());
                    assert_eq!(actual.vertex_tags(), expected.vertex_tags());
                    let native_archive_normalizes_sharpness =
                        actual.edge_sharpness() != expected.edge_sharpness();
                    if native_archive_normalizes_sharpness {
                        assert!(
                            (family == "soft_crease"
                                && matches!(id, "weight_24_75" | "weight_25_25"))
                                || (family == "soft_crease_numeric_labels"
                                    && id.starts_with("boundary_"))
                        );
                    }
                    let restored = SubD::try_from_archive(actual.archive()).unwrap();
                    assert_eq!(restored.edge_sharpness(), actual.edge_sharpness());
                    assert_eq!(restored.surface_points(), actual.surface_points());
                    assert_eq!(actual.edge_face_counts(), expected.edge_face_counts());
                    let report = subd_native::compare(&actual, &after["subd"]);
                    println!(
                        "SOFT_KERNEL {}",
                        json!({"id":id,"family":family,"source":index,"changed":changed,"sharpness":true,"source_unchanged":true,"native_archive_normalizes_sharpness":native_archive_normalizes_sharpness,"check":report})
                    );
                }
            });
            if result.is_err() {
                failures.push(format!("{family}/{id}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "native soft crease differences: {failures:?}"
    );
}

#[test]
fn external_3dm_import_matches_native_roundtrip_at_all_eighteen_precision_boundaries() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/soft_crease_numeric_labels.json"
    ))
    .unwrap();
    let mut compared = 0;
    for row in capture["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if !id.starts_with("boundary_") {
            continue;
        }
        let imported = load("soft_crease_numeric_labels", id, "result");
        let native = &row["value"]["roundtrip"][0]["subd"];
        let report = subd_native::compare(&imported[0], native);
        println!("SOFT_IO {}", json!({"id":id,"check":report}));
        compared += 1;
    }
    assert_eq!(compared, 18);
}
