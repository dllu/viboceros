use super::*;
use crate::boolean_union::tests::{
    Boundary, box_brep, compare, regions, setup, snapshot, witnesses,
};
use serde_json::Value;

fn capture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/boolean_intersection_command.json"
    ))
    .unwrap()
}

#[test]
fn replays_27_native_common_and_two_set_outcomes_metadata_geometry_selection_and_history() {
    let capture = capture();
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"]
            .as_str()
            .unwrap()
            .strip_prefix("boolean_intersection_")
            .unwrap();
        if case.starts_with("cancel_") || matches!(case, "single" | "two_sets_cancel") {
            continue;
        }
        count += 1;
        let value = &row["value"];
        let (mut doc, ids, layers, groups) = setup(&value["before"]);
        let registry = CommandRegistry::with_builtins();
        let post = !matches!(
            case,
            "pre_reverse" | "pre_single" | "pre_delete_no_multiple"
        );
        let two = case.starts_with("two_sets") || case == "pre_single";
        let (first, second): (Vec<usize>, Vec<usize>) = match case {
            "two_sets_reverse" => (vec![1], vec![0]),
            "two_sets_disjoint" | "two_sets_common" => (vec![0, 1], vec![2]),
            "two_sets_first_reverse" => (vec![1, 0], vec![2]),
            "two_sets_multi" | "two_sets_one_disjoint" => (vec![0], vec![1, 2]),
            _ if two => (vec![0], vec![1]),
            "post_reverse" => (vec![1, 0], vec![]),
            _ => ((0..ids.len()).collect(), vec![]),
        };
        for &i in first.iter().chain(&second) {
            doc.select_objects_direct([ids[i]], SelectionMode::Add)
                .unwrap();
        }
        let delete = if matches!(
            case,
            "delete_no" | "remember" | "two_sets_delete_no" | "pre_delete_no_multiple"
        ) {
            "No"
        } else {
            "Yes"
        };
        let mut command = format!("BooleanIntersection DeleteInput={delete}");
        if two {
            let set = |indices: &[usize]| {
                indices
                    .iter()
                    .map(|&i| ids[i].to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            command.push_str(&format!(
                " FirstSet={} SecondSet={}",
                set(&first),
                set(&second)
            ));
        }
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
        for (object, native) in doc.objects().zip(expected.as_array().unwrap()) {
            if native["source"].is_null() {
                let Geometry::Brep(b) = object.geometry() else {
                    panic!("intersection B-rep")
                };
                let actual = regions(b);
                let native =
                    serde_json::from_value::<Boundary>(native["face_regions"].clone()).unwrap();
                witnesses(&actual, &native, case);
                witnesses(&native, &actual, case);
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
        if case == "remember" {
            doc.clear_selection();
            // Recreate the recipe in this registry to verify the successful preference.
            let (mut fresh, ids, layers, groups) = setup(&value["before"]);
            fresh
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            registry
                .execute_postselected(&mut fresh, "BooleanIntersection", Default::default())
                .unwrap();
            compare(
                &snapshot(&fresh, &ids, &layers, &groups),
                &value["followup"]["after"],
                "remember followup",
            );
        }
    }
    assert_eq!(count, 27);
}

#[test]
fn unsupported_inputs_invalid_sets_and_locked_objects_preserve_objects_and_redo() {
    for kind in [
        "open",
        "curved",
        "locked",
        "duplicate",
        "missing",
        "invalid",
    ] {
        let mut doc = Document::default();
        let a = doc
            .add_geometry(Geometry::Brep(box_brep([[0., 2.]; 3])))
            .unwrap();
        let b = box_brep([[1., 3.]; 3]);
        let geometry = match kind {
            "open" => Geometry::Brep(b.sub_brep(&[0], Tolerance::DEFAULT).unwrap()),
            "curved" => Geometry::NurbsSurface(
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
            ),
            _ => Geometry::Brep(b),
        };
        let b = doc.add_geometry(geometry).unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 10,0,0").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        doc.select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        if kind == "locked" {
            doc.set_objects_locked([b], true).unwrap();
        }
        let command = match kind {
            "duplicate" => format!("BooleanIntersection FirstSet={a} SecondSet={a}"),
            "missing" => format!("BooleanIntersection FirstSet={a} SecondSet=999999"),
            "invalid" => "BooleanIntersection DeleteInput=No Bogus=Yes".into(),
            "locked" => format!("BooleanIntersection FirstSet={a} SecondSet={b}"),
            _ => "BooleanIntersection".into(),
        };
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let redo = doc.redo_label().map(str::to_owned);
        if kind == "curved" && cfg!(feature = "native-smlib") {
            registry.execute(&mut doc, &command).unwrap();
            assert!(doc.objects().len() > 0);
            assert!(
                doc.objects()
                    .all(|o| matches!(o.geometry(),Geometry::Brep(b)if b.is_solid()))
            );
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            continue;
        }
        assert!(registry.execute(&mut doc, &command).is_err(), "{kind}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before, "{kind}");
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
}
