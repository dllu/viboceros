//! Replay recorded public getter actions through the real staged profile editor.
#[path = "support/soft_crease.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::{Value, json};
use std::path::Path;
use viboceros_command::soft_crease::SoftCreaseSession;
use viboceros_document::Geometry;

#[test]
fn staged_profiles_match_native_chain_followup_and_anchor_workflows() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let mut failures = Vec::new();
    let mut count = 0;
    for family in [
        "soft_crease_chains",
        "soft_crease_followup",
        "soft_crease_anchors",
    ] {
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in native["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let result = std::panic::catch_unwind(|| {
                let value = &row["value"];
                let (mut document, objects, targets) = fixture::setup(root, family, id, value);
                let originals = document
                    .objects()
                    .map(|o| (o.id(), o.geometry_snapshot().clone()))
                    .collect::<Vec<_>>();
                let mut session = SoftCreaseSession::prepare(&document, targets).unwrap();
                let mut highlighted = None;
                let mut constant = false;
                let mut cancelled = false;
                for action in value["input"]["actions"].as_array().unwrap() {
                    if let Some(vertex) = action["vertex"].as_u64() {
                        let name = format!("Input {}", action["source"].as_u64().unwrap());
                        let oid = objects.iter().find(|(_, n, _)| *n == name).unwrap().0;
                        let chain = session
                            .chains()
                            .enumerate()
                            .find(|(_, (id, c))| {
                                *id == oid && c.vertices.contains(&(vertex as usize))
                            })
                            .unwrap()
                            .0;
                        highlighted = Some((chain, vertex as usize));
                    } else if let Some(weight) = action.as_f64() {
                        let result = match (constant, highlighted) {
                            (false, Some((chain, vertex))) => session.vertex(chain, vertex, weight),
                            (true, Some((chain, _))) => session.constant(Some(chain), weight),
                            (_, None) => session.constant(None, weight),
                        };
                        if !(0. ..=100.).contains(&weight) {
                            assert!(result.is_err());
                            continue;
                        }
                        result.unwrap();
                        highlighted = None;
                        constant = false;
                    } else {
                        match action.as_str().unwrap() {
                            "CreaseThisChain" => {
                                session.crease(Some(highlighted.unwrap().0)).unwrap();
                                highlighted = None;
                            }
                            "ConstantWeightChain" | "ConstantWeight" => constant = true,
                            "CreaseAll" | "Crease" => {
                                session.crease(None).unwrap();
                                highlighted = None;
                            }
                            "Enter" => {
                                highlighted = None;
                            }
                            "Cancel" => {
                                cancelled = true;
                                break;
                            }
                            other => panic!("unhandled {other}"),
                        }
                    }
                }
                for (oid, original) in &originals {
                    assert_eq!(document.object(*oid).unwrap().geometry_snapshot(), original);
                }
                if !cancelled {
                    session.commit(&mut document).unwrap();
                }
                for (oid, name, _) in &objects {
                    let expected = value["after"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|o| o["name"] == *name)
                        .unwrap();
                    let Geometry::SubD(actual) = document.object(*oid).unwrap().geometry() else {
                        unreachable!()
                    };
                    let check = subd_native::compare(actual, &expected["subd"]);
                    println!(
                        "SOFT_SESSION {}",
                        json!({"id":id,"family":family,"object":name,"check":check,"cancelled":cancelled,"source_unchanged":true})
                    );
                }
            });
            if result.is_err() {
                failures.push(format!("{family}/{id}"));
            }
            count += 1;
        }
    }
    assert_eq!(count, 48);
    assert!(failures.is_empty(), "profile differences: {failures:?}");
}
