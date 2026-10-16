use super::*;
use crate::viewport::{ComponentClick, ComponentPick, EdgePick};
use viboceros_command::ComponentSelectionKind;
use viboceros_geometry::{Brep, NurbsSurface, PointCloud3, Vector3};

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn setup(surface: bool) -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    let geometry = if surface {
        Geometry::NurbsSurface(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    point(0., 0., 0.),
                    point(4., 0., 0.),
                    point(0., 6., 0.),
                    point(4., 6., 2.),
                ],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        )
    } else {
        Geometry::Brep(
            Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 4.], [0., 6.], [0., 2.]],
                app.document.tolerance(),
            )
            .unwrap(),
        )
    };
    let parent = app.document.add_geometry(geometry).unwrap();
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
    (app, parent, source)
}

#[test]
fn orient_curve_edge_full_and_partial_mouse_routing_preserves_parent_and_batch_history() {
    for surface in [false, true] {
        for partial in [false, true] {
            let (mut app, parent, source) = setup(surface);
            let before = app
                .document
                .object(parent)
                .unwrap()
                .geometry_snapshot()
                .clone();
            let original = app.document.object(source).unwrap().geometry().clone();
            enter(&mut app, "OrientOnCrv Copy=Yes");
            enter(&mut app, "w0,0,0");
            if partial {
                enter(&mut app, "SubCrv");
            }
            app.handle_viewport_action(ViewportOutput {
                edge_click: Some(vec![EdgePick {
                    object: parent,
                    edge: 0,
                }]),
                ..Default::default()
            });
            if partial {
                enter(&mut app, "w1,0,0");
                app.update_subcurve_hover(point(3., 0., 0.));
                let p = app.subcurve_draft_preview().unwrap();
                assert!(
                    p.geometry.endpoints[0]
                        .distance_to(point(1., 0., 0.))
                        .unwrap()
                        < 1e-12
                );
                assert!(
                    p.geometry.endpoints[1]
                        .distance_to(point(3., 0., 0.))
                        .unwrap()
                        < 1e-12
                );
                enter(&mut app, "w3,0,0");
            }
            assert_eq!(app.document.objects().count(), 2);
            assert!(app.document.undo_label().is_none());
            let preview = app.affine_preview().unwrap();
            let map = preview
                .definition
                .transform_at(
                    preview.frame.unwrap(),
                    point(2., 0., 0.),
                    app.document.tolerance(),
                )
                .unwrap();
            for (a, b) in [
                (point(0., 0., 0.), point(2., 0., 0.)),
                (point(1., 0., 0.), point(3., 0., 0.)),
                (point(0., 1., 0.), point(2., 1., 0.)),
                (point(0., 0., 1.), point(2., 0., 1.)),
            ] {
                assert!(map.transform_point(a).unwrap().distance_to(b).unwrap() < 1e-12);
            }
            enter(&mut app, "w2,0,0");
            enter(&mut app, "w2.5,0,0");
            enter(&mut app, "");
            assert_eq!(app.document.objects().count(), 4);
            assert_eq!(app.document.is_selected(source), !partial);
            assert!(
                before
                    .shares_storage_with(app.document.object(parent).unwrap().geometry_snapshot())
            );
            assert_eq!(app.document.object(source).unwrap().geometry(), &original);
            assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnCrv"));
            assert_eq!(app.document.objects().count(), 2);
            app.document.redo().unwrap();
            assert_eq!(app.document.objects().count(), 4);
            assert!(
                before
                    .shares_storage_with(app.document.object(parent).unwrap().geometry_snapshot())
            );
        }
    }
}

#[test]
fn orient_curve_edge_chooser_and_changed_parent_return_to_guide_selection() {
    let (mut app, parent, _) = setup(true);
    let other = app
        .document
        .add_geometry(app.document.object(parent).unwrap().geometry().clone())
        .unwrap();
    enter(&mut app, "OrientOnCrv");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "SubCrv");
    let picks = vec![parent, other]
        .into_iter()
        .map(|object| ComponentPick {
            object,
            index: 0,
            kind: ComponentSelectionKind::BrepEdge,
        })
        .collect();
    app.accept_component_click(ComponentClick {
        picks,
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    assert!(app.component_selection.has_choices());
    enter(&mut app, "2");
    assert!(!app.component_selection.has_choices());
    enter(&mut app, "w1,0,0");
    let changed = Geometry::NurbsSurface(
        NurbsSurface::try_bilinear([
            point(0., 0., 1.),
            point(4., 0., 1.),
            point(0., 6., 1.),
            point(4., 6., 1.),
        ])
        .unwrap(),
    );
    app.document
        .replace_object_geometries([(other, changed)])
        .unwrap();
    let external = app
        .document
        .object(other)
        .unwrap()
        .geometry_snapshot()
        .clone();
    assert!(!app.validate_orient_guide());
    assert!(app.picking_orient_curve());
    assert!(app.affine_preview().is_none());
    assert!(app.subcurve_draft_preview().is_none());
    app.pick_orient_edge(EdgePick {
        object: parent,
        edge: 0,
    });
    enter(&mut app, "w1,0,0");
    enter(&mut app, "w3,0,0");
    enter(&mut app, "w2,0,0");
    assert!(app.active_command.is_none());
    app.document.undo().unwrap();
    assert!(external.shares_storage_with(app.document.object(other).unwrap().geometry_snapshot()));
}

#[test]
fn orient_curve_edge_typed_input_rejects_invalid_indices_without_model_edits() {
    let (mut app, parent, _) = setup(false);
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "OrientOnCrv");
    enter(&mut app, "w0,0,0");
    enter(&mut app, &format!("Edge={parent},999"));
    assert!(app.picking_orient_curve());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, &format!("Edge={parent},0"));
    enter(&mut app, "w2,0,0");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("OrientOnCrv"));
}

