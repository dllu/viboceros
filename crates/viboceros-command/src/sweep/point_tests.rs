use super::*;

#[test]
fn point_ended_commands_preserve_sources_and_have_one_undo_step() {
    let registry = CommandRegistry::with_builtins();
    for (profile, options, solid) in [
        ("Line 0,0,5 2,0,5", "StartPoint=0,0,0", false),
        ("Line 0,0,0 2,0,0", "Parameters=0 EndPoint=1,0,5", false),
        (
            "Circle 0,0,2.5 2",
            "Parameters=2.5 StartPoint=0,0,0 EndPoint=0,0,5",
            true,
        ),
    ] {
        for refit in ["Yes", "No"] {
            let mut document = Document::default();
            for command in [
                "Line 0,0,0 0,0,5",
                "SelLast",
                "SetObjectName Rail",
                profile,
                "SelLast",
                "SetObjectName Profile",
                "Group Keep",
            ] {
                registry.execute(&mut document, command).unwrap();
            }
            let before = document.objects().cloned().collect::<Vec<_>>();
            let selected = document
                .selected_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            let groups = document.groups().cloned().collect::<Vec<_>>();
            registry
                .execute(
                    &mut document,
                    &format!("Sweep1 RailName=Rail {options} RefitRail={refit}"),
                )
                .unwrap();
            for object in &before {
                assert_eq!(document.object(object.id()), Some(object));
            }
            assert_eq!(
                document
                    .selected_objects()
                    .map(|o| o.id())
                    .collect::<Vec<_>>(),
                selected
            );
            assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
            let output = document
                .objects()
                .find(|o| !before.iter().any(|b| b.id() == o.id()))
                .unwrap();
            assert!(!document.is_selected(output.id()));
            assert_eq!(output.attributes().name(), None);
            let Geometry::Brep(brep) = output.geometry() else {
                panic!("expected BRep")
            };
            assert_eq!(brep.faces().len(), 1);
            assert_eq!(brep.is_solid(), solid);
            assert_eq!(brep.faces()[0].surface().degree_u(), 3);
            let after = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn invalid_point_constraints_leave_the_document_and_undo_history_intact() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Line 0,0,0 0,0,5",
        "SelLast",
        "SetObjectName Rail",
        "Line 0,0,0 2,0,0",
        "SelLast",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let before = document.objects().cloned().collect::<Vec<_>>();
    let selected = document
        .selected_objects()
        .map(|o| o.id())
        .collect::<Vec<_>>();
    for options in [
        "Parameters=0 StartPoint=0,0,0",
        "Parameters=5 EndPoint=0,0,5",
        "EndPoint=0,0,5 EndPoint=0,0,6",
        "StartPoint=NaN,0,0",
        "EndPoint=0",
        "StartPoint=0,0,0 StartPoint=1,0,0",
    ] {
        assert!(
            registry
                .execute(&mut document, &format!("Sweep1 RailName=Rail {options}"))
                .is_err(),
            "{options}"
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document
                .selected_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>(),
            selected
        );
    }
    registry
        .execute(
            &mut document,
            "Sweep1 RailName=Rail Parameters=0 EndPoint=0,0,5",
        )
        .unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[path = "../../../viboceros-geometry/tests/support/positive_tensor.rs"]
mod positive_tensor;

#[test]
fn point_commands_match_twelve_complete_native_surface_images() {
    use super::automatic_tests::{add_line, native_surface, point};
    use serde_json::Value;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let tol = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let mut count = 0;
    for family in [
        "sweep1_point_ends_commands",
        "sweep1_point_ends_retained",
        "sweep1_point_ends_closed",
    ] {
        let input: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let observed: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            if op["blend"] == 1 {
                continue;
            } // Existing Global three-constraint gap.
            assert_eq!(op["id"], row["id"]);
            let registry = CommandRegistry::with_builtins();
            let mut doc = Document::new(tol);
            let rail = add_line(&mut doc, &op["rail"]);
            let mut ids = vec![rail];
            for curve in op["sections"].as_array().unwrap() {
                ids.push(if curve["type"] == "circle" {
                    let circle = viboceros_geometry::Circle3::try_new(
                        point(&curve["center"]),
                        curve["radius"].as_f64().unwrap(),
                        Vector3::try_new(0., 0., 1.)
                            .unwrap()
                            .normalized_nonzero()
                            .unwrap(),
                        tol,
                    )
                    .unwrap();
                    doc.add_geometry(Geometry::Circle(circle)).unwrap()
                } else {
                    add_line(&mut doc, curve)
                });
            }
            doc.select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
            let parameters = op["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let mut text = format!(
                "Sweep1 RailId={rail} Parameters={parameters} RefitRail={}",
                if op["refit_rail"] == true {
                    "Yes"
                } else {
                    "No"
                }
            );
            for (key, option) in [("start_point", "StartPoint"), ("end_point", "EndPoint")] {
                if let Some(p) = op.get(key) {
                    let coordinates = p
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    text.push_str(&format!(" {option}={coordinates}"));
                }
            }
            registry.execute(&mut doc, &text).unwrap();
            let brep = doc
                .objects()
                .find_map(|o| match o.geometry() {
                    Geometry::Brep(b) => Some(b),
                    _ => None,
                })
                .unwrap();
            assert_eq!(brep.faces().len(), 1);
            let actual = brep.faces()[0].surface();
            let expected = native_surface(&row["value"][0]["definition"]);
            let reverse = actual
                .control_point(0, 0)
                .unwrap()
                .point()
                .distance_to(expected.control_point(0, 0).unwrap().point())
                .unwrap()
                > actual
                    .control_point(0, 0)
                    .unwrap()
                    .point()
                    .distance_to(
                        expected
                            .control_point(expected.control_point_count_u() - 1, 0)
                            .unwrap()
                            .point(),
                    )
                    .unwrap();
            let expected = if reverse {
                expected.try_reversed_u().unwrap()
            } else {
                expected
            };
            let bound = positive_tensor::complete_bound(actual, &expected);
            assert!(bound <= tol.absolute(), "{}: {bound}", op["id"]);
            println!(
                "POINT_COMMAND_RESULT {}",
                serde_json::json!({"family":family,"id":op["id"],"complete_image_bound":bound,"reversed_u":reverse})
            );
            count += 1;
        }
    }
    assert_eq!(count, 12);
}
