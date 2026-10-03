use super::*;
use crate::morph_test_support::*;
use serde_json::Value;
use viboceros_document::SelectionMode;

fn options(op: &Value) -> TaperOptions {
    TaperOptions {
        copy: op["copy"].as_bool().unwrap(),
        rigid: op["rigid"].as_bool().unwrap(),
        flat: op["flat"].as_bool().unwrap(),
        infinite: op["infinite"].as_bool().unwrap(),
        preserve_structure: op["preserve"].as_bool().unwrap(),
    }
}
fn distance(value: &Value) -> TaperDistance {
    if let Some(v) = value.as_f64() {
        TaperDistance::Number(v)
    } else {
        TaperDistance::Point(p(value))
    }
}
fn context(op: &Value) -> CommandContext {
    let (x, y) = match op["cplane"].as_str().unwrap_or("WorldXY") {
        "WorldYZ" => ([0., 1., 0.], [0., 0., 1.]),
        "WorldZX" => ([0., 0., 1.], [1., 0., 0.]),
        _ => ([1., 0., 0.], [0., 1., 0.]),
    };
    CommandContext {
        construction_plane: Frame3::try_from_directions(
            Point3::try_from([0.; 3]).unwrap(),
            Vector3::try_from(x).unwrap(),
            Vector3::try_from(y).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    }
}
fn command_axis(op: &Value) -> (Point3, Point3) {
    let endpoints = match op["axis"].as_str() {
        Some("Skew") => Some(([1., 2., 3.], [4., -2., 11.])),
        Some("X") => Some(([0.; 3], [10., 0., 0.])),
        Some("Y") => Some(([0.; 3], [0., 10., 0.])),
        _ => None,
    };
    endpoints
        .map(|(a, b)| (Point3::try_from(a).unwrap(), Point3::try_from(b).unwrap()))
        .unwrap_or_else(|| axis(op))
}

#[test]
fn retained_actual_taper_commands_match_geometry_attributes_groups_and_history() {
    for (fixtures, observations) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_geometry_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_geometry_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_fitting_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_fitting_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_command_followup.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_command_followup.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_command_boundary.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_command_boundary.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_command_threshold.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/taper_command_threshold.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/taper_identity_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/taper_identity_command.json"),
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
            let registry = CommandRegistry::with_builtins();
            let label = op["id"].as_str().unwrap();
            assert_eq!(op["id"], row["id"]);
            let v = &row["value"];
            let result = v["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == "Taper")
                .unwrap()["result"]
                .as_str()
                .unwrap();
            let (mut doc, ids) = setup(op, &v["before"]);
            let originals = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().geometry().clone())
                .collect::<Vec<_>>();
            let (start, end) = command_axis(op);
            let options = options(op);
            let sources = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let mut group = doc.begin_history_group("Taper").unwrap();
            let mut accepted = false;
            for target in std::iter::once(&op["end_distance"])
                .chain(op["targets"].as_array().into_iter().flatten())
            {
                let r = registry.execute_in_history_group(
                    &mut doc,
                    &format!(
                        "Taper {} {} {} {} {} Sources={sources}",
                        format_point(start),
                        format_point(end),
                        distance(&op["start_distance"]).command_argument(),
                        distance(target).command_argument(),
                        options.command_options()
                    ),
                    context(op),
                    &mut group,
                );
                if result == "Cancel" && !options.copy {
                    assert!(r.is_err(), "{label}: expected invalid distance");
                } else {
                    r.unwrap_or_else(|e| panic!("{label}: {e}"));
                    accepted = true;
                }
            }
            // Radius diagnostics append _Cancel to end a rejected getter. When
            // Taper has already succeeded, that later selection cleanup belongs
            // to Cancel. All other redo metadata still belongs to Taper.
            let trailing_cancel = v["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["name"] == "Cancel")
                && result == "Success";
            let epsilon = if options.rigid {
                1e-7
            } else if op["shape"] == "Points"
                || op["shape"] == "Mesh"
                || (options.preserve_structure && op["shape"] != "Box")
            {
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
                if trailing_cancel && phase == "redo" {
                    let mut without_cancel_selection = expected.clone();
                    for (object, row) in doc
                        .objects()
                        .zip(without_cancel_selection["objects"].as_array_mut().unwrap())
                    {
                        row["selected"] = doc.is_selected(object.id()).into();
                    }
                    assert_metadata(
                        &doc,
                        &without_cancel_selection,
                        &format!("{label}: {phase}"),
                    );
                } else {
                    assert_metadata(&doc, expected, &format!("{label}: {phase}"));
                }
                for (i, (actual, record)) in doc
                    .objects()
                    .zip(expected["objects"].as_array().unwrap())
                    .enumerate()
                {
                    compare_geometry(
                        actual.geometry(),
                        &record["geometry"],
                        &originals[i % ids.len()],
                        &v["before"]["objects"][i % ids.len()]["geometry"],
                        epsilon,
                        &format!("{label}: {phase}"),
                    );
                    if op["id"].as_str().unwrap().starts_with("taper-identity-") && phase != "undo"
                    {
                        compare_preserved_structure(
                            actual.geometry(),
                            &record["geometry"],
                            &originals[i % ids.len()],
                            label,
                        );
                        assert_eq!(
                            actual.geometry(),
                            &originals[i % ids.len()],
                            "{label}: identity structure"
                        );
                    }
                    if accepted
                        && phase != "undo"
                        && options.preserve_structure
                        && !options.rigid
                        && op["shape"] != "Box"
                    {
                        compare_preserved_structure(
                            actual.geometry(),
                            &record["geometry"],
                            &originals[i % ids.len()],
                            label,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_inputs_and_mesh_collapse_leave_the_whole_document_unchanged() {
    let registry = CommandRegistry::with_builtins();
    for copy in [false, true] {
        let mut doc = Document::default();
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_from([2., 1., 5.]).unwrap()))
            .unwrap();
        let mesh = doc
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
                        .map(|p| Point3::try_from(p).unwrap())
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
        for input in [
            "Taper 0,0,0 0,0,0 2 1".to_string(),
            "Taper 0,0,0 0,0,10 0 1".to_string(),
            "Taper 0,0,0 0,0,10 2 0".to_string(),
            "Taper 0,0,0 0,0,10 2 NaN".to_string(),
            "Taper 0,0,0 0,0,10 0,0,5 1".to_string(),
            "Taper 0,0,0 0,0,10 2 1 Flat=Yes Flat=No".to_string(),
            // The triangle lies at the zero-scale crossing of the infinite map.
            format!(
                "Taper 0,0,-10 0,0,10 2 -2 Infinite=Yes Copy={}",
                if copy { "Yes" } else { "No" }
            ),
        ] {
            assert!(registry.execute(&mut doc, &input).is_err(), "{input}");
            assert_eq!(state(&doc), before);
            assert_eq!(doc.undo_label(), label.as_deref());
            assert_eq!(doc.selected_object_count(), 2);
            assert_eq!(registry.taper_options_default(), TaperOptions::default());
        }
    }
}
