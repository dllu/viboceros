fn audit_matrix(root: &std::path::Path, matrix: &str, count: usize) {
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("observations/remove_knot_{matrix}.json")))
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
                &root.join(format!("fixtures/remove_knot_{matrix}")),
            )
        })) {
            Ok(gap) => eprintln!(
                "REMOVE_KNOT_COMMAND_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            Err(_) => {
                eprintln!("REMOVE_KNOT_COMMAND_MISMATCH {matrix}/{name}");
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
    CommandRegistry, open_3dm_with_views,
    remove_knot::{Draft, Options},
};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[path = "support/remove_knot.rs"]
mod topology;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn replay(v: &Value, root: &std::path::Path) -> f64 {
    let r = &v["recipe"];
    let name = r["id"].as_str().unwrap();
    let (mut doc, _, _, views) =
        open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap()).unwrap();
    let projection = viboceros_command::picking::Projection::from_view(
        &views.iter().find(|v| v.active).unwrap().camera,
    )
    .unwrap();
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
        options
            .accept(&format!(
                "Direction={}",
                r["direction"].as_str().unwrap_or("U")
            ))
            .unwrap();
        let mut picks = v["picks"].as_array().unwrap().iter();
        for action in r["actions"].as_array().unwrap() {
            if let Some(option) = action.as_str() {
                match option {
                    "Undo" => {
                        draft.undo();
                    }
                    _ => {
                        assert!(options.accept(option).unwrap());
                    }
                }
            } else if let Some(t) = action["parameter"].as_f64() {
                draft.parameter(t).unwrap();
            } else {
                draft
                    .point_in_view(point(picks.next().unwrap()), options, Some(projection))
                    .unwrap();
            }
            assert_eq!(doc.object(id), Some(&before), "{name}: immutable source");
            assert!(!doc.can_undo());
        }
        if r["cancel"] != true {
            let mut command = String::from("RemoveKnot Points ");
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
            registry
                .execute_with_pick_projection(
                    &mut doc,
                    &command,
                    Default::default(),
                    r["preselect"] == false || range.is_some(),
                    projection,
                )
                .unwrap();
            {
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
        assert!(v["before"][0]["geometry"]["faces"].as_u64().unwrap() > 1);
        assert!(!v["success"].as_bool().unwrap());
    }
    if v["success"] == false {
        doc.clear_selection();
    }

    let mut gap = topology::check(&doc, &v["after"], id, name);
    if !v["undo"].is_null() {
        assert_eq!(doc.undo_label(), Some("RemoveKnot"));
        doc.undo().unwrap();
        gap = gap.max(topology::check(&doc, &v["undo"], id, name));
        assert_eq!(doc.object(id), Some(&before));
        doc.redo().unwrap();
        gap = gap.max(topology::check(&doc, &v["redo"], id, name));
    }
    assert!(gap <= 1e-8, "{name}: {gap}");
    gap
}
#[test]
fn all_native_remove_knot_actions_geometry_metadata_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    audit_matrix(&root, "command", 149);
}

#[test]
fn standalone_surfaces_use_the_selected_backend_for_all_native_actions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_knot_command.json")).unwrap(),
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
        let (mut doc, _, _, views) = open_3dm_with_views(
            root.join("fixtures/remove_knot_command")
                .join(format!("{name}.3dm"))
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let projection = viboceros_command::picking::Projection::from_view(
            &views.iter().find(|v| v.active).unwrap().camera,
        )
        .unwrap();
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
        let mut command = String::from("RemoveKnot Points ");
        for token in v["tokens"].as_array().unwrap() {
            let token = token.as_str().unwrap();
            if token != "_Enter" {
                command.push_str(token);
                command.push(' ');
            }
        }
        CommandRegistry::with_builtins()
            .execute_with_pick_projection(&mut doc, &command, Default::default(), false, projection)
            .unwrap();
        let viboceros_document::Geometry::NurbsSurface(actual) = doc.object(id).unwrap().geometry()
        else {
            panic!("result surface")
        };
        let gap = topology::surface(actual, &v["after"][0]["geometry"]["surfaces"][0]);
        assert!(gap <= 1e-8, "{name}: {gap}");
        count += 1;
        eprintln!(
            "REMOVE_KNOT_SURFACE_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":gap})
        );
    }
    assert_eq!(count, 40);
}

