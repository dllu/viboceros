use super::*;

#[test]
fn remove_from_group_modes_match_recorded_rhino() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/remove_from_group.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/remove_from_group.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected = observed["results"].as_array().unwrap();
    assert_eq!(expected.len(), 4);
    assert_eq!(actual.results.len(), expected.len());
    for (actual, expected) in actual.results.iter().zip(expected) {
        assert_eq!(actual.id, expected["id"]);
        assert_eq!(actual.value, expected["value"], "{}", actual.id);
    }
}

#[test]
fn add_to_group_named_targets_match_recorded_rhino() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/add_to_group.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/add_to_group.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected = observed["results"].as_array().unwrap();
    assert_eq!(expected.len(), 3);
    assert_eq!(actual.results.len(), expected.len());
    for (actual, expected) in actual.results.iter().zip(expected) {
        assert_eq!(actual.id, expected["id"]);
        assert_eq!(actual.value, expected["value"], "{}", actual.id);
    }
}

const FIXTURE: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/group_memberships.json");

#[test]
fn previous_selection_sequences_match_recorded_rhino_sets() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/selection_recall.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/selection_recall.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(rows.len(), 84);
    assert_eq!(response.results.len(), rows.len());
    for (actual, expected) in response.results.iter().zip(rows) {
        assert_eq!(actual.id, expected["id"].as_str().unwrap());
        let selected = actual.value["states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|state| {
                state["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|object| object["selected"] == true)
                    .map(|object| object["source"].clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(selected), expected["selected"], "{}", actual.id);
    }
}

#[test]
fn permanent_group_steps_keep_order_and_exact_reverse_membership_records() {
    let request: ProbeRequest = serde_json::from_str(FIXTURE).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 56);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::GroupMemberships { fixture: f, id } = operation else {
            panic!("group fixture")
        };
        let states = result.value["states"].as_array().unwrap();
        assert_eq!(states.len(), f.steps.len() + 1, "{id}");
        for state in states {
            let mut reverse = BTreeMap::<String, Vec<usize>>::new();
            for group in state["groups"].as_array().unwrap() {
                assert!(
                    reverse
                        .insert(group["name"].as_str().unwrap().to_owned(), Vec::new())
                        .is_none()
                );
            }
            for object in state["objects"].as_array().unwrap() {
                let source = object["source"].as_u64().unwrap() as usize;
                assert!(source < f.sources.len());
                let groups = object["groups"].as_array().unwrap();
                let mut seen = BTreeSet::new();
                for group in groups {
                    let name = group.as_str().unwrap();
                    assert!(seen.insert(name));
                    reverse
                        .get_mut(name)
                        .expect("membership definition exists")
                        .push(source);
                }
                assert!(!object["points"].as_array().unwrap().is_empty());
            }
            for group in state["groups"].as_array().unwrap() {
                let members = reverse.get_mut(group["name"].as_str().unwrap()).unwrap();
                members.sort_unstable();
                assert_eq!(group["members"], json!(members), "{id}");
            }
        }
        for (i, step) in f.steps.iter().enumerate() {
            if let Step::Set { object, groups } = step {
                let record = states[i + 1]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["source"] == *object && r["retained"] == true)
                    .unwrap();
                assert_eq!(
                    record["groups"],
                    json!(
                        groups
                            .iter()
                            .map(|g| format!("Group-{g}"))
                            .collect::<Vec<_>>()
                    ),
                    "{id}"
                );
            }
        }
    }
}

