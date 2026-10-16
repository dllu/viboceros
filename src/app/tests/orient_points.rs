use super::*;
use viboceros_geometry::{LineSegment, PointCloud3};
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(0., 1., 0.),
                point(0., 0., 1.),
            ])
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, source)
}
#[test]
fn orient_points_repeated_copy_preview_and_history_use_the_same_originals() {
    for command in ["Orient", "Orient3Pt"] {
        let (mut app, source) = setup();
        enter(&mut app, &format!("{command} Copy=Yes"));
        enter(&mut app, "w0,0,0");
        enter(&mut app, "w1,0,0");
        if command == "Orient3Pt" {
            enter(&mut app, "w0,1,0");
        }
        assert!(app.affine_preview().is_some());
        for x in [3., 6.] {
            enter(&mut app, &format!("w{x},0,0"));
            enter(&mut app, &format!("w{},0,0", x + 1.));
            if command == "Orient3Pt" {
                enter(&mut app, &format!("w{x},1,0"));
            }
            assert!(app.active_command.is_some());
        }
        enter(&mut app, "Cancel");
        assert!(app.active_command.is_none());
        assert_eq!(app.document.objects().count(), 3);
        assert!(app.document.is_selected(source));
        assert_eq!(app.document.undo().unwrap().as_deref(), Some(command));
        assert_eq!(app.document.objects().count(), 1);
        app.document.redo().unwrap();
        assert_eq!(app.document.objects().count(), 3);
    }
}
#[test]
fn orient_points_rejects_degenerate_references_and_targets_without_edits() {
    let (mut app, _) = setup();
    enter(&mut app, "Orient3Pt");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    enter(&mut app, "w2,0,0");
    enter(&mut app, "w0,1,0");
    enter(&mut app, "w4,0,0");
    enter(&mut app, "w4,0,0");
    enter(&mut app, "w5,0,0");
    enter(&mut app, "w6,0,0");
    assert!(app.active_command.is_some());
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "w4,1,0");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("Orient3Pt"));
}
#[test]
fn orient_points_command_first_grips_and_scale_memory_follow_their_own_options() {
    let mut app = test_app();
    let curve = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0., 0., 0.),
                point(1., 0., 0.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([curve]).unwrap();
    enter(&mut app, "Orient Copy=Yes");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    enter(&mut app, "Scale=1D");
    enter(&mut app, "Cancel");
    assert_eq!(
        app.commands
            .orient_point_scale(viboceros_command::orient_points::Kind::Two),
        viboceros_command::orient_points::Scale::OneDimensional
    );
    assert_eq!(
        app.commands
            .orient_point_scale(viboceros_command::orient_points::Kind::Three),
        viboceros_command::orient_points::Scale::No
    );
    enter(&mut app, "Orient Copy=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w2,0,0");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(
        app.document.object(curve).unwrap().geometry(),
        &Geometry::Line(
            LineSegment::try_new(
                point(0., 0., 0.),
                point(1., 0., 0.),
                app.document.tolerance()
            )
            .unwrap()
        )
    );
    let copy = app.document.objects().find(|o| o.id() != curve).unwrap();
    assert_eq!(
        app.document.control_point_locations(copy.id()),
        Some([point(0., 0., 0.), point(2., 0., 0.)].as_slice())
    );
    let vertices = copy.geometry().bounds();
    assert_eq!(vertices.max(), point(2., 0., 0.));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Orient"));
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn orient_points_native_workflows_match_all_witnesses_selection_groups_and_preview() {
    use serde_json::Value;
    for (fixture, native) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/orient_points.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/orient_points_rhino_20261016.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/orient_points_rotation.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/orient_points_rotation_rhino_20261016.json"
            ),
        ),
    ] {
        let request: Value = serde_json::from_str(fixture).unwrap();
        let native: Value = serde_json::from_str(native).unwrap();
        let p = |v: &Value| {
            Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
        };
        let token = |v: &Value| format!("w{}", p(v).to_array().map(|x| x.to_string()).join(","));
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut app = test_app();
            let base = p(&op["references"][0]).to_array();
            let witnesses = (0..4)
                .map(|i| {
                    Point3::try_from(std::array::from_fn(|j| {
                        base[j] + if i == j + 1 { 1. } else { 0. }
                    }))
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let original = Geometry::PointCloud(PointCloud3::try_new(witnesses.clone()).unwrap());
            let source = app.document.add_geometry(original.clone()).unwrap();
            if op["group"] == true {
                app.document
                    .add_group(Some("Owned orient source".into()), [source])
                    .unwrap();
            }
            if op["postselected"] != true {
                app.document
                    .select_objects_direct([source], SelectionMode::Replace)
                    .unwrap();
            }
            app.document.clear_history().unwrap();
            let command = op["command"].as_str().unwrap();
            if let Some(scale) = op["setup_scale"].as_str() {
                enter(&mut app, &format!("{command} Scale={scale}"));
                enter(&mut app, "Cancel");
            }
            let scale = if op["omit_scale"] == true {
                String::new()
            } else {
                format!("Scale={}", op["scale"].as_str().unwrap())
            };
            enter(
                &mut app,
                &format!(
                    "{command} {scale} Copy={}",
                    if op["copy"] == true { "Yes" } else { "No" }
                ),
            );
            if op["postselected"] == true {
                enter(&mut app, &format!("SelID {source}"));
                enter(&mut app, "");
            }
            let references = op["references"].as_array().unwrap();
            let targets = op["targets"].as_array().unwrap();
            let stop = op["cancel_after"].as_u64().map(|v| v as usize);
            for (i, reference) in references.iter().enumerate() {
                if stop == Some(i) {
                    break;
                }
                enter(&mut app, &token(reference));
            }
            if stop.is_none_or(|n| n >= references.len()) {
                if let Some(scale) = op["edit_scale"].as_str() {
                    enter(&mut app, &format!("Scale={scale}"));
                }
                let batches = std::iter::once(targets.clone()).chain(
                    op["batch_targets"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|v| v.as_array().unwrap().clone()),
                );
                for (batch, targets) in batches.enumerate() {
                    for (i, target) in targets.iter().enumerate() {
                        if stop == Some(references.len() + i) {
                            break;
                        }
                        if i == targets.len() - 1 {
                            let preview = app.affine_preview().unwrap();
                            let map = preview
                                .definition
                                .transform_at(
                                    preview.frame.unwrap_or_else(|| {
                                        viboceros_command::CommandContext::default()
                                            .construction_plane
                                    }),
                                    p(target),
                                    app.document.tolerance(),
                                )
                                .unwrap();
                            // Match by the affine witness itself rather than Rhino's object table order.
                            assert!(
                                row["value"]["outputs"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .filter(|v| op["copy"] != true || v["original"] == false)
                                    .any(|cloud| {
                                        witnesses
                                            .iter()
                                            .zip(cloud["points"].as_array().unwrap())
                                            .all(|(a, b)| {
                                                map.transform_point(*a)
                                                    .unwrap()
                                                    .distance_to(p(b))
                                                    .unwrap()
                                                    < 1.001e-6
                                            })
                                    }),
                                "{} preview batch {batch}",
                                op["id"]
                            );
                        }
                        if batch == 0 && i == 0 && op["default_target"] == true {
                            enter(&mut app, "");
                        } else {
                            enter(&mut app, &token(target));
                        }
                    }
                }
            }
            if app.active_command.is_some() {
                enter(&mut app, "Cancel");
            }
            assert!(app.active_command.is_none());
            let expected = row["value"]["outputs"].as_array().unwrap();
            let mut used = std::collections::BTreeSet::new();
            let actual = app.document.objects().collect::<Vec<_>>();
            assert_eq!(actual.len(), expected.len(), "{}", op["id"]);
            for object in actual {
                let Geometry::PointCloud(cloud) = object.geometry() else {
                    panic!()
                };
                let (index, value) = expected
                    .iter()
                    .enumerate()
                    .find(|(i, v)| {
                        !used.contains(i)
                            && v["original"].as_bool() == Some(object.id() == source)
                            && cloud
                                .points()
                                .iter()
                                .zip(v["points"].as_array().unwrap())
                                .all(|(a, b)| a.distance_to(p(b)).unwrap() < 1.001e-6)
                    })
                    .unwrap_or_else(|| panic!("{} placement", op["id"]));
                used.insert(index);
                assert_eq!(
                    app.document.is_selected(object.id()),
                    value["selected"].as_bool().unwrap(),
                    "{}",
                    op["id"]
                );
                assert_eq!(
                    object.group_ids().len(),
                    value["group_count"].as_u64().unwrap() as usize
                );
            }
            assert_eq!(
                app.document.groups().len(),
                row["value"]["group_count"].as_u64().unwrap() as usize
            );
            if stop.is_some() {
                assert!(app.document.undo_label().is_none());
                assert_eq!(app.document.object(source).unwrap().geometry(), &original);
            } else {
                assert_eq!(app.document.undo().unwrap().as_deref(), Some(command));
                assert_eq!(app.document.objects().count(), 1);
                assert_eq!(app.document.object(source).unwrap().geometry(), &original);
                app.document.redo().unwrap();
                assert_eq!(app.document.objects().count(), expected.len());
            }
        }
    }
}
