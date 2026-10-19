fn audit_matrix(root: &std::path::Path, matrix: &str, count: usize) {
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("observations/insert_knot_{matrix}.json")))
            .unwrap(),
    )
    .unwrap();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    let mut failures = Vec::new();
    for row in rows {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(
                &row["value"],
                &root.join(format!("fixtures/insert_knot_{matrix}")),
            )
        })) {
            Ok(gap) => eprintln!(
                "INSERT_KNOT_COMMAND_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            Err(_) => {
                eprintln!("INSERT_KNOT_COMMAND_MISMATCH {matrix}/{name}");
                failures.push(name);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Native COMMAND mismatches: {failures:?}"
    );
}
use serde_json::Value;
use viboceros_command::{
    CommandRegistry,
    insert_knot::{Draft, Options},
    open_3dm_with_views,
};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn replay(v: &Value, root: &std::path::Path) -> f64 {
    let r = &v["recipe"];
    let name = r["id"].as_str().unwrap();
    let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
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
    let before = doc.object(id).unwrap().clone();
    let range = r["subcurve"].as_array().map(|x| {
        let c = before.geometry().curve_ref().unwrap();
        [
            c.parameter_at(x[0].as_f64().unwrap()).unwrap(),
            c.parameter_at(x[1].as_f64().unwrap()).unwrap(),
        ]
    });
    if let Ok(mut draft) = Draft::new(&doc, id, range) {
        let mut options = Options::default();
        for (key, default) in [
            ("Direction", "U"),
            ("Symmetrical", "No"),
            ("Midpoints", "No"),
        ] {
            let name = key.to_ascii_lowercase();
            let raw = match &r[&name] {
                Value::Bool(b) => {
                    if *b {
                        "Yes"
                    } else {
                        "No"
                    }
                }
                Value::String(s) => s.as_str(),
                _ => default,
            };
            options.accept(&format!("{key}={raw}")).unwrap();
        }
        let mut picks = v["picks"].as_array().unwrap().iter();
        for action in r["actions"].as_array().unwrap() {
            if let Some(option) = action.as_str() {
                match option {
                    "Automatic" => draft.automatic(options).unwrap(),
                    "Undo" => {
                        draft.undo();
                    }
                    _ => {
                        assert!(options.accept(option).unwrap());
                    }
                }
            } else if let Some(t) = action["parameter"].as_f64() {
                draft.parameter(t, options).unwrap();
            } else {
                draft.point(point(picks.next().unwrap()), options).unwrap();
            }
            assert_eq!(doc.object(id), Some(&before), "{name}: immutable source");
            assert!(!doc.can_undo());
        }
        if r["cancel"] != true {
            let mut command = String::from("InsertKnot Points ");
            if let Some([a, b]) = range {
                command.push_str(&format!("SubCrv={id},{a},{b} "));
            }
            for token in v["tokens"].as_array().unwrap() {
                let token = token.as_str().unwrap();
                if token != "_Enter" {
                    command.push_str(token);
                    command.push(' ');
                }
            }
            if range.is_none() {
                doc.select_object(id, SelectionMode::Replace).unwrap();
            }
            let registry = CommandRegistry::with_builtins();
            if r["preselect"] == false || range.is_some() {
                registry
                    .execute_postselected(&mut doc, &command, Default::default())
                    .unwrap();
            } else {
                registry.execute(&mut doc, &command).unwrap();
            }
            if draft.picks() > 0 {
                let output = if range.is_some() {
                    doc.objects().find(|o| o.id() != id).unwrap()
                } else {
                    doc.object(id).unwrap()
                };
                assert_eq!(
                    output.geometry(),
                    draft.geometry(),
                    "{name}: script and staged API agree"
                );
            }
        }
    } else {
        assert_eq!(name, "polysurface");
    }
    let mut gap = comparison::check(&doc, &v["after"], id, name);
    if !v["undo"].is_null() {
        assert_eq!(doc.undo_label(), Some("InsertKnot"));
        doc.undo().unwrap();
        gap = gap.max(comparison::check(&doc, &v["undo"], id, name));
        assert_eq!(doc.object(id), Some(&before));
        doc.redo().unwrap();
        gap = gap.max(comparison::check(&doc, &v["redo"], id, name));
    }
    assert!(gap <= 1e-8, "{name}: {gap}");
    gap
}
#[test]
fn all_native_insert_knot_actions_geometry_metadata_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    audit_matrix(&root, "command", 156);
    audit_matrix(&root, "followup", 49);
}

#[test]
fn insert_knot_native_symmetric_complement_and_center_picks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    audit_matrix(&root, "symmetry", 19);
}

#[test]
fn standalone_surfaces_use_the_selected_backend_for_all_native_actions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/insert_knot_command.json")).unwrap(),
    )
    .unwrap();
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        let v = &row["value"];
        let r = &v["recipe"];
        if !matches!(r["family"].as_str(), Some("surface" | "surface_rational"))
            || r["cancel"] == true
        {
            continue;
        }
        let name = row["id"].as_str().unwrap();
        let mut doc = open_3dm_with_views(
            root.join("fixtures/insert_knot_command")
                .join(format!("{name}.3dm"))
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
        let viboceros_document::Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!("source surface")
        };
        let surface = brep.faces()[0].surface().clone();
        doc.replace_object_geometries([(id, viboceros_document::Geometry::NurbsSurface(surface))])
            .unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        doc.clear_history().unwrap();
        let mut command = String::from("InsertKnot Points ");
        for token in v["tokens"].as_array().unwrap() {
            let token = token.as_str().unwrap();
            if token != "_Enter" {
                command.push_str(token);
                command.push(' ');
            }
        }
        CommandRegistry::with_builtins()
            .execute(&mut doc, &command)
            .unwrap();
        let viboceros_document::Geometry::NurbsSurface(actual) = doc.object(id).unwrap().geometry()
        else {
            panic!("result surface")
        };
        let gap = comparison::surface(actual, &v["after"][0]["geometry"]["surfaces"][0]);
        assert!(gap <= 1e-8, "{name}: {gap}");
        count += 1;
        eprintln!(
            "INSERT_KNOT_SURFACE_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":gap})
        );
    }
    assert_eq!(count, 30);
}

#[test]
fn point_script_options_are_case_insensitive_and_fail_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = viboceros_document::Document::default();
    registry.execute(&mut doc, "Line 0,0,0 6,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    let original = doc.object(id).unwrap().clone();
    registry
        .execute(&mut doc, "insertknot symmetrical=yes automatic")
        .unwrap();
    assert_eq!(
        doc.object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap()
            .knots(),
        &[0., 0., 3., 6., 6.]
    );
    doc.undo().unwrap();
    assert_eq!(doc.object(id), Some(&original));
    assert!(doc.can_redo());
    assert!(
        registry
            .execute(&mut doc, "InsertKnot Points 2 NaN")
            .is_err()
    );
    assert_eq!(doc.object(id), Some(&original));
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    registry
        .execute(&mut doc, "InsertKnot Points Symmetrical=No 2 4 Undo")
        .unwrap();
    assert_eq!(
        doc.object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap()
            .knots(),
        &[0., 0., 2., 6., 6.]
    );
}
