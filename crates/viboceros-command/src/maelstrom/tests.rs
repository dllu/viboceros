use super::*;
use crate::morph_test_support::*;
use serde_json::Value;
use viboceros_document::SelectionMode;
mod surface_image;

fn radius(v: &Value) -> MaelstromRadius {
    v.as_f64()
        .map(MaelstromRadius::Number)
        .unwrap_or_else(|| MaelstromRadius::Point(p(v)))
}
fn context(op: &Value) -> CommandContext {
    CommandContext {
        construction_plane: Frame3::try_from_normal(
            p(&op["origin"]),
            Vector3::try_from(serde_json::from_value::<[Real; 3]>(op["normal"].clone()).unwrap())
                .unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap(),
    }
}

#[test]
fn actual_maelstrom_commands_match_geometry_attributes_groups_selection_and_history() {
    let mut count = 0;
    for (fixture, observation) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_geometry_command.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_geometry_command.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_fitting_command.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_fitting_command.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_identity_command.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_identity_command.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/maelstrom_fit_correspondence.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_fit_correspondence.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/maelstrom_radius_command.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/maelstrom_radius_command.json"
            ),
        ),
    ] {
        let f: Value = serde_json::from_str(fixture).unwrap();
        let o: Value = serde_json::from_str(observation).unwrap();
        assert_eq!(
            f["operations"].as_array().unwrap().len(),
            o["results"].as_array().unwrap().len()
        );
        for (op, row) in f["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(o["results"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(op["id"], row["id"]);
            let label = op["id"].as_str().unwrap();
            let v = &row["value"];
            let result = v["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == "Maelstrom")
                .unwrap()["result"]
                .as_str()
                .unwrap();
            let registry = CommandRegistry::with_builtins();
            let (mut doc, ids) = setup(op, &v["before"]);
            let originals = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().geometry().clone())
                .collect::<Vec<_>>();
            let options = MaelstromOptions {
                copy: op["copy"].as_bool().unwrap(),
                rigid: op["rigid"].as_bool().unwrap(),
            };
            let sources = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let mut group = doc.begin_history_group("Maelstrom").unwrap();
            for angle in
                std::iter::once(&op["degrees"]).chain(op["angles"].as_array().into_iter().flatten())
            {
                let r = registry.execute_in_history_group(
                    &mut doc,
                    &format!(
                        "Maelstrom {} {} {} {} {} Sources={sources}",
                        format_point(p(&op["origin"])),
                        radius(&op["radius0"]).command_argument(),
                        radius(&op["radius1"]).command_argument(),
                        angle.as_f64().unwrap(),
                        options.command_options()
                    ),
                    context(op),
                    &mut group,
                );
                if result == "Cancel" && !options.copy {
                    assert!(r.is_err(), "{label}: invalid radius accepted");
                } else {
                    r.unwrap_or_else(|e| panic!("{label}: {e}"));
                }
            }
            let epsilon = if options.rigid {
                1e-7
            } else if op["shape"] == "Points" {
                1e-11
            } else {
                2. * op["tolerance"].as_f64().unwrap_or(1e-5).max(1e-5)
            };
            for phase in ["after", "undo", "redo"] {
                if v[phase].is_null() {
                    continue;
                }
                if phase == "undo" {
                    registry.execute(&mut doc, "Undo").unwrap();
                }
                if phase == "redo" {
                    registry.execute(&mut doc, "Redo").unwrap();
                }
                let expected = &v[phase];
                assert_eq!(
                    doc.objects().count(),
                    expected["objects"].as_array().unwrap().len(),
                    "{label}: {phase}"
                );
                let trailing_cancel = v["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["name"] == "Cancel")
                    && result == "Success";
                if trailing_cancel && phase == "redo" {
                    let mut command_state = expected.clone();
                    for (object, row) in doc
                        .objects()
                        .zip(command_state["objects"].as_array_mut().unwrap())
                    {
                        row["selected"] = doc.is_selected(object.id()).into();
                    }
                    assert_metadata(&doc, &command_state, &format!("{label}: {phase}"));
                } else {
                    assert_metadata(&doc, expected, &format!("{label}: {phase}"));
                }
                for (i, (object, record)) in doc
                    .objects()
                    .zip(expected["objects"].as_array().unwrap())
                    .enumerate()
                {
                    let compare = if op["shape"] == "Box" {
                        surface_image::compare
                    } else {
                        compare_geometry
                    };
                    compare(
                        object.geometry(),
                        &record["geometry"],
                        &originals[i % ids.len()],
                        &v["before"]["objects"][i % ids.len()]["geometry"],
                        epsilon,
                        &format!("{label}: {phase}"),
                    );
                    if op["degrees"] == 0. && phase != "undo" {
                        if op["shape"] == "Line" {
                            compare_preserved_structure(
                                object.geometry(),
                                &record["geometry"],
                                &originals[i % ids.len()],
                                label,
                            );
                        } else {
                            assert_eq!(
                                object.geometry(),
                                &originals[i % ids.len()],
                                "{label}: identity structure"
                            );
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 85);
}

#[test]
fn native_fit_parameter_slip_is_geometric_and_exact_point_maps_still_match() {
    use viboceros_geometry::PointMorph;
    let f: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/maelstrom_fit_correspondence.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/maelstrom_fit_correspondence.json"
    ))
    .unwrap();
    let op = &f["operations"][0];
    let value = &r["results"][0]["value"];
    let morph = point_morph(
        p(&op["origin"]),
        radius(&op["radius0"]),
        radius(&op["radius1"]),
        op["degrees"].as_f64().unwrap(),
        context(op),
    )
    .unwrap();
    let samples = value["fit_correspondence"].as_array().unwrap();
    assert_eq!(samples.len(), 486);
    assert!(
        samples
            .iter()
            .any(|s| s["parameter_error"].as_f64().unwrap() > 8e-5)
    );
    for sample in samples {
        let s = surface(
            &value["before"]["objects"][0]["geometry"]["surfaces"]
                [sample["face"].as_u64().unwrap() as usize],
        );
        let point = s
            .evaluate(
                s.parameter_at_u(sample["uv"][0].as_f64().unwrap()).unwrap(),
                s.parameter_at_v(sample["uv"][1].as_f64().unwrap()).unwrap(),
            )
            .unwrap();
        near(
            morph.morph_point(point).unwrap(),
            p(&sample["exact"]),
            1e-11,
            "diagnostic SDK map",
        );
        near(
            p(&sample["exact"]),
            p(&sample["closest"]),
            2e-6,
            "native geometric image",
        );
        assert!(matches!(
            sample["relation"].as_str(),
            Some("Interior" | "Boundary")
        ));
    }
}

#[test]
fn remembered_radius_immediate_copy_and_reset_rigid_match_29_native_steps() {
    let observation: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/maelstrom_options_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let add_source = |doc: &mut Document| {
        doc.add_geometry(Geometry::Point(Point3::try_from([3., 1., 5.]).unwrap()))
            .unwrap()
    };
    let mut source = add_source(&mut doc);
    for row in observation["results"][0]["value"]["records"]
        .as_array()
        .unwrap()
    {
        registry.begin_copy_options("Maelstrom");
        let assert_defaults = |query: &Value| {
            assert_eq!(
                registry.maelstrom_options_default(),
                MaelstromOptions {
                    copy: query["defaults"]["Copy"].as_bool().unwrap(),
                    rigid: query["defaults"]["Rigid"].as_bool().unwrap()
                }
            );
            assert_eq!(
                registry.maelstrom_radius_default(),
                query["first_radius"].as_f64().unwrap()
            );
        };
        assert_defaults(&row["query_before"]);
        let step = &row["step"];
        match step["kind"].as_str().unwrap() {
            "Maelstrom" => {
                let first_radius = step["radius0"].as_f64().unwrap_or(2.);
                assert!(registry.remember_maelstrom_radius(first_radius));
                let mut options = registry.maelstrom_options_default();
                for (name, value) in step["options"].as_object().unwrap() {
                    options
                        .update(&format!(
                            "{name}={}",
                            if value.as_bool().unwrap() {
                                "Yes"
                            } else {
                                "No"
                            }
                        ))
                        .unwrap();
                    if name == "Copy" {
                        registry.remember_maelstrom_copy_option(options.copy);
                    }
                }
                let finish = step["finish"].as_str().unwrap();
                if finish == "Complete" || finish.starts_with("CopyThen") {
                    doc.select_objects_direct([source], SelectionMode::Replace)
                        .unwrap();
                    registry
                        .execute(
                            &mut doc,
                            &format!(
                                "Maelstrom 0,0,0 {first_radius} 5 90 {}",
                                options.command_options()
                            ),
                        )
                        .unwrap();
                }
                if let Some(pending) = step["pending_options"].as_object() {
                    for (name, value) in pending {
                        options
                            .update(&format!(
                                "{name}={}",
                                if value.as_bool().unwrap() {
                                    "Yes"
                                } else {
                                    "No"
                                }
                            ))
                            .unwrap();
                        if name == "Copy" {
                            registry.remember_maelstrom_copy_option(options.copy);
                        }
                    }
                }
            }
            "RememberCopyOptions" => {
                registry
                    .execute(
                        &mut doc,
                        if step["enabled"].as_bool().unwrap() {
                            "RememberCopyOptions Yes"
                        } else {
                            "RememberCopyOptions No"
                        },
                    )
                    .unwrap();
            }
            "Undo" => {
                registry.execute(&mut doc, "Undo").unwrap();
            }
            "Redo" => {
                registry.execute(&mut doc, "Redo").unwrap();
            }
            "New" => {
                doc = Document::default();
                source = add_source(&mut doc);
            }
            _ => panic!("unexpected preference action"),
        }
        registry.begin_copy_options("Maelstrom");
        assert_defaults(&row["query_after"]);
    }
    let remembered = registry.maelstrom_radius_default();
    for value in [0., -1., SDK_ZERO, Real::NAN, Real::INFINITY] {
        assert!(!registry.remember_maelstrom_radius(value));
        assert_eq!(registry.maelstrom_radius_default(), remembered);
    }
    assert_eq!(
        CommandRegistry::with_builtins().maelstrom_radius_default(),
        1.
    );
}

#[test]
fn invalid_commands_keep_document_state_and_save_only_accepted_getter_preferences() {
    for copy in [false, true] {
        let mut doc = Document::default();
        let ids = [[2., 1., 5.], [6., 1., 7.]].map(|p| {
            doc.add_geometry(Geometry::Point(Point3::try_from(p).unwrap()))
                .unwrap()
        });
        doc.add_group(None, ids).unwrap();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let state = |doc: &Document| {
            doc.objects()
                .map(|o| {
                    (
                        o.id(),
                        o.geometry().clone(),
                        o.attributes().clone(),
                        o.group_ids().to_vec(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let before = state(&doc);
        let label = doc.undo_label().map(str::to_owned);
        for (input, accepted_circle) in [
            ("Maelstrom 0,0,0 0 5 90", false),
            ("Maelstrom 0,0,0 2 NaN 90", true),
            ("Maelstrom 0,0,0 -2 5 90", false),
            ("Maelstrom 0,0,0 2 5 NaN", true),
            ("Maelstrom 0,0,0 0,0,0 5 90", false),
            ("Maelstrom 0,0,0 2 0,0,5 90", true),
            ("Maelstrom 0,0,0 2 5 90 Rigid=Yes Rigid=No", false),
            ("Maelstrom 0,0,0 2 5 90 PreserveStructure=Yes", false),
            ("Maelstrom 0,0,0 2 5", false),
        ] {
            let registry = CommandRegistry::with_builtins();
            let input = format!("{input} Copy={}", if copy { "Yes" } else { "No" });
            assert!(registry.execute(&mut doc, &input).is_err(), "{input}");
            assert_eq!(state(&doc), before, "{input}");
            assert_eq!(doc.undo_label(), label.as_deref());
            assert_eq!(doc.selected_object_count(), 2);
            assert_eq!(
                registry.maelstrom_options_default(),
                MaelstromOptions {
                    copy: accepted_circle && copy,
                    rigid: false
                }
            );
            assert_eq!(
                registry.maelstrom_radius_default(),
                if accepted_circle { 2. } else { 1. }
            );
        }
    }
}
