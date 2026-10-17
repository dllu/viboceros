use super::*;
use viboceros_geometry::PointMorph;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (
    VibocerosApp,
    ObjectId,
    ObjectId,
    ObjectId,
    serde_json::Value,
) {
    let mut app = test_app();
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/flow_command.json"
    ))
    .unwrap();
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/flow_command_rhino_20261016.json"
    ))
    .unwrap();
    let i = request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v["id"] == "cmd-arc-False-False-True")
        .unwrap();
    let f: viboceros_oracle::FlowFixture =
        serde_json::from_value(request["operations"][i].clone()).unwrap();
    let [ga, gb] = f.guides(app.document.tolerance()).unwrap();
    let a = app.document.add_geometry(ga).unwrap();
    let b = app.document.add_geometry(gb).unwrap();
    let source = app
        .document
        .add_geometry(Geometry::PointCloud(
            viboceros_geometry::PointCloud3::try_new(
                f.points
                    .iter()
                    .map(|p| Point3::try_from(*p).unwrap())
                    .collect(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, source, a, b, native["results"][i]["value"].clone())
}
#[test]
fn flow_picks_guides_previews_native_points_and_commits_one_undo() {
    let (mut app, source, a, b, expected) = setup();
    let original = app.document.object(source).unwrap().geometry().clone();
    let guides = [a, b].map(|id| app.document.object(id).unwrap().geometry().clone());
    let before = format!("{:?}", app.document);
    enter(&mut app, "Flow Copy=Yes Stretch=Yes");
    assert!(app.picking_flow_guide());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.pick_flow_guide(Some(a), None);
    app.pick_flow_guide(Some(b), None);
    assert!(!app.picking_flow_guide());
    let preview = app.flow_session.as_ref().unwrap().preview().unwrap();
    let Geometry::PointCloud(cloud) = &original else {
        panic!()
    };
    for (p, n) in cloud
        .points()
        .iter()
        .zip(expected["sdk_points"].as_array().unwrap())
    {
        let q: [f64; 3] = serde_json::from_value(n.clone()).unwrap();
        assert!(
            preview
                .morph
                .morph_point(*p)
                .unwrap()
                .distance_to(Point3::try_from(q).unwrap())
                .unwrap()
                < 1e-6
        );
    }
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Copy=No");
    enter(&mut app, "");
    assert!(app.flow_session.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 3);
    for (id, g) in [a, b].into_iter().zip(guides) {
        assert_eq!(app.document.object(id).unwrap().geometry(), &g);
    }
    assert_ne!(app.document.object(source).unwrap().geometry(), &original);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Flow"));
    assert_eq!(app.document.object(source).unwrap().geometry(), &original);
    assert!(app.document.undo().unwrap().is_none());
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Flow"));
}
#[test]
fn flow_command_first_options_cancel_and_stale_guides_leave_sources_intact() {
    let (mut app, source, a, b, _) = setup();
    app.document.clear_selection();
    enter(&mut app, "Flow Stretch=Yes");
    assert!(app.object_prompt.is_some());
    enter(&mut app, &format!("SelID {source}"));
    enter(&mut app, "");
    enter(&mut app, &format!("SelID {a}"));
    enter(&mut app, &format!("SelID {b}"));
    let before = format!("{:?}", app.document);
    enter(&mut app, "Rigid=Maybe");
    enter(&mut app, "Roadlike=Yes");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "Flow");
    app.pick_flow_guide(Some(a), None);
    app.pick_flow_guide(Some(b), None);
    app.document
        .replace_object_geometries([(b, Geometry::Point(point(0., 0., 0.)))])
        .unwrap();
    let edited = format!("{:?}", app.document);
    app.validate_flow_guides();
    assert!(app.picking_flow_guide());
    assert!(app.flow_session.as_ref().unwrap().preview().is_none());
    enter(&mut app, "");
    assert_eq!(format!("{:?}", app.document), edited);
    assert!(app.document.is_selected(source));
}
#[test]
fn flow_surface_edge_reference_does_not_select_or_deform_the_parent() {
    let (mut app, source, _, target, _) = setup();
    let parent = app
        .document
        .add_geometry(Geometry::Brep(
            viboceros_geometry::Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 10.], [0., 2.], [0., 1.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let original = app.document.object(parent).unwrap().geometry().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Flow Copy=No Stretch=Yes");
    enter(&mut app, &format!("Edge={parent},0"));
    enter(&mut app, &format!("SelID {target}"));
    assert!(
        app.flow_session.as_ref().unwrap().preview().is_some(),
        "{:?}",
        app.command_log
    );
    enter(&mut app, "");
    assert_eq!(app.document.object(parent).unwrap().geometry(), &original);
    assert!(!app.document.is_selected(parent));
    assert!(app.document.is_selected(source));
}

#[test]
fn flow_control_and_edit_grips_preview_copy_rigid_and_command_first_getters() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/flow_grips.json"
    ))
    .unwrap();
    for edit in [false, true] {
        for copy in [false, true] {
            for postselected in [false, true] {
                let name = format!(
                    "{}-{}copy{}-rigidTrue{}",
                    if edit { "edit" } else { "control" },
                    if postselected { "postselected-" } else { "" },
                    if copy { "True" } else { "False" },
                    if postselected { "" } else { "-groupsTrue" }
                );
                let op = request["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"] == name)
                    .unwrap();
                let f: viboceros_oracle::FlowGripsFixture =
                    serde_json::from_value(op.clone()).unwrap();
                let mut app = test_app();
                let geometry = f.source_geometry(app.document.tolerance()).unwrap();
                let [a, b] = f.guides(app.document.tolerance()).unwrap();
                let a = app.document.add_geometry(a).unwrap();
                let b = app.document.add_geometry(b).unwrap();
                let source = app.document.add_geometry(geometry.clone()).unwrap();
                if edit {
                    app.document.enable_edit_points([source]).unwrap();
                } else {
                    app.document.enable_control_points([source]).unwrap();
                }
                let picks = f
                    .selected
                    .iter()
                    .map(|&index| viboceros_document::ControlPointId {
                        object: source,
                        index,
                    })
                    .collect::<Vec<_>>();
                if !postselected {
                    app.document
                        .select_control_points(picks.iter().copied(), SelectionMode::Add)
                        .unwrap();
                }
                app.document.clear_history().unwrap();
                enter(
                    &mut app,
                    &format!("Flow {}", f.options.command().arguments()),
                );
                if postselected {
                    assert!(app.object_prompt.is_some());
                    app.handle_viewport_action(ViewportOutput {
                        control_point_selection: Some(crate::viewport::ControlPointSelection {
                            picks: picks.clone(),
                            mode: SelectionMode::Add,
                        }),
                        ..Default::default()
                    });
                    enter(&mut app, "");
                }
                assert!(app.picking_flow_guide(), "{name}: {:?}", app.command_log);
                enter(&mut app, &format!("SelID {a}"));
                enter(&mut app, &format!("SelID {b}"));
                let preview = app.flow_session.as_ref().unwrap().preview().unwrap();
                assert_eq!(preview.grips, picks);
                assert_eq!(app.document.object(source).unwrap().geometry(), &geometry);
                let expected = geometry
                    .with_morphed_grips_in_mode(
                        &picks.iter().map(|g| g.index).collect(),
                        &**preview.morph,
                        app.document.tolerance(),
                        f.mode(),
                    )
                    .unwrap();
                enter(&mut app, "");
                assert!(
                    app.active_command.is_none(),
                    "{name}: {:?}",
                    app.command_log
                );
                let result = if copy {
                    app.document
                        .objects()
                        .find(|o| ![a, b, source].contains(&o.id()))
                        .unwrap()
                        .id()
                } else {
                    source
                };
                assert_eq!(
                    app.document.object(result).unwrap().geometry(),
                    &expected,
                    "{name}"
                );
                if copy {
                    assert_eq!(app.document.object(source).unwrap().geometry(), &geometry);
                }
                app.document.undo().unwrap();
                assert_eq!(app.document.object(source).unwrap().geometry(), &geometry);
                assert!(app.document.undo().unwrap().is_none());
            }
        }
    }
}
