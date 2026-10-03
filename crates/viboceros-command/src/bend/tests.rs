use super::*;
use crate::morph_test_support::*;
use serde_json::Value;
use viboceros_document::SelectionMode;

fn options(op: &Value) -> BendOptions {
    BendOptions {
        copy: op["copy"].as_bool().unwrap(),
        rigid: op["rigid"].as_bool().unwrap(),
        limit_to_spine: op["limit_to_spine"].as_bool().unwrap(),
        symmetric: op["symmetric"].as_bool().unwrap(),
        preserve_structure: op["preserve"].as_bool().unwrap(),
        non_attenuated: op["non_attenuated"].as_bool().unwrap(),
        // The geometry capture explicitly resets the numeric angle to zero
        // for its independent through-point cases.
        angle: Some(op["angle"].as_f64().unwrap_or(0.)),
    }
}

#[test]
fn actual_bend_geometry_attributes_groups_and_history_match_seventy_six_native_cases() {
    let registry = CommandRegistry::with_builtins();
    for (fixtures, observations) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_geometry_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_geometry_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_fitting_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_fitting_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_limited_rigid_command.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/bend_limited_rigid_command.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/bend_rigid_midpoint_command.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/bend_rigid_midpoint_command.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/bend_rigid_frame_boundary_command.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/bend_rigid_frame_boundary_command.json"
            ),
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
            let source_geometry = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().geometry().clone())
                .collect::<Vec<_>>();
            let (start, end) = axis(op);
            let options = options(op);
            let sources = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let mut history = doc.begin_history_group("Bend").unwrap();
            for through in std::iter::once(&op["through"])
                .chain(op["targets"].as_array().into_iter().flatten())
            {
                registry
                    .execute_in_history_group(
                        &mut doc,
                        &format!(
                            "Bend {} {} {} {} Sources={sources}",
                            format_point(start),
                            format_point(end),
                            format_point(p(through)),
                            options.command_options()
                        ),
                        CommandContext::default(),
                        &mut history,
                    )
                    .unwrap_or_else(|e| panic!("{label}: {e}"));
            }
            let epsilon = if options.rigid {
                1e-7
            } else if op["shape"] == "Points"
                || (options.preserve_structure && op["shape"] != "Box")
            {
                1e-11
            } else {
                2. * op["tolerance"].as_f64().unwrap_or(1e-5).max(1e-5)
            };
            for phase in ["after", "undo", "redo"] {
                if phase == "undo" {
                    registry.execute(&mut doc, "Undo").unwrap();
                } else if phase == "redo" {
                    registry.execute(&mut doc, "Redo").unwrap();
                }
                let expected = &row["value"][phase];
                assert_eq!(
                    doc.objects().count(),
                    expected["objects"].as_array().unwrap().len(),
                    "{label}: {phase}"
                );
                assert_metadata(&doc, expected, &format!("{label}: {phase}"));
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
                        &format!("{label}: {phase}"),
                    );
                    if phase != "undo"
                        && options.preserve_structure
                        && !options.rigid
                        && op["shape"] != "Box"
                    {
                        compare_preserved_structure(
                            actual.geometry(),
                            &record["geometry"],
                            &source_geometry[i % ids.len()],
                            label,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_bend_inputs_and_collapsing_meshes_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    for copy in [false, true] {
        let mut doc = Document::default();
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
            .unwrap();
        let r = 10. / std::f64::consts::FRAC_PI_2;
        let mesh = doc
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    [[r, 0., 1.], [r, 0., 2.], [r, 1., 3.]]
                        .map(|v| Point3::try_from(v).unwrap())
                        .to_vec(),
                    vec![[0, 1, 2]],
                    doc.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        doc.add_group(None, [point, mesh]).unwrap();
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
        for input in [
            "Bend 0,0,0 0,0,0 10,0,10".to_owned(),
            "Bend 0,0,0 0,0,10 0,0,5".to_owned(),
            "Bend 0,0,0 0,0,10 10,0,10 Angle=NaN".to_owned(),
            "Bend 0,0,0 0,0,10 10,0,10 Angle=-90".to_owned(),
            "Bend 0,0,0 0,0,10 10,0,10 Rigid=Yes Rigid=No".to_owned(),
            format!(
                "Bend 0,0,0 0,0,10 10,0,10 Angle=90 LimitToSpine=Yes NonAttenuated=Yes Copy={}",
                if copy { "Yes" } else { "No" }
            ),
        ] {
            assert!(registry.execute(&mut doc, &input).is_err(), "{input}");
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
            assert_eq!(doc.selected_object_count(), 2);
            assert_eq!(registry.bend_options_default(), BendOptions::default());
        }
    }
}

#[test]
fn angle_zero_selects_through_point_mode_and_preferences_outlive_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 10.).unwrap()))
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut doc,
            "Bend 0,0,0 0,0,10 10,0,10 Angle=0 LimitToSpine=Yes NonAttenuated=Yes",
        )
        .unwrap();
    let Geometry::Point(result) = doc.object(id).unwrap().geometry() else {
        panic!();
    };
    assert!(
        result
            .distance_to(Point3::try_new(0., 0., 10.).unwrap())
            .unwrap()
            > 1.
    );
    let options = BendOptions {
        limit_to_spine: true,
        non_attenuated: true,
        ..BendOptions::default()
    };
    assert_eq!(registry.bend_options_default(), options);
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(registry.bend_options_default(), options);
    assert_eq!(registry.transform_scalar_default("Bend"), None);
    assert_eq!(
        CommandRegistry::with_builtins().bend_options_default(),
        BendOptions::default()
    );
}