#[test]
fn scripted_removal_rejects_invalid_options_atomically_and_keeps_legacy_parameters() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = viboceros_document::Document::default();
    registry.execute(&mut doc, "Line 0,0,0 6,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    registry.execute(&mut doc, "InsertKnot Points 2 4").unwrap();
    let before = doc.object(id).unwrap().clone();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    doc.undo().unwrap();
    assert!(doc.can_redo());
    for text in [
        "RemoveKnot Points 2,0,0 NaN",
        "RemoveKnot Points Direction=Both 2,0,0",
        "RemoveKnot Points Symmetrical=Yes 2,0,0",
    ] {
        assert!(registry.execute(&mut doc, text).is_err(), "{text}");
        assert_eq!(doc.object(id), Some(&before));
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
    }
    registry
        .execute(&mut doc, "RemoveKnot Points 2,0,0 Undo 4,0,0")
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
    doc.undo().unwrap();
    assert_eq!(doc.object(id), Some(&before));
    doc.select_object(id, SelectionMode::Replace).unwrap();
    registry.execute(&mut doc, "RemoveKnot 2").unwrap();
    assert_eq!(
        doc.object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap()
            .knots(),
        &[0., 0., 4., 6., 6.]
    );
}

#[test]
fn coincident_markers_remove_the_selected_later_value_without_reselection() {
    let mut doc = viboceros_document::Document::default();
    let curve = viboceros_geometry::NurbsCurve::try_new(
        1,
        [(-1., 0.), (0., 0.), (1., 1.), (0., 0.), (1., 0.)]
            .into_iter()
            .map(|(x, y)| Point3::try_new(x, y, 0.).unwrap())
            .collect(),
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap();
    let id = doc
        .add_geometry(viboceros_document::Geometry::NurbsCurve(curve))
        .unwrap();
    let mut draft = Draft::new(&doc, id, None).unwrap();
    draft
        .point(Point3::try_new(0., 0., 0.).unwrap(), Options::default())
        .unwrap();
    assert_eq!(
        draft
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap()
            .knots(),
        &[0., 0., 1., 2., 4., 4.]
    );
    assert_eq!(draft.picks(), 1);
}

#[test]
fn followup_merged_polycurve_clocks_repeated_picks_and_deformed_periodic_tensors() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    audit_matrix(&root, "followup", 6);
}

#[test]
fn smooth_periodic_surface_deformation_repeated_picks_and_undo() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    audit_matrix(&root, "smooth_periodic", 2);
}

#[test]
fn screen_projection_uses_the_perspective_camera_for_the_crossing_case() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_knot_command.json")).unwrap(),
    )
    .unwrap();
    let v = &capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "periodic_surface_V_U")
        .unwrap()["value"];
    let (doc, _, _, views) = open_3dm_with_views(
        root.join("fixtures/remove_knot_command/periodic_surface_V_U.3dm")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let id = v["source_id"].as_str().unwrap().parse().unwrap();
    let mut found = false;
    for view in views {
        let projection = viboceros_command::picking::Projection::from_view(&view.camera).unwrap();
        let mut d = Draft::new(&doc, id, None).unwrap();
        d.point_in_view(point(&v["picks"][0]), Options::default(), Some(projection))
            .unwrap();
        let viboceros_document::Geometry::Brep(b) = d.geometry() else {
            panic!()
        };
        let gap = topology::surface(
            b.faces()[0].surface(),
            &v["after"][0]["geometry"]["surfaces"][0],
        );
        eprintln!(
            "VIEW_DIAGNOSTIC name={} active={} camera={:?} direction={:?} up={:?} frustum={:?} port={:?} gap={gap}",
            view.camera.name,
            view.active,
            view.camera.camera_location,
            view.camera.camera_direction,
            view.camera.camera_up,
            view.camera.frustum,
            view.camera.screen_port
        );
        if view.camera.projection == viboceros_io::ThreeDmProjection::Perspective {
            assert!(gap <= 1e-8, "{gap}");
            found = true;
        }
    }
    assert!(found);
}
