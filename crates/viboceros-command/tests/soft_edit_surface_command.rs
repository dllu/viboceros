use serde_json::Value;
use viboceros_command::{
    CommandRegistry, open_3dm_with_views,
    soft_edit_surface::{Draft, Options},
};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[allow(dead_code)]
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[path = "support/soft_edit_surface.rs"]
mod surface_comparison;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn token(v: &Value) -> String {
    let [x, y, z] = point(v).to_array();
    format!("{x},{y},{z}")
}
#[test]
fn native_surface_soft_edits_previews_metadata_selection_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    for (dataset, count) in [
        ("soft_edit_surface_command", 108),
        ("soft_edit_surface_shapes", 105),
        ("soft_edit_surface_seams", 55),
    ] {
        let capture: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{dataset}.json"))).unwrap(),
        )
        .unwrap();
        let rows = capture["results"].as_array().unwrap();
        assert_eq!(rows.len(), count);
        for row in rows {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let recipe = &v["recipe"];
            let result = std::panic::catch_unwind(|| {
                let mut doc = open_3dm_with_views(
                    root.join(format!("fixtures/{dataset}/{name}.3dm"))
                        .to_str()
                        .unwrap(),
                )
                .unwrap()
                .0;
                let id = v["source_id"]
                    .as_str()
                    .unwrap()
                    .parse::<ObjectId>()
                    .unwrap();
                doc.clear_selection();
                if v["before"][0]["selected"] == true {
                    doc.select_object(id, SelectionMode::Replace).unwrap();
                }
                doc.clear_history().unwrap();
                assert!(
                    surface_comparison::check(&doc, &v["before"], id, name) <= 1e-8,
                    "{name}: import"
                );
                let before = doc.objects().cloned().collect::<Vec<_>>();
                let mut options = Options::default();
                for w in [
                    format!("U_Distance={}", recipe["distance"][0]),
                    format!("V_Distance={}", recipe["distance"][1]),
                    format!(
                        "FixEdges={}",
                        if recipe["fix_edges"] == true {
                            "Yes"
                        } else {
                            "No"
                        }
                    ),
                    format!("Copy={}", if recipe["copy"] == true { "Yes" } else { "No" }),
                    format!(
                        "DirConstraint={}",
                        recipe["constraint"].as_str().unwrap_or("None")
                    ),
                ] {
                    assert!(options.accept(&w).unwrap());
                }
                let mut draft = Draft::new(&doc, id).unwrap();
                draft.set_options(options).unwrap();
                draft.point(point(&v["reference"])).unwrap();
                let preview = draft.preview(point(&v["target"])).unwrap().unwrap();
                assert_eq!(
                    doc.objects().cloned().collect::<Vec<_>>(),
                    before,
                    "{name}: preview changed model"
                );
                let registry = CommandRegistry::with_builtins();
                let changed = v["undo"].is_array();
                if recipe["cancel"] != true || recipe["cancel_after"] == true {
                    doc.select_object(id, SelectionMode::Replace).unwrap();
                    let input = format!(
                        "SoftEditSrf {} {} {}",
                        token(&v["reference"]),
                        token(&v["target"]),
                        options.arguments()
                    );
                    if recipe["preselect"] == false {
                        registry
                            .execute_postselected(&mut doc, &input, Default::default())
                            .unwrap();
                    } else {
                        registry.execute(&mut doc, &input).unwrap();
                    }
                    if changed {
                        let object = if options.copy {
                            doc.objects().find(|o| o.id() != id).unwrap()
                        } else {
                            doc.object(id).unwrap()
                        };
                        assert_eq!(object.geometry(), &preview, "{name}: API and preview");
                    }
                }
                assert_eq!(doc.object(id).unwrap().attributes(), before[0].attributes());
                assert_eq!(doc.object(id).unwrap().group_ids(), before[0].group_ids());
                let mut gap = surface_comparison::check(&doc, &v["after"], id, name);
                gap = gap.max(surface_comparison::check_boundaries(&doc, v, id, name));
                if changed {
                    assert_eq!(doc.undo_label(), Some("SoftEditSrf"));
                    registry.execute(&mut doc, "Undo").unwrap();
                    gap = gap.max(surface_comparison::check(&doc, &v["undo"], id, name));
                    registry.execute(&mut doc, "Redo").unwrap();
                    gap = gap.max(surface_comparison::check(&doc, &v["redo"], id, name));
                }
                eprintln!(
                    "SOFT_EDIT_SURFACE_API_AUDIT {}",
                    serde_json::json!({"dataset":dataset,"id":name,"maximum_error":gap})
                );
                let closed_v =
                    viboceros_command::surface_reference::surface(before[0].geometry(), None)
                        .unwrap()
                        .0
                        .is_closed_v()
                        .unwrap();
                assert!(
                    gap <= if closed_v { 1e-7 } else { 5e-8 },
                    "{name}: difference {gap}"
                );
            });
            if result.is_err() {
                failures.push(name.to_owned());
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Surface soft edit differences: {failures:?}"
    );
}