#[test]
fn observed_copy_and_explode_selection_does_not_expand_group_peers() {
    let request: ProbeRequest = serde_json::from_str(FIXTURE).unwrap();
    let response = run_request(&request).unwrap();
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::GroupMemberships { fixture: f, .. } = operation else {
            unreachable!()
        };
        let states = result.value["states"].as_array().unwrap();
        for (i, step) in f.steps.iter().enumerate() {
            if matches!(
                step,
                Step::Command {
                    name: ObjectCommand::Copy
                        | ObjectCommand::Array
                        | ObjectCommand::ArrayLinear
                        | ObjectCommand::ArrayPolar
                }
            ) {
                let selected = |state: &Value| {
                    state["objects"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|r| r["selected"] == true)
                        .map(|r| {
                            (
                                r["source"].as_u64().unwrap(),
                                r["retained"].as_bool().unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    selected(&states[i]),
                    selected(&states[i + 1]),
                    "{}",
                    result.id
                );
            }
        }
        if result.id == "ordered-explode" {
            let objects = states.last().unwrap()["objects"].as_array().unwrap();
            assert_eq!(objects.len(), 3);
            for object in objects {
                assert_eq!(object["selected"], object["source"] == 0);
            }
        }
    }
}

#[test]
fn invalid_fixture_sizes_indices_duplicates_and_deleted_groups_are_rejected() {
    let valid = json!({"sources":[{"type":"point","point":[0,0,0]}],"groups":[[0]],"steps":[]});
    for (key, value) in [
        ("sources", json!([])),
        ("groups", json!([[0, 0]])),
        ("groups", json!([[1]])),
        ("steps", json!([{"kind":"select","objects":[0,0]}])),
        ("steps", json!([{"kind":"set","object":0,"groups":[0,0]}])),
        ("steps", json!([{"kind":"add","group":1,"objects":[0]}])),
        ("steps", json!([{"kind":"set","object":1,"groups":[]}])),
    ] {
        let mut candidate = valid.clone();
        candidate[key] = value;
        let f: GroupMembershipFixture = serde_json::from_value(candidate).unwrap();
        assert!(run(&f, Tolerance::DEFAULT).is_err());
    }
    for step in [
        json!({"kind":"set","object":0,"groups":[0]}),
        json!({"kind":"add","group":0,"objects":[0]}),
        json!({"kind":"delete_group","group":0}),
    ] {
        let mut candidate = valid.clone();
        candidate["steps"] = json!([{"kind":"delete_group","group":0},step]);
        let f: GroupMembershipFixture = serde_json::from_value(candidate).unwrap();
        assert!(run(&f, Tolerance::DEFAULT).is_err());
    }
    for key in ["sources", "groups", "steps"] {
        let mut candidate = valid.clone();
        candidate[key] = match key {
            "sources" => json!(vec![valid["sources"][0].clone(); 33]),
            "groups" => json!(vec![json!([]); 17]),
            _ => json!(vec![json!({"kind":"select","objects":[]}); 65]),
        };
        let f: GroupMembershipFixture = serde_json::from_value(candidate).unwrap();
        assert!(run(&f, Tolerance::DEFAULT).is_err());
    }
}

#[test]
fn automatic_name_normalization_preserves_style_and_relative_numbering() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    document.set_object_names([(id, Some("0".into()))]).unwrap();
    for name in ["Group107", "Group2", "Group-0", "Group-0 copy"] {
        document.add_group(Some(name.into()), [id]).unwrap();
    }
    let value = record(&document, &[id], &BTreeMap::from([(id, 0)])).unwrap();
    assert_eq!(
        value["objects"][0]["groups"],
        json!(["CopyGroup-1", "CopyGroup-0", "Group-0", "Group-0 copy"])
    );
}

#[test]
fn bezier_pieces_keep_exact_loci_with_unit_domains_and_fresh_attributes() {
    let mut request: ProbeRequest = serde_json::from_str(FIXTURE).unwrap();
    request.operations.retain(|o| o.id() == "ordered-bezier");
    let Operation::GroupMemberships { fixture: f, .. } = &request.operations[0] else {
        panic!("group fixture")
    };
    let tolerance = request.tolerance.geometry().unwrap();
    let Geometry::NurbsCurve(curve) = f.sources[0].geometry(tolerance).unwrap() else {
        panic!("curve")
    };
    let response = run_request(&request).unwrap();
    let states = response.results[0].value["states"].as_array().unwrap();
    let outputs = states.last().unwrap()["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["source"] == 0)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for (output, (a, b)) in outputs.into_iter().zip(curve.spans()) {
        assert_eq!(output["domain"], json!([0., 1.]));
        assert_eq!(output["groups"], json!([]));
        assert_eq!(output["selected"], json!(false));
        assert!(output["name"].is_null());
        for (i, p) in output["points"].as_array().unwrap().iter().enumerate() {
            let t = a + (b - a) * i as f64 / 32.;
            let expected = curve.evaluate(t).unwrap().to_array();
            for axis in 0..3 {
                assert!((p[axis].as_f64().unwrap() - expected[axis]).abs() < 1e-12);
            }
        }
    }
}
