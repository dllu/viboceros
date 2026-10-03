use super::*;
use crate::morph_test_support::*;
use serde_json::Value;
use viboceros_document::SelectionMode;

#[test]
fn small_angle_rigid_frames_match_five_native_orthogonality_boundary_cases() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/twist_rigid_frame_boundary_command.json"
    ))
    .unwrap();
    let observations: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/twist_rigid_frame_boundary_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (op, row) in fixtures["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observations["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        let (mut doc, _) = setup(op, &row["value"]["before"]);
        registry
            .execute(
                &mut doc,
                &format!(
                    "Twist 0,0,0 0,0,10 {} Rigid=Yes Copy=No Infinite=No PreserveStructure=No",
                    op["degrees"].as_f64().unwrap()
                ),
            )
            .unwrap();
        for phase in ["after", "undo", "redo"] {
            if phase == "undo" {
                registry.execute(&mut doc, "Undo").unwrap();
            } else if phase == "redo" {
                registry.execute(&mut doc, "Redo").unwrap();
            }
            let expected = &row["value"][phase];
            assert_metadata(&doc, expected, label);
            for (object, native) in doc.objects().zip(expected["objects"].as_array().unwrap()) {
                let Geometry::Point(actual) = object.geometry() else {
                    panic!();
                };
                near(*actual, p(&native["geometry"]["points"][0]), 1e-7, label);
            }
        }
    }
}

#[test]
fn replay_actual_twist_commands_rigid_placements_and_fitting_tolerances() {
    let registry = CommandRegistry::with_builtins();
    for (fixtures, observations) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_rigid_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_rigid_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_tight_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_tight_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_fitting_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_fitting_command.json"),
        ),
    ] {
        let f: Value = serde_json::from_str(fixtures).unwrap();
        let o: Value = serde_json::from_str(observations).unwrap();
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
            let label = op["id"].as_str().unwrap();
            assert_eq!(op["id"], row["id"]);
            assert_eq!(row["value"]["success"], true);
            let (mut doc, ids) = setup(op, &row["value"]["before"]);
            let (start, end) = axis(op);
            let source_geometry = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().geometry().clone())
                .collect::<Vec<_>>();
            let options = TwistOptions {
                copy: op["copy"].as_bool().unwrap(),
                rigid: op["rigid"].as_bool().unwrap(),
                infinite: op["infinite"].as_bool().unwrap(),
                preserve_structure: op["preserve"].as_bool().unwrap(),
            };
            let input = format!(
                "Twist {} {} {} {}",
                format_point(start),
                format_point(end),
                op["degrees"].as_f64().unwrap(),
                options.command_options()
            );
            registry
                .execute(&mut doc, &input)
                .unwrap_or_else(|e| panic!("{label}: {e}"));
            let expected = &row["value"]["after"];
            assert_eq!(
                doc.objects().count(),
                expected["objects"].as_array().unwrap().len(),
                "{label}"
            );
            let epsilon = if options.rigid {
                1e-7
            } else if op["shape"] == "Points"
                || (options.preserve_structure && op["shape"] != "Box")
            {
                1e-11
            } else {
                2. * op["tolerance"].as_f64().unwrap_or(1e-5).max(1e-5)
            };
            for (i, (actual, record)) in doc
                .objects()
                .zip(expected["objects"].as_array().unwrap())
                .enumerate()
            {
                compare_geometry(
                    actual.geometry(),
                    &record["geometry"],
                    &source_geometry[i % ids.len()],
                    &row["value"]["before"]["objects"][i % ids.len()]["geometry"],
                    epsilon,
                    label,
                );
                if options.preserve_structure && !options.rigid && op["shape"] != "Box" {
                    compare_preserved_structure(
                        actual.geometry(),
                        &record["geometry"],
                        &source_geometry[i % ids.len()],
                        label,
                    );
                }
                assert_eq!(
                    doc.is_selected(actual.id()),
                    record["selected"].as_bool().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    actual.attributes().name(),
                    record["name"].as_str(),
                    "{label}"
                );
                assert_eq!(
                    actual.group_ids().len(),
                    record["groups"].as_array().unwrap().len(),
                    "{label}"
                );
            }
            assert_metadata(&doc, expected, label);
            if let Some(expected) = row["value"]["undo"].as_object() {
                registry.execute(&mut doc, "Undo").unwrap();
                assert_eq!(doc.objects().count(), ids.len(), "{label}");
                assert_eq!(
                    doc.groups().count(),
                    expected["groups"].as_array().unwrap().len(),
                    "{label}"
                );
                assert_metadata(&doc, &row["value"]["undo"], label);
                registry.execute(&mut doc, "Redo").unwrap();
                assert_metadata(&doc, &row["value"]["redo"], label);
                assert_eq!(
                    doc.objects().count(),
                    row["value"]["redo"]["objects"].as_array().unwrap().len(),
                    "{label}"
                );
            }
        }
    }
}
#[test]
fn twist_invalid_inputs_leave_geometry_and_history_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.object(id).unwrap().geometry().clone();
    let depth = doc.undo_label().map(str::to_owned);
    for input in [
        "Twist 0,0,0 0,0,0 90",
        "Twist 0,0,0 0,0,10 NaN",
        "Twist 0,0,0 0,0,10 90 Copy=Maybe",
        "Twist 0,0,0 0,0,10 90 Rigid=Yes Rigid=No",
        "Twist 0,0,0 0,0,10 0,0,5 1,0,5",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(doc.object(id).unwrap().geometry(), &before);
        assert_eq!(doc.undo_label().map(str::to_owned), depth);
    }
    registry
        .execute(&mut doc, "Twist 0,0,0 0,0,10 1,0,0 0,1,0")
        .unwrap();
    near(
        match doc.object(id).unwrap().geometry() {
            Geometry::Point(p) => *p,
            _ => unreachable!(),
        },
        Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap(),
        1e-12,
        "reference angle",
    );
}

