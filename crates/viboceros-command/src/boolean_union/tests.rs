use super::*;
use serde_json::{Value, json};
use viboceros_document::{GroupId, LayerId};

pub(crate) fn box_brep(bounds: [[f64; 2]; 3]) -> Brep {
    let frame = Frame3::try_from_directions(
        Point3::try_from([0.; 3]).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(frame, bounds, Tolerance::DEFAULT).unwrap()
}

fn capture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/boolean_union_command.json"
    ))
    .unwrap()
}

pub(crate) fn setup(before: &Value) -> (Document, Vec<ObjectId>, Vec<LayerId>, Vec<GroupId>) {
    let mut doc = Document::default();
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    for row in before.as_array().unwrap() {
        let index = ids.len();
        let layer = doc
            .add_layer(format!("Source {index}"), ColorRgb::BLACK)
            .unwrap();
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name(format!("source-{index}"))
            .with_object_color(ColorRgb::new(20 + index as u8, 40, 60))
            .try_with_user_text("Code", format!("attribute-{index}"))
            .unwrap();
        let geometry = if row["kind"] == "Point" {
            Geometry::Point(
                Point3::try_from(serde_json::from_value::<[f64; 3]>(row["point"].clone()).unwrap())
                    .unwrap(),
            )
        } else {
            let points = row["vertices"].as_array().unwrap();
            let bounds = std::array::from_fn(|i| {
                [
                    points
                        .iter()
                        .map(|p| p[i].as_f64().unwrap())
                        .fold(f64::INFINITY, f64::min),
                    points
                        .iter()
                        .map(|p| p[i].as_f64().unwrap())
                        .fold(f64::NEG_INFINITY, f64::max),
                ]
            });
            Geometry::Brep(box_brep(bounds))
        };
        let id = doc.add_geometry_with_attributes(geometry, attrs).unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{index}")))
            .unwrap();
        ids.push(id);
        layers.push(layer);
    }
    let mut groups = ids
        .iter()
        .map(|&id| doc.add_group(None, [id]).unwrap())
        .collect::<Vec<_>>();
    groups.push(doc.add_group(None, ids.iter().copied()).unwrap());
    (doc, ids, layers, groups)
}

pub(crate) fn snapshot(
    doc: &Document,
    ids: &[ObjectId],
    layers: &[LayerId],
    groups: &[GroupId],
) -> Value {
    Value::Array(doc.objects().map(|o| {
        let a = o.attributes();
        let c = a.object_color();
        let mut row = json!({"source":ids.iter().position(|id|*id==o.id()),
            "selected":doc.is_selected(o.id()),"name":a.name(),"layer":layers.iter().position(|id|*id==a.layer_id()),
            "color":[c.red,c.green,c.blue],"groups":groups.iter().enumerate().filter_map(|(i,g)|o.group_ids().contains(g).then_some(i)).collect::<Vec<_>>(),
            "attribute_text":a.user_text().get("Code"),"geometry_text":o.geometry_user_text().get("Code")});
        match o.geometry() {
            Geometry::Brep(b) => {
                row["kind"] = json!("Brep"); row["valid"] = json!(true); row["solid"] = json!(b.is_solid());
                row["faces"] = json!(b.faces().len()); row["edges"] = json!(b.edges().len());
                row["area"] = json!(b.area(doc.tolerance()).unwrap());
                if b.is_solid() {
                    let m=b.volume_mass_properties(doc.tolerance()).unwrap();
                    row["volume"] = json!(m.signed_volume().unwrap());
                    row["centroid"] = json!(m.centroid().unwrap().to_array());
                }else {row["volume"]=Value::Null;row["centroid"]=Value::Null;}
            }
            Geometry::Point(p) => { row["kind"]=json!("Point");row["point"]=json!(p.to_array()); }
            _ => panic!("closed recipe"),
        }
        row
    }).collect())
}

pub(crate) fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let epsilon = if path.ends_with("volume") || path.contains("centroid") {
                1e-10
            } else {
                1e-9
            };
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= epsilon,
                "{path}: {actual} != {expected}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                compare(value, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

pub(crate) type Boundary = Vec<Vec<Vec<[f64; 3]>>>;
pub(crate) fn regions(b: &Brep) -> Boundary {
    b.faces()
        .iter()
        .map(|f| {
            f.loops()
                .iter()
                .map(|l| {
                    l.trims()
                        .iter()
                        .map(|t| b.vertices()[t.vertices()[0]].point().to_array())
                        .collect()
                })
                .collect()
        })
        .collect()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3])
}
fn on_boundary(p: [f64; 3], regions: &Boundary) -> bool {
    regions.iter().any(|face| {
        let outer = &face[0];
        let n = (1..outer.len() - 1)
            .map(|i| cross(sub(outer[i], outer[0]), sub(outer[i + 1], outer[0])))
            .find(|n| dot(*n, *n) > 1e-20)
            .unwrap();
        if dot(n, sub(p, outer[0])).abs() > 1e-7 * dot(n, n).sqrt() {
            return false;
        }
        let axis = (0..3)
            .max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs()))
            .unwrap();
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut inside = false;
        for ring in face {
            for i in 0..ring.len() {
                let (s, e) = (ring[i], ring[(i + 1) % ring.len()]);
                let d = sub(e, s);
                let t = (dot(sub(p, s), d) / dot(d, d)).clamp(0., 1.);
                let q = std::array::from_fn(|i| s[i] + t * d[i]);
                if dot(sub(p, q), sub(p, q)) <= 1e-14 {
                    return true;
                }
                if (s[b] > p[b]) != (e[b] > p[b])
                    && p[a] < s[a] + (p[b] - s[b]) * (e[a] - s[a]) / (e[b] - s[b])
                {
                    inside = !inside;
                }
            }
        }
        inside
    })
}
pub(crate) fn witnesses(a: &Boundary, b: &Boundary, label: &str) {
    for face in a {
        for ring in face {
            for i in 0..ring.len() {
                let (start, end) = (ring[i], ring[(i + 1) % ring.len()]);
                for station in 0..=8 {
                    let p = std::array::from_fn(|i| {
                        start[i] + (end[i] - start[i]) * station as f64 / 8.
                    });
                    assert!(on_boundary(p, b), "{label} boundary {p:?}");
                }
            }
        }
    }
}

