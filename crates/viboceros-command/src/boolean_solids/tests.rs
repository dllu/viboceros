use super::*;
use crate::boolean_union::tests::{Boundary, box_brep, compare, regions, snapshot, witnesses};
use serde_json::{Value, json};
use viboceros_document::{GroupId, LayerId};
use viboceros_geometry::{BrepBooleanOperation, BrepSolidOrientation};
mod compound;
mod pairs;

fn one(a: Brep, b: Brep, op: BrepBooleanOperation) -> Brep {
    let tolerance = Tolerance::DEFAULT;
    let (brep, sources) = match op {
        BrepBooleanOperation::Union => {
            let r = viboceros_geometry::union_convex_breps(&[&a, &b], tolerance)
                .unwrap()
                .remove(0);
            (r.brep, r.face_sources)
        }
        BrepBooleanOperation::Difference => {
            let r = viboceros_geometry::subtract_convex_breps(&a, &[&b], tolerance)
                .unwrap()
                .remove(0);
            (r.brep, r.face_sources)
        }
        BrepBooleanOperation::Intersection => {
            let r = viboceros_geometry::intersect_convex_breps(&[&a, &b], tolerance)
                .unwrap()
                .unwrap();
            (r.brep, r.face_sources)
        }
    };
    merged(brep, &sources, tolerance).unwrap()
}

fn sources(case: &str) -> Vec<Brep> {
    let tolerance = Tolerance::DEFAULT;
    let compound = |pieces| Brep::try_disjoint_union(pieces, tolerance).unwrap();
    let mut a = box_brep([[0., 3.]; 3]);
    let mut b = box_brep([[1.5, 3.5], [1.5, 3.5], [1., 2.]]);
    if case.contains("concave")
        || matches!(
            case,
            "u_disjoint_member" | "u_two_components" | "i_two_components"
        )
    {
        a = one(
            a,
            box_brep([[2., 4.], [1., 2.], [0., 3.]]),
            BrepBooleanOperation::Union,
        );
        if case.contains("coplanar") {
            b = box_brep([[2., 4.], [1., 3.], [0., 3.]]);
        }
    } else if case.contains("cavity") {
        a = compound(vec![a, box_brep([[1., 2.]; 3]).reversed()]);
    } else if case.contains("island") {
        a = compound(vec![
            box_brep([[0., 4.]; 3]),
            box_brep([[1., 3.]; 3]).reversed(),
            box_brep([[1.5, 2.5]; 3]),
        ]);
        b = box_brep([[2., 5.]; 3]);
    } else if case.contains("disjoint_shells") || case.contains("partial_multishell") {
        a = compound(vec![
            a,
            box_brep(if case.contains("partial") {
                [[10., 11.]; 3]
            } else {
                [[4., 5.]; 3]
            }),
        ]);
        if case.contains("disjoint_shells") {
            b = box_brep([[2., 4.5]; 3]);
        }
    } else if case.contains("rounded_uv") {
        a = one(
            a,
            box_brep([[0.5, 2.5], [-1., 4.], [-1., 4.]]),
            BrepBooleanOperation::Intersection,
        );
    } else {
        a = one(
            a,
            box_brep([[1., 2.], [1., 2.], [-1., 4.]]),
            BrepBooleanOperation::Difference,
        );
        if case.contains("two_holes") {
            let interval = if case.contains("singular") {
                [2., 3.]
            } else {
                [2.25, 3.25]
            };
            b = one(
                box_brep([[1., 4.]; 3]),
                box_brep([interval, interval, [0., 5.]]),
                BrepBooleanOperation::Difference,
            );
        }
        if case.contains("reversed_hole") {
            a = a.reversed();
        }
    }
    if case.contains("split") {
        b = box_brep([[1.5, 2.5], [-1., 4.], [-1., 4.]]);
    }
    if case.contains("unopened") {
        b = box_brep([[2.5, 3.5], [2.5, 3.5], [1., 2.]]);
    }
    let mut result = vec![a, b];
    match case {
        "u_disjoint_member" | "d_noninteracting_target" => result.push(box_brep([[10., 11.]; 3])),
        "u_hole_three" | "d_hole_three_cutters" => {
            result.push(box_brep([[1.5, 2.5], [0.5, 1.5], [1., 2.]]))
        }
        "i_hole_three_common" => result.push(box_brep([[1.75, 4.], [1.5, 3.5], [0.5, 2.5]])),
        "i_hole_three_sets" | "i_cavity_first_multi" | "i_cavity_second_multi" => {
            result.push(box_brep([[0.75, 2.25]; 3]))
        }
        "d_two_targets" | "d_two_targets_split" => result.push(box_brep([[1., 4.]; 3])),
        "u_two_components" | "i_two_components" => {
            result.extend([box_brep([[10., 12.]; 3]), box_brep([[11., 13.]; 3])])
        }
        _ => {}
    }
    result
}

