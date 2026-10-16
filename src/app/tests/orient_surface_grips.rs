use super::*;
use viboceros_document::{ControlPointId, GripMode};
use viboceros_oracle::OrientSurfaceGripsFixture;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn setup(f: &OrientSurfaceGripsFixture) -> (VibocerosApp, ObjectId, Option<ObjectId>, ObjectId) {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let guide = app
        .document
        .add_geometry(f.placement.guide(tolerance).unwrap())
        .unwrap();
    let source = app
        .document
        .add_geometry(f.source_geometry(tolerance).unwrap())
        .unwrap();
    let peer = f.peer.then(|| {
        app.document
            .add_geometry(Geometry::Point(point(1., 1., 1.)))
            .unwrap()
    });
    if f.placement.group {
        app.document
            .add_group(None, std::iter::once(source).chain(peer))
            .unwrap();
    }
    match f.mode() {
        GripMode::Control => app.document.enable_control_points([source]).unwrap(),
        GripMode::Edit => app.document.enable_edit_points([source]).unwrap(),
    };
    if !f.placement.postselected {
        app.document
            .select_control_points(
                f.selected.iter().map(|&index| ControlPointId {
                    object: source,
                    index,
                }),
                SelectionMode::Add,
            )
            .unwrap();
        app.document
            .select_objects_direct(
                peer.into_iter().chain(f.parent.then_some(source)),
                SelectionMode::Replace,
            )
            .unwrap();
    }
    app.document.clear_history().unwrap();
    (app, source, peer, guide)
}
fn recipes() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/orient_surface_grips.json"
    ))
    .unwrap()
}
fn close(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value as V;
    match (a, b) {
        (V::Number(a), V::Number(b)) => (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6,
        (V::Array(a), V::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
        }
        (V::Object(a), V::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|b| close(v, b)))
        }
        _ => a == b,
    }
}
#[test]
fn orient_surface_grips_interactive_replays_native_owner_geometry_groups_and_selections() {
    let recipes = recipes();
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/orient_surface_grips_rhino_20261016.json"
    ))
    .unwrap();
    assert_eq!(
        recipes["operations"].as_array().unwrap().len(),
        native["results"].as_array().unwrap().len()
    );
    for (r, n) in recipes["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(r["id"], n["id"]);
        let f: OrientSurfaceGripsFixture = serde_json::from_value(r.clone()).unwrap();
        let (mut app, source, peer, guide) = setup(&f);
        enter(
            &mut app,
            &format!("OrientOnSrf {}", f.placement.options.options().arguments()),
        );
        if f.placement.postselected {
            assert!(app.object_prompt.is_some());
            enter(&mut app, "SelPt");
            enter(&mut app, "");
        }
        for p in [f.placement.base, f.placement.reference] {
            enter(
                &mut app,
                &format!("w{}", p.map(|x| x.to_string()).join(",")),
            );
        }
        assert!(
            app.picking_orient_surface(),
            "{} {:?}",
            r["id"],
            app.command_log
        );
        app.accept_component_face_hit(guide, 0, None);
        for (uv, scale, angle) in std::iter::once((
            f.placement.uv,
            f.placement.options.scale,
            f.placement.options.rotation,
        ))
        .chain(f.placement.batch.iter().map(|b| {
            (
                b.uv,
                b.scale.unwrap_or(f.placement.options.scale),
                b.rotation.unwrap_or(f.placement.options.rotation),
            )
        })) {
            let p = f.placement.target(uv, app.document.tolerance()).unwrap();
            assert!(app.accept_drafting_point(p));
            enter(&mut app, &scale.to_string());
            enter(&mut app, &angle.to_string());
        }
        if app.active_command.is_some() {
            enter(&mut app, "");
        }
        let actual = f.observe(&app.document, source, peer, guide).unwrap();
        let expected = &n["value"]["after"];
        for key in ["group_count", "guide_unchanged", "guide_selected"] {
            assert_eq!(actual[key], expected[key], "{} {key}", r["id"]);
        }
        let a = actual["outputs"].as_array().unwrap();
        let b = expected["outputs"].as_array().unwrap();
        assert_eq!(a.len(), b.len(), "{} {:?}", r["id"], app.command_log);
        let mut used = std::collections::BTreeSet::new();
        for output in a {
            let (i, _) = b
                .iter()
                .enumerate()
                .find(|(i, e)| !used.contains(i) && close(output, e))
                .unwrap_or_else(|| panic!("{} actual {} expected {}", r["id"], output, expected));
            used.insert(i);
        }
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnSrf"));
        assert_eq!(
            app.document.object(source).unwrap().geometry(),
            &f.source_geometry(app.document.tolerance()).unwrap()
        );
        app.document.redo().unwrap();
    }
}
#[test]
fn orient_surface_grips_freezes_empty_object_sources_and_rejects_guide_owner_grips() {
    let f: OrientSurfaceGripsFixture =
        serde_json::from_value(recipes()["operations"][0].clone()).unwrap();
    let (mut app, source, _, guide) = setup(&f);
    let unrelated = app
        .document
        .add_geometry(Geometry::Point(point(99., 0., 0.)))
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "OrientOnSrf ScalePrompt=Yes RotationPrompt=Yes Copy=No",
    );
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    app.accept_orient_surface(guide, None);
    app.document
        .select_objects_direct([unrelated], SelectionMode::Replace)
        .unwrap();
    // A selection change alone must not become another captured object source.
    let q = f
        .placement
        .target(f.placement.uv, app.document.tolerance())
        .unwrap();
    assert!(app.accept_drafting_point(q));
    enter(&mut app, "1");
    enter(&mut app, "0");
    assert_eq!(
        app.document.object(unrelated).unwrap().geometry(),
        &Geometry::Point(point(99., 0., 0.))
    );
    assert!(app.active_command.is_none());
    assert_ne!(
        app.document.object(source).unwrap().geometry(),
        &f.source_geometry(app.document.tolerance()).unwrap()
    );
    app.document.enable_control_points([guide]).unwrap();
    app.document.clear_selection();
    app.document
        .select_control_points(
            [ControlPointId {
                object: guide,
                index: 0,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    let line =
        format!("OrientOnSrf 0,0,0 1,0,0 1,1,1 Surface={guide} Sources= GripSources={guide}:0");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(app.commands.execute(&mut app.document, &line).is_err());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.object(source).is_some());
}