#[test]
fn replays_27_closed_command_outcomes_metadata_selection_geometry_and_history() {
    let capture = capture();
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"]
            .as_str()
            .unwrap()
            .strip_prefix("boolean_union_")
            .unwrap();
        if case.starts_with("cancel_") || matches!(case, "open_surface" | "touch_edge") {
            continue;
        }
        count += 1;
        let value = &row["value"];
        let (mut doc, ids, layers, groups) = setup(&value["before"]);
        let registry = CommandRegistry::with_builtins();
        let post = !matches!(case, "pre_reverse" | "pre_delete_no");
        let mut order = ids.clone();
        if matches!(case, "pre_reverse" | "post_reverse") {
            order.reverse();
        }
        for id in order {
            doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        }
        let delete = if matches!(case, "delete_no" | "remember" | "pre_delete_no") {
            "No"
        } else {
            "Yes"
        };
        let merge = if matches!(
            case,
            "merge_yes"
                | "partial_merge"
                | "touch_merge"
                | "remember"
                | "three_shared_merge"
                | "defaults"
        ) {
            "Yes"
        } else {
            "No"
        };
        let command = if case == "defaults" {
            "BooleanUnion".into()
        } else {
            format!("BooleanUnion DeleteInput={delete} MergeCoplanarFaces={merge}")
        };
        let result = if post {
            registry.execute_postselected(&mut doc, &command, Default::default())
        } else {
            registry.execute(&mut doc, &command)
        };
        let expected = &value["command"]["after"];
        let success = expected
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["source"].is_null());
        assert_eq!(result.is_ok(), success, "{case}: {result:?}");
        compare(&snapshot(&doc, &ids, &layers, &groups), expected, case);
        if success {
            for (o, native) in doc.objects().zip(expected.as_array().unwrap()) {
                if native["source"].is_null() {
                    let Geometry::Brep(b) = o.geometry() else {
                        panic!("union B-rep")
                    };
                    let a = regions(b);
                    let b =
                        serde_json::from_value::<Boundary>(native["face_regions"].clone()).unwrap();
                    witnesses(&a, &b, case);
                    witnesses(&b, &a, case);
                }
            }
        }
        if case == "undo_redo" {
            for _ in 0..2 {
                registry.execute(&mut doc, "Undo").unwrap();
                compare(
                    &snapshot(&doc, &ids, &layers, &groups),
                    &value["undo"]["after"],
                    "Undo",
                );
                registry.execute(&mut doc, "Redo").unwrap();
                compare(
                    &snapshot(&doc, &ids, &layers, &groups),
                    &value["redo"]["after"],
                    "Redo",
                );
            }
        }
    }
    assert_eq!(count, 27); // Three prompt cancellations and two unsupported successes excluded.
}

#[test]
fn unsupported_inputs_and_excluded_locked_objects_preserve_geometry_ids_and_redo() {
    for kind in ["open", "curved", "locked"] {
        let mut doc = Document::default();
        let first = doc
            .add_geometry(Geometry::Brep(box_brep([[0., 2.]; 3])))
            .unwrap();
        let geometry = if kind == "open" {
            Geometry::Brep(
                box_brep([[1., 3.]; 3])
                    .sub_brep(&[0], Tolerance::DEFAULT)
                    .unwrap(),
            )
        } else if kind == "curved" {
            Geometry::NurbsSurface(
                NurbsSurface::try_sphere(
                    Frame3::try_from_normal(
                        Point3::try_from([1.; 3]).unwrap(),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    1.,
                )
                .unwrap(),
            )
        } else {
            Geometry::Brep(box_brep([[1., 3.]; 3]))
        };
        let second = doc.add_geometry(geometry).unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 10,0,0").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        doc.select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        if kind == "locked" {
            doc.set_objects_locked([second], true).unwrap();
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let redo = doc.redo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut doc, "BooleanUnion").is_err(),
            "{kind}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
}

#[test]
fn accepted_preferences_survive_failure_but_invalid_batches_and_fresh_registries_do_not() {
    let command = BooleanUnionCommand::default();
    command
        .accept_object_selection_options(&["_DeleteInput", "_No", "MergeCoplanarFaces=No"])
        .unwrap();
    for arguments in [
        vec!["DeleteInput=Yes", "MergeCoplanarFaces=Maybe"],
        vec!["DeleteInput=Yes", "DeleteInput=No"],
        vec!["Bogus=Yes"],
    ] {
        assert!(command.accept_object_selection_options(&arguments).is_err());
        let p = command.object_selection_prompt(&[]).unwrap().unwrap();
        assert!(p.options.iter().all(|o| !o.value));
    }
    let mut doc = Document::default();
    assert!(command.run(&mut doc, &["MergeCoplanarFaces=Yes"]).is_err());
    let p = command.object_selection_prompt(&[]).unwrap().unwrap();
    assert!(!p.options[0].value && p.options[1].value);
    assert!(
        BooleanUnionCommand::default()
            .object_selection_prompt(&[])
            .unwrap()
            .unwrap()
            .options
            .iter()
            .all(|o| o.value)
    );
}
