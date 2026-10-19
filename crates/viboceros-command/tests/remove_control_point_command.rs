use serde_json::Value;
use viboceros_command::{
    CommandRegistry, open_3dm_with_views,
    picking::Projection,
    remove_control_point::{Draft, Options},
};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[path = "support/remove_control_point.rs"]
mod qualification;
#[path = "support/remove_knot.rs"]
mod topology;

#[test]
fn actual_remove_control_point_geometry_metadata_selection_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_control_point_command.json"))
            .unwrap(),
    )
    .unwrap();
    let mut failures = Vec::new();
    let mut unexpected = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(&root, &row["value"])
        })) {
            Ok(_)
                if matches!(
                    row["value"]["recipe"]["family"].as_str(),
                    Some("polycurve" | "box")
                ) => {}
            Ok(Ok(gap)) => eprintln!(
                "REMOVE_CONTROL_POINT_AUDIT {}",
                serde_json::json!({"id":name,"maximum_error":gap})
            ),
            Ok(Err(())) => failures.push(name),
            Err(_) => unexpected.push(name),
        }
    }
    assert!(
        unexpected.is_empty(),
        "Unexpected workflow failures: {unexpected:?}"
    );
    assert_eq!(
        failures,
        qualification::KNOWN_GETTER_DIFFERENCES,
        "native getter differences changed"
    );
}
fn replay(root: &std::path::Path, v: &Value) -> Result<f64, ()> {
    let r = &v["recipe"];
    let name = r["id"].as_str().unwrap();
    let (mut doc, _, _, views) = open_3dm_with_views(
        root.join(format!("fixtures/remove_control_point_command/{name}.3dm"))
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let projection =
        Projection::from_view(&views.iter().find(|v| v.active).unwrap().camera).unwrap();
    let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
    doc.clear_selection();
    if v["before"][0]["selected"] == true {
        doc.select_object(id, SelectionMode::Replace).unwrap();
    }
    doc.clear_history().unwrap();
    let before = doc.object(id).unwrap().clone();
    let mut draft = match Draft::new(&doc, id) {
        Ok(d) => d,
        Err(_) => {
            assert!(
                matches!(r["family"].as_str(), Some("polycurve" | "box")),
                "{name}: unexpected unsupported source"
            );
            assert!(!doc.can_undo());
            assert_eq!(doc.object(id), Some(&before));
            eprintln!("REMOVE_CONTROL_POINT_UNSUPPORTED {name}");
            return Ok(0.);
        }
    };
    let mut options = Options::default();
    let mut picks = v["picks"].as_array().unwrap().iter();
    for token in v["tokens"].as_array().unwrap() {
        let token = token.as_str().unwrap();
        if token.starts_with('w') {
            let p = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(picks.next().unwrap().clone()).unwrap(),
            )
            .unwrap();
            draft.point_in_view(p, options, Some(projection)).unwrap();
        } else if !matches!(token, "_Enter" | "!") {
            assert!(
                viboceros_command::remove_control_point::accept_option(&mut options, token)
                    .unwrap()
            );
        }
        assert_eq!(doc.object(id), Some(&before), "{name}: immutable source");
        assert!(!doc.can_undo());
    }
    if r["cancel"] != true {
        let command = format!(
            "RemoveControlPoint Points {}",
            v["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|t| t.as_str())
                .filter(|t| *t != "_Enter")
                .collect::<Vec<_>>()
                .join(" ")
        );
        doc.select_object(id, SelectionMode::Replace).unwrap();
        CommandRegistry::with_builtins()
            .execute_with_pick_projection(
                &mut doc,
                &command,
                Default::default(),
                r["preselect"] == false,
                projection,
            )
            .unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            draft.geometry(),
            "{name}: batch/draft agree"
        );
    } else {
        doc.clear_selection();
    }
    let native = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        topology::check(&doc, &v["after"], id, name)
    }));
    let actual = doc.object(id).unwrap().clone();
    assert_eq!(actual.attributes(), before.attributes());
    assert_eq!(actual.group_ids(), before.group_ids());
    if doc.can_undo() {
        assert_eq!(doc.undo_label(), Some("RemoveControlPoint"));
        doc.undo().unwrap();
        assert_eq!(doc.object(id), Some(&before));
        if !v["undo"].is_null() {
            assert!(topology::check(&doc, &v["undo"], id, name) <= 1e-8);
        }
        doc.redo().unwrap();
        assert_eq!(doc.object(id), Some(&actual));
    } else {
        assert_eq!(actual, before);
        assert!(!doc.can_redo());
        assert!(v["undo"].is_null() || qualification::KNOWN_GETTER_DIFFERENCES.contains(&name));
    }
    match native {
        Ok(gap) if gap <= 1e-8 => Ok(gap),
        _ => Err(()),
    }
}

