#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::CommandRegistry;
use viboceros_document::Document;
fn included(id: &str) -> bool {
    [
        "arc_default",
        "arc_s_shape",
        "arc_spatial",
        "arc_alternate",
        "arc_trim",
        "arc_join",
    ]
    .contains(&id)
}
fn command(doc: &Document, v: &serde_json::Value) -> String {
    let source = &v["prescribed_case"];
    let curves = &source["curves"];
    let a = reference_inputs::guide(doc, 0);
    let b = reference_inputs::guide(doc, 1);
    let mut words = vec![format!(
        "ArcBlend Curve={a} {} Curve={b} {}",
        reference_inputs::token(&curves[0]["end"]),
        reference_inputs::token(&curves[1]["start"])
    )];
    for a in &source["actions"].as_array().unwrap()[2..] {
        if a == "Enter" {
            continue;
        }
        if a == "AlternateSolution" {
            words.push("AlternateSolution=Yes".into());
        } else {
            words.push(reference_inputs::token(a));
        }
    }
    words.join(" ")
}
#[test]
fn arc_blend_commands_match_native_geometry_attributes_selection_history_and_archives() {
    let registry = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in ["arc_blend_command", "arc_blend_options"] {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            if !included(id) {
                continue;
            }
            let v = &row["value"];
            let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
            let parents = doc.objects().cloned().collect::<Vec<_>>();
            let command = command(&doc, v);
            registry
                .execute(&mut doc, &command)
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            let mut error = replay::check(&doc, &v["after"], false);
            if id == "arc_trim" {
                for parent in &parents {
                    let now = doc.object(parent.id()).unwrap();
                    assert_eq!(now, parent);
                    assert!(
                        parent
                            .geometry_snapshot()
                            .shares_storage_with(now.geometry_snapshot())
                    );
                }
            }
            assert_eq!(doc.undo_label(), Some("ArcBlend"));
            doc.undo().unwrap();
            error = error.max(replay::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(replay::check(&doc, &v["redo"], false));
            let path = directory.path().join("blend.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(replay::check(&restored, &v["after"], true));
            println!(
                "ARC_BLEND_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 6);
}
#[test]
fn invalid_arc_blend_options_and_stale_sources_are_atomic() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    r.execute(&mut doc, "Line -5,0,0 0,0,0").unwrap();
    r.execute(&mut doc, "Line 5,4,0 5,9,0").unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    r.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for options in [
        "Ratio=0",
        "Ratio=NaN",
        "Join=Yes",
        "Parameter1=20",
        "Ratio=1 Garbage=No",
    ] {
        assert!(
            r.execute(
                &mut doc,
                &format!(
                    "ArcBlend Curve={} 0,0,0 Curve={} 5,4,0 {options}",
                    ids[0], ids[1]
                )
            )
            .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    let mut input = viboceros_command::arc_blend::Input::new(doc.tolerance());
    input
        .pick_curve(
            &doc,
            ids[0],
            None,
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
        )
        .unwrap();
    input
        .pick_curve(
            &doc,
            ids[1],
            None,
            viboceros_geometry::Point3::try_new(5., 4., 0.).unwrap(),
        )
        .unwrap();
    doc.delete_objects([ids[0]]).unwrap();
    let before = format!("{doc:?}");
    assert!(input.commit(&mut doc).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn interior_parameter_trimming_and_joining_retain_the_outer_source_portions() {
    for join in [false, true] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Line -5,0,0 0,0,0").unwrap();
        registry.execute(&mut doc, "Line 5,4,0 5,9,0").unwrap();
        let parents = doc.objects().cloned().collect::<Vec<_>>();
        doc.clear_history().unwrap();
        registry.execute(&mut doc, &format!("ArcBlend Curve={} 0,0,0 Curve={} 5,4,0 Trim=Yes Join={} Parameter1=4 Parameter2=1", parents[0].id(), parents[1].id(), if join { "Yes" } else { "No" })).unwrap();
        let point = |x, y| viboceros_geometry::Point3::try_new(x, y, 0.).unwrap();
        let curve = doc
            .objects()
            .last()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        let start = if join { point(-5., 0.) } else { point(-1., 0.) };
        let end = if join { point(5., 9.) } else { point(5., 5.) };
        assert!(
            curve
                .evaluate(*curve.domain().start())
                .unwrap()
                .distance_to(start)
                .unwrap()
                < 1e-10
        );
        assert!(
            curve
                .evaluate(*curve.domain().end())
                .unwrap()
                .distance_to(end)
                .unwrap()
                < 1e-10
        );
        assert_eq!(doc.objects().len(), if join { 1 } else { 3 });
        if !join {
            for (i, end) in [point(-1., 0.), point(5., 9.)].into_iter().enumerate() {
                let curve = doc
                    .object(parents[i].id())
                    .unwrap()
                    .geometry()
                    .curve_ref()
                    .unwrap();
                assert!(
                    curve
                        .evaluate(*curve.domain().end())
                        .unwrap()
                        .distance_to(end)
                        .unwrap()
                        < 1e-10
                );
            }
        }
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), parents);
        assert!(!doc.can_undo());
    }
}

#[test]
fn edge_blends_preserve_surface_storage_and_reject_surface_trimming_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Plane 0,0,0 4,4,0").unwrap();
    registry.execute(&mut doc, "Line 8,3,0 8,8,0").unwrap();
    let parents = doc.objects().cloned().collect::<Vec<_>>();
    doc.clear_history().unwrap();
    let command = format!(
        "ArcBlend Edge={},0 4,0,0 Curve={} 8,3,0",
        parents[0].id(),
        parents[1].id()
    );
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(&mut doc, &format!("{command} Trim=Yes"))
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    registry.execute(&mut doc, &command).unwrap();
    assert_eq!(doc.objects().len(), 3);
    for parent in &parents {
        let now = doc.object(parent.id()).unwrap();
        assert_eq!(now, parent);
        assert!(
            parent
                .geometry_snapshot()
                .shares_storage_with(now.geometry_snapshot())
        );
    }
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), parents);
    assert!(!doc.can_undo());
}

#[test]
fn scripts_apply_contact_parameters_before_constructing_the_two_arcs() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line -5,0,0 0,0,0").unwrap();
    registry.execute(&mut doc, "Line 0,0,0 0,5,0").unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.clear_history().unwrap();
    registry
        .execute(
            &mut doc,
            &format!(
                "ArcBlend Curve={} 0,0,0 Curve={} 0,0,0 Ratio=1 Trim=Yes Parameter1=4 Parameter2=1",
                ids[0], ids[1]
            ),
        )
        .unwrap();
    let curve = doc
        .objects()
        .last()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    let a = curve.evaluate(*curve.domain().start()).unwrap();
    let b = curve.evaluate(*curve.domain().end()).unwrap();
    assert!(
        a.distance_to(viboceros_geometry::Point3::try_new(-1., 0., 0.).unwrap())
            .unwrap()
            < 1e-10
    );
    assert!(
        b.distance_to(viboceros_geometry::Point3::try_new(0., 1., 0.).unwrap())
            .unwrap()
            < 1e-10
    );
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert!(!doc.can_undo());
}
