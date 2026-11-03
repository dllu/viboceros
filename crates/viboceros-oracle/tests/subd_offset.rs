#[path = "support/subd_sources.rs"]
mod source_archives;
use serde_json::{Value, json};
use viboceros_document::Geometry;
use viboceros_geometry::SubD;
#[path = "support/subd_join.rs"]
mod common;
#[path = "support/subd_offset.rs"]
mod support;
fn value(s: &SubD) -> Value {
    json!({"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"surface_points":s.surface_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":s.faces(),"edges":s.edges(),"vertex_tags":s.vertex_tags(),"edge_tags":s.edge_tags(),"valid":true})
}
fn expected(v: &Value) -> Value {
    let mut v = v.clone();
    v.as_object_mut().unwrap().remove("surface_mesh");
    v
}
#[test]
fn subd_offset_sdk_controls_topology_tags_and_limit_points_match_native() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.3dm");
    for (op, native) in support::cases_for("subd_offset_sdk") {
        let id = op["id"].as_str().unwrap();
        let bytes = native["archive"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>();
        std::fs::write(&path, bytes).unwrap();
        let (doc, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        let Geometry::SubD(source) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        support::compare(
            &value(source),
            &expected(&native["before"]),
            &format!("{id}:before"),
        );
        let archive = source.archive().to_vec();
        let result = source.try_offset(
            op["distance"].as_f64().unwrap(),
            op["solid"].as_bool().unwrap(),
        );
        assert_eq!(source.archive(), archive);
        if native["output"].is_null() {
            assert!(result.is_err(), "{id}");
        } else {
            support::compare(&value(&result.unwrap()), &expected(&native["output"]), id);
        }
    }
}

#[test]
fn offset_subd_commands_match_native_geometry_metadata_selection_and_history() {
    use viboceros_document::SelectionMode;
    use viboceros_oracle::subd_join::snapshot;
    let sources = support::sources();
    for (op, native) in support::cases_for("subd_offset_command") {
        let (mut doc, id) = support::fixture(&sources[op["scene"].as_str().unwrap()]);
        if op["preselect"] == true {
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        support::compare(
            &snapshot(&doc, &[id]),
            &support::expected(&native["before"]),
            op["id"].as_str().unwrap(),
        );
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let registry = viboceros_command::CommandRegistry::with_builtins();
        let command = support::options(&op).command();
        if op["preselect"] == true {
            registry.execute(&mut doc, &command).unwrap();
        } else {
            registry
                .execute_postselected(&mut doc, &command, Default::default())
                .unwrap();
        }
        support::compare(
            &snapshot(&doc, &[id]),
            &support::expected(&native["after"]),
            op["id"].as_str().unwrap(),
        );
        // The capture macro's trailing Cancel releases retained original picks.
        doc.clear_selection();
        support::compare(
            &snapshot(&doc, &[id]),
            &support::expected(&native["after_script"]),
            op["id"].as_str().unwrap(),
        );
        doc.undo().unwrap();
        support::compare(
            &snapshot(&doc, &[id]),
            &support::expected(&native["undo"]),
            op["id"].as_str().unwrap(),
        );
        doc.redo().unwrap();
        support::compare(
            &snapshot(&doc, &[id]),
            &support::expected(&native["redo"]),
            op["id"].as_str().unwrap(),
        );
    }
}
#[test]
fn offset_subd_native_defaults_and_preferences_survive_completion_and_cancel() {
    use viboceros_document::SelectionMode;
    use viboceros_oracle::subd_join::snapshot;
    let sources = support::sources();
    let registry = viboceros_command::CommandRegistry::with_builtins();
    let native = support::cases_for("subd_offset_preferences").remove(0).1;
    for (i, row) in native.as_array().unwrap().iter().enumerate() {
        let (mut doc, id) = support::fixture(&sources["quad"]);
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let input = &row["input"];
        let command = if input["defaults"] == true {
            "OffsetSubD".into()
        } else {
            support::options(input).command()
        };
        if input["cancel"] == true {
            registry.remember_offset_subd_options(support::options(input));
        } else {
            registry.execute(&mut doc, &command).unwrap();
        }
        support::compare(
            &snapshot(&doc, &[id]),
            &support::expected(&row["value"]["after"]),
            &format!("preferences {i}"),
        );
    }
}

#[test]
fn offset_subd_batches_match_native_output_order_groups_and_history() {
    use viboceros_document::SelectionMode;
    use viboceros_oracle::subd_join::snapshot;
    let sources = support::sources();
    for (op, native) in support::cases_for("subd_offset_batch") {
        let (mut doc, ids) = support::fixture_many(&[&sources["quad"], &sources["closed"]]);
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let command = viboceros_command::offset_subd::Options {
            distance: 0.75,
            solid: op["solid"].as_bool().unwrap(),
            both_sides: op["both_sides"].as_bool().unwrap(),
            ..Default::default()
        }
        .command();
        let registry = viboceros_command::CommandRegistry::with_builtins();
        if op["preselect"] == true {
            registry.execute(&mut doc, &command).unwrap();
        } else {
            registry
                .execute_postselected(&mut doc, &command, Default::default())
                .unwrap();
        }
        support::compare(
            &snapshot(&doc, &ids),
            &support::expected(&native["after"]),
            op["id"].as_str().unwrap(),
        );
        doc.clear_selection();
        doc.undo().unwrap();
        support::compare(
            &snapshot(&doc, &ids),
            &support::expected(&native["undo"]),
            op["id"].as_str().unwrap(),
        );
        doc.redo().unwrap();
        support::compare(
            &snapshot(&doc, &ids),
            &support::expected(&native["redo"]),
            op["id"].as_str().unwrap(),
        );
    }
}
#[test]
fn offset_subd_soft_features_and_refined_shells_roundtrip_native_3dm() {
    use viboceros_oracle::subd_join::snapshot;
    let sources = support::sources();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("offset.3dm");
    for source in [&sources["bent"], &sources["closed"]] {
        let source = source.subdivided(1).unwrap();
        let edge = source
            .edge_face_counts()
            .iter()
            .position(|&n| n == 2)
            .unwrap();
        let source = source
            .try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
            .unwrap()
            .0;
        for distance in [-0.25, 0.25] {
            for solid in [false, true] {
                let result = source.try_offset(distance, solid).unwrap();
                assert_eq!(
                    &result.edge_sharpness()[..source.edges().len()],
                    source.edge_sharpness()
                );
                if solid && source.has_boundary() {
                    assert_eq!(
                        &result.edge_sharpness()[source.edges().len()..2 * source.edges().len()],
                        source.edge_sharpness()
                    );
                }
                let (mut doc, id) = support::fixture(&source);
                doc.add_geometry(Geometry::SubD(result)).unwrap();
                viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[])
                    .unwrap();
                let (restored, _, _, _, _) =
                    viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap())
                        .unwrap();
                support::compare(
                    &snapshot(&restored, &[id]),
                    &snapshot(&doc, &[id]),
                    "soft shell roundtrip",
                );
                for (a, b) in doc.objects().zip(restored.objects()) {
                    let (Geometry::SubD(a), Geometry::SubD(b)) = (a.geometry(), b.geometry())
                    else {
                        panic!()
                    };
                    assert_eq!(a.edge_sharpness(), b.edge_sharpness());
                }
            }
        }
    }
}