#[test]
fn native_repeated_surface_offsets_copies_and_copy_toggles() {
    assert_repeated_surface_workflows("soft_edit_surface_sequence", 16);
}

#[test]
fn native_repeated_closed_surface_workflows() {
    assert_repeated_surface_workflows("soft_edit_surface_closed_sequence", 12);
}

fn assert_repeated_surface_workflows(stem: &str, count: usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("observations/{stem}.json"))).unwrap(),
    )
    .unwrap();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    for row in rows {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let mut doc = open_3dm_with_views(
            root.join(format!("fixtures/{stem}/{name}.3dm"))
                .to_str()
                .unwrap(),
        )
        .unwrap()
        .0;
        let id = v["source_id"]
            .as_str()
            .unwrap()
            .parse::<ObjectId>()
            .unwrap();
        doc.clear_selection();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let mut options = Options {
            distances: [4., 4.],
            constraint: viboceros_command::soft_edit_surface::Constraint::None,
            copy: recipe["copy"] == true,
            ..Default::default()
        };
        let mut draft = Draft::new(&doc, id).unwrap();
        for (i, pair) in v["all_points"]
            .as_array()
            .unwrap()
            .chunks_exact(2)
            .enumerate()
        {
            if i > 0
                && let Some(copy) = recipe["moves"][i - 1]["copy"].as_bool()
            {
                options.copy = copy;
            }
            draft.set_options(options).unwrap();
            draft.point(point(&pair[0])).unwrap();
            assert!(draft.preview(point(&pair[1])).unwrap().is_some());
            draft.point(point(&pair[1])).unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: staged mutation"
            );
        }
        if let Some(last) = v["all_points"]
            .as_array()
            .unwrap()
            .last()
            .filter(|_| v["all_points"].as_array().unwrap().len() % 2 == 1)
        {
            draft.point(point(last)).unwrap();
            draft.cancel_offset();
        }
        doc.begin_transaction("SoftEditSrf").unwrap();
        draft.commit(&mut doc, false).unwrap();
        doc.commit_transaction().unwrap();
        let mut gap = surface_comparison::check(&doc, &v["after"], id, name);
        gap = gap.max(surface_comparison::check_boundaries(&doc, v, id, name));
        let registry = CommandRegistry::with_builtins();
        if v["undo"].is_array() {
            registry.execute(&mut doc, "Undo").unwrap();
            gap = gap.max(surface_comparison::check(&doc, &v["undo"], id, name));
            registry.execute(&mut doc, "Redo").unwrap();
            gap = gap.max(surface_comparison::check(&doc, &v["redo"], id, name));
        } else {
            assert!(doc.undo_label().is_none());
        }
        eprintln!(
            "SOFT_EDIT_SURFACE_SEQUENCE_API_AUDIT {}",
            serde_json::json!({"dataset":stem,"id":name,"maximum_error":gap})
        );
        assert!(
            gap <= if stem == "soft_edit_surface_closed_sequence" {
                2e-7
            } else {
                1e-7
            },
            "{name}: difference {gap}"
        );
    }
}