#[test]
fn orient_curve_edge_invalidates_when_parent_is_unavailable_or_tolerance_changes() {
    for change in 0..4 {
        let (mut app, parent, source) = setup(false);
        enter(&mut app, "OrientOnCrv");
        enter(&mut app, "w0,0,0");
        app.pick_orient_edge(EdgePick {
            object: parent,
            edge: 0,
        });
        let original = app.document.object(source).unwrap().geometry().clone();
        assert!(app.affine_preview().is_some());
        match change {
            0 => {
                app.document
                    .set_objects_visibility([parent], false)
                    .unwrap();
            }
            1 => {
                app.document.set_objects_locked([parent], true).unwrap();
            }
            2 => {
                app.document
                    .set_tolerance(Tolerance::try_new(1e-4, 1e-9, 1e-6).unwrap());
            }
            _ => {
                app.document.delete_objects([parent]).unwrap();
            }
        }
        enter(&mut app, "w2,0,0");
        assert!(app.picking_orient_curve());
        assert!(app.affine_preview().is_none());
        assert_eq!(app.document.object(source).unwrap().geometry(), &original);
        assert_ne!(app.document.undo_label(), Some("OrientOnCrv"));
    }
}

#[test]
fn orient_curve_edge_native_witnesses_match_preview_placement_selection_and_history() {
    use serde_json::Value;
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/orient_on_curve_edge.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/orient_on_curve_edge_rhino_20261016.json"
    ))
    .unwrap();
    let p = |v: &Value| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
    };
    let token = |v: &Value| format!("w{}", p(v).to_array().map(|x| x.to_string()).join(","));
    let ops = request["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), rows.len());
    for (op, row) in ops.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        assert_eq!(row["value"]["command_returned"], true);
        let mut app = test_app();
        let s = &op["source"];
        let geometry = match s["type"].as_str().unwrap() {
            "box_brep" => {
                let a = p(&s["min"]).to_array();
                let b = p(&s["max"]).to_array();
                Geometry::Brep(
                    Brep::try_box(
                        viboceros_command::CommandContext::default().construction_plane,
                        std::array::from_fn(|i| [a[i], b[i]]),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                )
            }
            "surface" => Geometry::NurbsSurface(
                NurbsSurface::try_new_rational(
                    s["degree_u"].as_u64().unwrap() as usize,
                    s["degree_v"].as_u64().unwrap() as usize,
                    s["control_point_count_u"].as_u64().unwrap() as usize,
                    s["control_point_count_v"].as_u64().unwrap() as usize,
                    s["control_points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| {
                            viboceros_geometry::WeightedPoint3::try_new(
                                p(&v["point"]),
                                v["weight"].as_f64().unwrap(),
                            )
                            .unwrap()
                        })
                        .collect(),
                    serde_json::from_value(s["knots_u"].clone()).unwrap(),
                    serde_json::from_value(s["knots_v"].clone()).unwrap(),
                )
                .unwrap(),
            ),
            "cylinder_brep" => Geometry::Brep(
                Brep::try_cylinder(
                    Frame3::try_from_normal(
                        p(&s["origin"]),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                    s["radius"].as_f64().unwrap(),
                    0.,
                    s["height"].as_f64().unwrap(),
                    app.document.tolerance(),
                )
                .unwrap(),
            ),
            _ => panic!("edge fixture parent"),
        };
        let edge = if !op["edge_anchor"].is_null() {
            let Geometry::Brep(brep) = &geometry else {
                panic!("anchor parent")
            };
            let anchor = p(&op["edge_anchor"]);
            brep.edges()
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let t = e
                        .curve()
                        .closest_parameter(anchor, app.document.tolerance())
                        .unwrap();
                    (
                        i,
                        e.curve().evaluate(t).unwrap().distance_to(anchor).unwrap(),
                    )
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0
        } else {
            op["edge"].as_u64().unwrap() as usize
        };
        let parent = app.document.add_geometry(geometry).unwrap();
        let parent_before = app
            .document
            .object(parent)
            .unwrap()
            .geometry_snapshot()
            .clone();
        let base = p(&op["base"]).to_array();
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
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let o = &op["options"];
        let options = viboceros_command::orient_curve::Options {
            copy: o["copy"].as_bool().unwrap_or(false),
            on_curve: o["on_curve"].as_bool().unwrap_or(false),
            perpendicular: o["perpendicular"].as_bool().unwrap_or(false),
            flip: o["flip"].as_bool().unwrap_or(false),
            x_flip: o["xflip"].as_bool().unwrap_or(false),
            y_flip: o["yflip"].as_bool().unwrap_or(false),
            ..Default::default()
        };
        enter(&mut app, &format!("OrientOnCrv {}", options.arguments()));
        if !options.on_curve {
            enter(&mut app, &token(&op["base"]));
        }
        if !op["subcurve"].is_null() {
            enter(&mut app, "SubCrv");
        }
        app.handle_viewport_action(ViewportOutput {
            edge_click: Some(vec![EdgePick {
                object: parent,
                edge,
            }]),
            ..Default::default()
        });
        if !op["subcurve"].is_null() {
            enter(&mut app, &token(&op["subcurve"]["start"]));
            enter(&mut app, &token(&op["subcurve"]["end"]));
        }
        if options.on_curve {
            enter(&mut app, &token(&op["base"]));
        }
        let expected = row["value"]["outputs"].as_array().unwrap();
        if o["rotate"].is_null() {
            let preview = app.affine_preview().unwrap();
            let map = preview
                .definition
                .transform_at(
                    preview.frame.unwrap(),
                    p(&op["target"]),
                    app.document.tolerance(),
                )
                .unwrap();
            let cloud = expected
                .iter()
                .find(|v| !options.copy || v["original"] == false)
                .unwrap();
            for (a, b) in witnesses.iter().zip(cloud["points"].as_array().unwrap()) {
                assert!(
                    map.transform_point(*a).unwrap().distance_to(p(b)).unwrap() < 1.001e-6,
                    "{} preview",
                    op["id"]
                );
            }
        } else {
            enter(&mut app, "Rotate=Yes");
        }
        assert!(app.document.undo_label().is_none());
        assert_eq!(app.document.object(source).unwrap().geometry(), &original);
        enter(&mut app, &token(&op["target"]));
        if let Some(angle) = o["rotate"].as_f64() {
            enter(&mut app, &angle.to_string());
        }
        if options.copy {
            enter(&mut app, "");
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let actual = app
            .document
            .objects()
            .filter(|o| matches!(o.geometry(), Geometry::PointCloud(_)))
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len());
        for a in &actual {
            let b = expected
                .iter()
                .find(|v| v["original"].as_bool() == Some(a.id() == source))
                .unwrap();
            assert_eq!(
                a.id() == source,
                b["original"].as_bool().unwrap(),
                "{}",
                op["id"]
            );
            assert_eq!(
                app.document.is_selected(a.id()),
                b["selected"].as_bool().unwrap(),
                "{}",
                op["id"]
            );
            let Geometry::PointCloud(cloud) = a.geometry() else {
                unreachable!()
            };
            for (a, b) in cloud.points().iter().zip(b["points"].as_array().unwrap()) {
                assert!(
                    a.distance_to(p(b)).unwrap() < 1.001e-6,
                    "{} placement",
                    op["id"]
                );
            }
        }
        let actual_count = actual.len();
        drop(actual);
        assert!(
            parent_before
                .shares_storage_with(app.document.object(parent).unwrap().geometry_snapshot())
        );
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnCrv"));
        assert_eq!(app.document.object(source).unwrap().geometry(), &original);
        assert_eq!(app.document.objects().count(), 2);
        app.document.redo().unwrap();
        assert_eq!(app.document.objects().count(), actual_count + 1);
    }
}