fn source() -> (viboceros_document::Document, ObjectId) {
    use viboceros_document::{Document, Geometry};
    use viboceros_geometry::NurbsCurve;
    let mut doc = Document::default();
    let curve = NurbsCurve::try_new(
        3,
        [[0., 0., 0.], [1., 3., 0.], [3., -1., 0.], [5., 0., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}

#[test]
fn malformed_or_repeated_input_is_atomic_and_preserves_redo() {
    let (mut doc, id) = source();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    doc.undo().unwrap();
    let before = doc.object(id).unwrap().clone();
    for command in [
        "RemoveControlPoint Points 1,3,0 NaN,0,0",
        "RemoveControlPoint Points 1,3,0 3,-1,0",
        "RemoveControlPoint Points Direction=Both 1,3,0",
        "RemoveControlPoint Points 1,3,0 Unknown=Yes",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(doc.object(id), Some(&before));
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
    }
    registry.execute(&mut doc, "RemoveControlPoint 1").unwrap();
    assert_ne!(doc.object(id).unwrap().geometry(), before.geometry());
    doc.undo().unwrap();
    assert_eq!(doc.object(id), Some(&before));
}

#[test]
fn staged_pick_is_immutable_and_rejects_stale_sources() {
    let (mut doc, id) = source();
    let before = doc.object(id).unwrap().clone();
    let mut draft = Draft::new(&doc, id).unwrap();
    let point = Point3::try_new(1., 3., 0.).unwrap();
    assert_eq!(
        draft
            .highlighted_points(point, Options::default(), None)
            .unwrap(),
        vec![point]
    );
    draft.point(point, Options::default()).unwrap();
    assert_eq!(doc.object(id), Some(&before));
    assert!(!doc.can_undo());
    assert!(draft.point(point, Options::default()).is_err());
    CommandRegistry::with_builtins()
        .execute(&mut doc, "RemoveControlPoint 2")
        .unwrap();
    let current = doc.object(id).unwrap().clone();
    assert!(!draft.current(&doc));
    assert!(draft.commit(&mut doc, false).is_err());
    assert_eq!(doc.object(id), Some(&current));
}

#[test]
fn standalone_surfaces_match_the_recorded_native_definitions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_control_point_command.json"))
            .unwrap(),
    )
    .unwrap();
    for name in [
        "surface_U",
        "surface_rational_V",
        "periodic_surface_U_U",
        "periodic_surface_V_U",
    ] {
        let v = &capture["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == name)
            .unwrap()["value"];
        let (mut doc, _, _, views) = open_3dm_with_views(
            root.join(format!("fixtures/remove_control_point_command/{name}.3dm"))
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let projection =
            Projection::from_view(&views.iter().find(|v| v.active).unwrap().camera).unwrap();
        let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
        let viboceros_document::Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let s = b.faces()[0].surface().clone();
        doc.replace_object_geometries([(id, viboceros_document::Geometry::NurbsSurface(s))])
            .unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        doc.clear_history().unwrap();
        let command = format!(
            "RemoveControlPoint Points {}",
            v["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(" ")
        );
        CommandRegistry::with_builtins()
            .execute_with_pick_projection(&mut doc, &command, Default::default(), false, projection)
            .unwrap();
        let viboceros_document::Geometry::NurbsSurface(s) = doc.object(id).unwrap().geometry()
        else {
            panic!()
        };
        let gap = topology::surface(s, &v["after"][0]["geometry"]["surfaces"][0]);
        assert!(gap <= 1e-8, "{name}: {gap}");
        eprintln!(
            "REMOVE_CONTROL_POINT_STANDALONE_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":gap})
        );
    }
}