#[test]
fn malformed_batches_and_stale_plans_preserve_model_and_redo() {
    use viboceros_document::{Document, Geometry};
    use viboceros_geometry::NurbsSurface;
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [[0., 0., 0.], [4., 0., 0.], [0., 4., 0.], [4., 4., 1.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "SoftEditSrf 0,0,0 0,0,1 DirConstraint=None")
        .unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let redo = doc.redo_label().map(str::to_owned);
    for input in [
        "SoftEditSrf 0,0,0",
        "SoftEditSrf 0,0,0 0,0,2 U_Distance=0",
        "SoftEditSrf 0,0,0 0,0,2 V_Distance=NaN",
        "SoftEditSrf 0,0,0 0,0,2 DirConstraint=Delete",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
    let mut draft = Draft::new(&doc, id).unwrap();
    draft.point(Point3::try_new(0., 0., 0.).unwrap()).unwrap();
    doc.replace_object_geometries([(
        id,
        Geometry::NurbsSurface(
            surface
                .prepare_soft_edit([0., 0.], [2., 2.], false, doc.tolerance())
                .unwrap()
                .apply(viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap())
                .unwrap(),
        ),
    )])
    .unwrap();
    let changed = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        draft
            .scene(&doc, Some(Point3::try_new(0., 0., 2.).unwrap()))
            .is_err()
    );
    assert!(draft.commit(&mut doc, false).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), changed);
}

#[test]
fn native_public_sdk_surface_edit_transitions() {
    use viboceros_document::{Document, Geometry};
    use viboceros_geometry::{NurbsSurface, Tolerance, Vector3, WeightedPoint3};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/soft_edit_surface_repeat_sdk.json"))
            .unwrap(),
    )
    .unwrap();
    let request: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/soft_edit_surface_repeat_sdk.json")).unwrap(),
    )
    .unwrap();
    let definition = |v: &Value| {
        NurbsSurface::try_new_rational(
            v["degree"][0].as_u64().unwrap() as usize,
            v["degree"][1].as_u64().unwrap() as usize,
            v["control_count"][0].as_u64().unwrap() as usize,
            v["control_count"][1].as_u64().unwrap() as usize,
            v["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap())
                        .unwrap()
                })
                .collect(),
            serde_json::from_value(v["knots_u"].clone()).unwrap(),
            serde_json::from_value(v["knots_v"].clone()).unwrap(),
        )
        .unwrap()
    };
    let mut failures = Vec::new();
    for (row, op) in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(request["operations"].as_array().unwrap())
    {
        for variant in row["value"]["variants"].as_array().unwrap() {
            for (index, step) in variant["steps"].as_array().unwrap().iter().enumerate() {
                let source = definition(&step["source"]);
                let expected = definition(&step["result"]);
                let uv = serde_json::from_value(step["parameter"].clone()).unwrap();
                let delta = Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(step["delta"].clone()).unwrap(),
                )
                .unwrap();
                let actual = source
                    .prepare_soft_edit(uv, [4., 4.], false, Tolerance::DEFAULT)
                    .unwrap()
                    .apply(delta)
                    .unwrap();
                let mut gap = 0_f64;
                for (a, b) in actual
                    .control_points()
                    .iter()
                    .zip(expected.control_points())
                {
                    gap = gap.max(a.point().distance_to(b.point()).unwrap());
                }
                eprintln!(
                    "SOFT_EDIT_SURFACE_SDK_AUDIT {}",
                    serde_json::json!({"id":row["id"],"mode":variant["mode"],"step":index,"maximum_error":gap})
                );
                if gap > 1e-7 {
                    failures.push(format!(
                        "{} {} step {index}: kernel {gap}",
                        row["id"], variant["mode"]
                    ));
                }
                if variant["mode"] == "updated" {
                    let mut doc = Document::default();
                    let id = doc.add_geometry(Geometry::NurbsSurface(source)).unwrap();
                    let mut draft = Draft::new(&doc, id).unwrap();
                    let options = Options {
                        distances: [4., 4.],
                        constraint: viboceros_command::soft_edit_surface::Constraint::None,
                        ..Default::default()
                    };
                    draft.set_options(options).unwrap();
                    draft.point(point(&op["points"][2 * index])).unwrap();
                    let anchor_gap = draft
                        .anchor()
                        .unwrap()
                        .distance_to(point(&step["anchor"]))
                        .unwrap();
                    if anchor_gap > 1e-7 {
                        failures.push(format!("{} step {index}: getter {anchor_gap}", row["id"]));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "SDK transition differences: {failures:?}"
    );
}