#[test]
fn orient_curve_edge_numeric_length_and_relative_copies_use_the_constrained_anchor() {
    let (mut app, parent, source) = setup(false);
    let snapshot = app
        .document
        .object(parent)
        .unwrap()
        .geometry_snapshot()
        .clone();
    enter(&mut app, "OrientOnCrv Copy=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "SubCrv");
    app.pick_orient_edge(EdgePick {
        object: parent,
        edge: 0,
    });
    enter(&mut app, "w1,0,0");
    enter(&mut app, "2");
    app.update_subcurve_hover(point(3., 0., 0.));
    enter(&mut app, "Direction=Locked");
    enter(&mut app, "w3,0,0");
    enter(&mut app, "w9,0,0");
    assert_eq!(app.last_point, Some(point(3., 0., 0.)));
    enter(&mut app, "rw-1,0,0");
    assert_eq!(app.last_point, Some(point(2., 0., 0.)));
    enter(&mut app, "");
    let mut origins = app
        .document
        .objects()
        .filter_map(|o| {
            if o.id() == source {
                return None;
            }
            let Geometry::PointCloud(c) = o.geometry() else {
                return None;
            };
            Some(c.points()[0].to_array())
        })
        .collect::<Vec<_>>();
    origins.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(origins, vec![[2., 0., 0.], [3., 0., 0.]]);
    assert!(snapshot.shares_storage_with(app.document.object(parent).unwrap().geometry_snapshot()));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnCrv"));
    assert_eq!(app.document.objects().count(), 2);
}