fn setup(case: &str) -> (Document, Vec<ObjectId>, Vec<LayerId>, Vec<GroupId>) {
    setup_sources(sources(case))
}

fn setup_sources(sources: Vec<Brep>) -> (Document, Vec<ObjectId>, Vec<LayerId>, Vec<GroupId>) {
    let mut doc = Document::default();
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    for (i, b) in sources.into_iter().enumerate() {
        let layer = doc
            .add_layer(format!("Source {i}"), ColorRgb::BLACK)
            .unwrap();
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name(format!("source-{i}"))
            .with_object_color(ColorRgb::new(20 + i as u8, 40, 60))
            .try_with_user_text("Code", format!("attribute-{i}"))
            .unwrap();
        let id = doc
            .add_geometry_with_attributes(Geometry::Brep(b), attrs)
            .unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{i}")))
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

// Runtime object order and coplanar face partitions are recorded independently
// of physical geometry. Pair objects by identity or metadata/mass properties;
// separate overlapping boundary shells can have the same centroid.
fn match_rows(actual: &Value, expected: &Value, case: &str) -> Vec<usize> {
    let a = actual.as_array().unwrap();
    let e = expected.as_array().unwrap();
    assert_eq!(
        a.len(),
        e.len(),
        "{case}: object count\n{actual}\n{expected}"
    );
    let mut used = BTreeSet::new();
    e.iter()
        .map(|native| {
            let i = a
                .iter()
                .enumerate()
                .find(|(i, row)| {
                    !used.contains(i)
                        && row["source"] == native["source"]
                        && row["name"] == native["name"]
                        && match (row["volume"].as_f64(), native["volume"].as_f64()) {
                            (Some(a), Some(b)) => (a - b).abs() < 1e-10,
                            (None, None) => row["volume"].is_null() && native["volume"].is_null(),
                            _ => false,
                        }
                        && match (row["centroid"].as_array(), native["centroid"].as_array()) {
                            (Some(a), Some(b)) => a.iter().zip(b).all(|(a, b)| {
                                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-10
                            }),
                            (None, None) => {
                                row["centroid"].is_null() && native["centroid"].is_null()
                            }
                            _ => false,
                        }
                })
                .map(|(i, _)| i)
                .unwrap_or_else(|| {
                    panic!("{case}: unmatched native object {native}\nactual {actual}")
                });
            used.insert(i);
            i
        })
        .collect()
}

fn compare_physical(
    doc: &Document,
    ids: &[ObjectId],
    layers: &[LayerId],
    groups: &[GroupId],
    expected: &Value,
    case: &str,
) -> Vec<Value> {
    let actual = snapshot(doc, ids, layers, groups);
    let permutation = match_rows(&actual, expected, case);
    let objects = doc.objects().collect::<Vec<_>>();
    let mut partitions = Vec::new();
    for (native, &i) in expected.as_array().unwrap().iter().zip(&permutation) {
        let mut row = actual[i].clone();
        if row["faces"] != native["faces"] || row["edges"] != native["edges"] {
            partitions.push(json!({"case":case,"source":native["source"],"name":native["name"],"actual":[row["faces"],row["edges"]],"native":[native["faces"],native["edges"]]}));
        }
        row.as_object_mut().unwrap().remove("faces");
        row.as_object_mut().unwrap().remove("edges");
        compare(&row, native, case);
        let Geometry::Brep(b) = objects[i].geometry() else {
            panic!("B-rep");
        };
        let own = regions(b);
        let captured = serde_json::from_value::<Boundary>(native["face_regions"].clone()).unwrap();
        witnesses(&own, &captured, case);
        witnesses(&captured, &own, case);
        if native["source"].is_null() && native["solid"] == true {
            assert_eq!(
                b.solid_orientation().unwrap(),
                BrepSolidOrientation::Outward,
                "{case}"
            );
        }
    }
    partitions
}

#[test]
fn replays_native_polyhedral_commands_including_nonmanifold_boundary() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/polyhedral_boolean_command.json"
    ))
    .unwrap();
    let mut matched = 0;
    let mut partitions = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"]
            .as_str()
            .unwrap()
            .strip_prefix("polycmd_")
            .unwrap();
        let value = &row["value"];
        let (mut doc, ids, layers, groups) = setup(case);
        assert!(
            compare_physical(&doc, &ids, &layers, &groups, &value["before"], case).is_empty(),
            "{case}: source topology partition"
        );
        let registry = CommandRegistry::with_builtins();
        let first = serde_json::from_value::<Vec<usize>>(value["first"].clone()).unwrap();
        let second = serde_json::from_value::<Vec<usize>>(value["second"].clone()).unwrap();
        let set = |indices: &[usize]| {
            indices
                .iter()
                .map(|&i| ids[i].to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut order = (0..ids.len()).collect::<Vec<_>>();
        if case == "u_concave_reverse" {
            order.reverse();
        }
        for i in order {
            doc.select_objects_direct([ids[i]], SelectionMode::Add)
                .unwrap();
        }
        let delete = if case.ends_with("_keep") || case == "u_hole_keep" {
            "No"
        } else {
            "Yes"
        };
        let command = match case.as_bytes()[0] {
            b'u' => format!(
                "BooleanUnion DeleteInput={delete} MergeCoplanarFaces={}",
                if case.ends_with("nomerge") {
                    "No"
                } else {
                    "Yes"
                }
            ),
            b'i' if case.ends_with("common") => format!("BooleanIntersection DeleteInput={delete}"),
            b'i' => format!(
                "BooleanIntersection DeleteInput={delete} FirstSet={} SecondSet={}",
                set(&first),
                set(&second)
            ),
            b'd' => format!(
                "BooleanDifference DeleteInput={delete} DeleteCutters={} FirstSet={} SecondSet={}",
                if case.ends_with("keep_cutters") {
                    "No"
                } else {
                    "Yes"
                },
                set(&first),
                set(&second)
            ),
            _ => unreachable!(),
        };
        let result = if case.ends_with("_pre") {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        };
        assert!(result.is_ok(), "{case}: {result:?}");
        partitions.extend(compare_physical(
            &doc,
            &ids,
            &layers,
            &groups,
            &value["command"]["after"],
            case,
        ));
        matched += 1;
        if !value["undo"].is_null() {
            for _ in 0..2 {
                registry.execute(&mut doc, "Undo").unwrap();
                compare_physical(&doc, &ids, &layers, &groups, &value["undo"]["after"], case);
                registry.execute(&mut doc, "Redo").unwrap();
                compare_physical(&doc, &ids, &layers, &groups, &value["redo"]["after"], case);
            }
        }
    }
    assert_eq!(matched, 65);
    let expected: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../docs/polyhedral-command-partitions.json"
    ))
    .unwrap();
    assert_eq!(partitions, expected, "unrecorded native seam difference");
    eprintln!(
        "native polyhedral command face partitions: {}",
        serde_json::to_string_pretty(&partitions).unwrap()
    );
}