#[test]
fn twist_scripted_preferences_are_shared_across_documents_and_isolated_between_registries() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(
            &mut doc,
            "Twist 0,0,0 0,0,10 0 Rigid=Yes Infinite=Yes PreserveStructure=Yes",
        )
        .unwrap();
    let expected = TwistOptions {
        rigid: true,
        infinite: true,
        preserve_structure: true,
        copy: false,
    };
    assert_eq!(registry.twist_options_default(), expected);
    for input in [
        "Twist 0,0,0 0,0,0 45 Rigid=No Infinite=No PreserveStructure=No",
        "Twist 0,0,0 0,0,10 NaN Rigid=No",
        "Twist 0,0,0 0,0,10 45 Rigid=Maybe",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(registry.twist_options_default(), expected);
        assert_eq!(registry.transform_scalar_default("Twist"), Some(0.));
    }
    registry.execute(&mut doc, "Twist 0,0,0 0,0,10 90").unwrap();
    near(
        match doc.object(id).unwrap().geometry() {
            Geometry::Point(p) => *p,
            _ => unreachable!(),
        },
        Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap(),
        1e-7,
        "remembered Infinite/Rigid",
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(registry.twist_options_default(), expected);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(registry.twist_options_default(), expected);
    let mut other = Document::default();
    let other_id = other
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 2.5).unwrap()))
        .unwrap();
    other
        .select_object(other_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut other, "_-Twist 0,0,0 0,0,10 90")
        .unwrap();
    let Geometry::Point(point) = other.object(other_id).unwrap().geometry() else {
        panic!();
    };
    let angle = std::f64::consts::FRAC_PI_8;
    near(
        *point,
        Point3::try_new(
            2. * angle.cos() - angle.sin(),
            2. * angle.sin() + angle.cos(),
            2.5,
        )
        .unwrap(),
        1e-7,
        "new document Infinite",
    );
    assert_eq!(
        CommandRegistry::with_builtins().twist_options_default(),
        TwistOptions::default()
    );
}

#[test]
fn repeated_twist_copies_match_four_native_batches_and_share_one_history_entry() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/twist_repeat_command.json"
    ))
    .unwrap();
    let o: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/twist_repeat_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (op, row) in f["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(o["results"].as_array().unwrap())
    {
        let (mut doc, ids) = setup(op, &row["value"]["before"]);
        let mut group = doc.begin_history_group("Twist").unwrap();
        for degrees in std::iter::once(op["degrees"].as_f64().unwrap()).chain(
            op["angles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap()),
        ) {
            registry
                .execute_in_history_group(
                    &mut doc,
                    &format!("Twist 0,0,0 0,0,10 {degrees} Copy=Yes"),
                    CommandContext::default(),
                    &mut group,
                )
                .unwrap();
        }
        let expected = &row["value"]["after"];
        assert_eq!(
            doc.objects().count(),
            expected["objects"].as_array().unwrap().len()
        );
        for (obj, record) in doc.objects().zip(expected["objects"].as_array().unwrap()) {
            let Geometry::Point(point) = obj.geometry() else {
                panic!();
            };
            near(
                *point,
                p(&record["geometry"]["points"][0]),
                1e-11,
                "repeated Twist",
            );
        }
        assert_eq!(
            doc.groups().count(),
            expected["groups"].as_array().unwrap().len()
        );
        assert_metadata(&doc, expected, "repeated Twist after");
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), ids.len());
        assert_metadata(&doc, &row["value"]["undo"], "repeated Twist Undo");
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.objects().count(),
            expected["objects"].as_array().unwrap().len()
        );
        assert_metadata(&doc, &row["value"]["redo"], "repeated Twist Redo");
    }
}

#[test]
fn failed_mixed_point_and_collapsing_mesh_twist_is_atomic() {
    let registry = CommandRegistry::with_builtins();
    for copy in [false, true] {
        let mut doc = Document::default();
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
            .unwrap();
        let half = 0.5_f64.sqrt();
        let vertices = [[1., 0., 0.], [half, -half, 5.], [0., -1., 10.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect();
        let mesh = doc
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(vertices, vec![[0, 1, 2]], doc.tolerance()).unwrap(),
            ))
            .unwrap();
        let group = doc.add_group(None, [point, mesh]).unwrap();
        doc.select_objects_direct([point, mesh], SelectionMode::Replace)
            .unwrap();
        let before = doc
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        let label = doc.undo_label().map(str::to_owned);
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "Twist 0,0,0 0,0,10 90 Infinite=Yes Copy={}",
                        if copy { "Yes" } else { "No" }
                    )
                )
                .is_err()
        );
        assert_eq!(
            doc.objects()
                .map(|o| (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec()
                ))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(doc.undo_label(), label.as_deref());
        assert_eq!(
            doc.group(group)
                .unwrap()
                .members()
                .collect::<std::collections::BTreeSet<_>>(),
            [point, mesh]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
        assert_eq!(doc.selected_object_count(), 2);
        assert_eq!(registry.twist_options_default(), TwistOptions::default());
        assert_eq!(registry.transform_scalar_default("Twist"), None);
    }
}
