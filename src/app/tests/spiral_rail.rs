use super::*;
use viboceros_command::spiral_input::Slot;
use viboceros_geometry::LineSegment;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn spiral_rail_frontend_native_sources_click_routes_curves_selection_and_history() {
    for (request, native) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/spiral_around_curve.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/spiral_around_curve_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/spiral_around_topology.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/spiral_around_topology_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/spiral_relative_points.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/spiral_relative_points_rhino_20261017.json"
            ),
        ),
    ] {
        let request: viboceros_oracle::ProbeRequest = serde_json::from_str(request).unwrap();
        let native: serde_json::Value = serde_json::from_str(native).unwrap();
        for (op, row) in request
            .operations
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let (id, kind, f) = match op {
                viboceros_oracle::Operation::HelixCommand { id, fixture } => (id, "Helix", fixture),
                viboceros_oracle::Operation::SpiralCommand { id, fixture } => {
                    (id, "Spiral", fixture)
                }
                _ => panic!(),
            };
            let mut app = test_app();
            let guides = f
                .source_geometries(app.document.tolerance())
                .unwrap()
                .into_iter()
                .map(|g| app.document.add_geometry(g).unwrap())
                .collect::<Vec<_>>();
            let snapshots = guides
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            app.document.clear_history().unwrap();
            if f.preselect {
                app.document
                    .select_object(guides[0], SelectionMode::Replace)
                    .unwrap();
            }
            app.last_point = f.prior_point.map(|p| Point3::try_from(p).unwrap());
            if let Some([p, x, y]) = f.construction_plane {
                app.viewports[app.active_viewport].set_construction_plane(
                    Frame3::try_from_directions(
                        Point3::try_from(p).unwrap(),
                        viboceros_geometry::Vector3::try_from(x).unwrap(),
                        viboceros_geometry::Vector3::try_from(y).unwrap(),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                );
            }
            enter(&mut app, kind);
            for action in &f.actions {
                use viboceros_oracle::SpiralCommandAction as Action;
                match action {
                    Action::Relative {
                        relative: [x, y, z],
                    } => enter(&mut app, &format!("r{x},{y},{z}")),
                    Action::Point([x, y, z]) => enter(&mut app, &format!("w{x},{y},{z}")),
                    Action::Number(n) => enter(&mut app, &n.to_string()),
                    Action::Option(word) => enter(&mut app, word),
                    Action::Pick { curve, edge, .. } => {
                        if let Some(edge) = edge {
                            app.accept_component_click(crate::viewport::ComponentClick {
                                picks: vec![crate::viewport::ComponentPick {
                                    object: guides[*curve],
                                    index: *edge,
                                    kind: viboceros_command::ComponentSelectionKind::BrepEdge,
                                }],
                                preselection: false,
                                modifiers: egui::Modifiers::NONE,
                            });
                        } else {
                            app.apply_selection_click(crate::viewport::SelectionClick {
                                object_id: Some(guides[*curve]),
                                mode: SelectionMode::Replace,
                            });
                        }
                    }
                }
                assert!(app.command_input.is_empty(), "{id}: {:?}", app.command_log);
            }
            assert!(app.spiral_session.is_none(), "{id}: {:?}", app.command_log);
            let outputs = app
                .document
                .objects()
                .filter(|o| !guides.contains(&o.id()))
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), 1, "{id}");
            let Geometry::NurbsCurve(c) = outputs[0].geometry() else {
                panic!()
            };
            let expected = &row["value"]["after"]["outputs"][0];
            assert_eq!(
                app.document.is_selected(outputs[0].id()),
                expected["selected"].as_bool().unwrap()
            );
            assert_eq!(
                c.is_closed().unwrap(),
                expected["closed"].as_bool().unwrap(),
                "{id}"
            );
            let expected = &expected["curve"];
            assert_eq!(c.degree(), expected["degree"].as_u64().unwrap() as usize);
            let knots: Vec<f64> = serde_json::from_value(expected["knots"].clone()).unwrap();
            assert_eq!(c.knots().len(), knots.len(), "{id}");
            for (a, b) in c.knots().iter().zip(knots) {
                assert!((a - b).abs() <= 1e-6, "{id}: {a} != {b}");
            }
            let controls = expected["control_points"].as_array().unwrap();
            assert_eq!(c.control_points().len(), controls.len(), "{id}");
            let mut maximum_control_distance = 0_f64;
            for (a, b) in c.control_points().iter().zip(controls) {
                let point: [f64; 3] = serde_json::from_value(b["point"].clone()).unwrap();
                let distance = a
                    .point()
                    .distance_to(Point3::try_from(point).unwrap())
                    .unwrap();
                maximum_control_distance = maximum_control_distance.max(distance);
                assert!(
                    distance
                        <= if id == "around-closed-spatial" {
                            3e-6
                        } else {
                            1e-6
                        },
                    "{id}: {:?} != {point:?}",
                    a.point()
                );
                assert!((a.weight() - b["weight"].as_f64().unwrap()).abs() <= 1e-6);
            }
            // This measured diagnostic remains outside the 1e-6 qualified corpus.
            if id == "around-closed-spatial" {
                assert!(maximum_control_distance > 1e-6);
            }
            for (stage, action) in [("after", None), ("undo", Some(false)), ("redo", Some(true))] {
                if let Some(redo) = action {
                    assert_eq!(
                        if redo {
                            app.document.redo().unwrap()
                        } else {
                            app.document.undo().unwrap()
                        }
                        .as_deref(),
                        Some(kind)
                    );
                }
                for ((id, source), i) in guides.iter().zip(&snapshots).zip(0..) {
                    assert!(source.shares_storage_with(
                        app.document.object(*id).unwrap().geometry_snapshot()
                    ));
                    if f.guide_selection {
                        assert_eq!(
                            app.document.is_selected(*id),
                            row["value"][stage]["guide_selected"][i].as_bool().unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn spiral_rail_frontend_revalidates_sources_and_density_getters_without_pending_edits() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 10.), app.document.tolerance()).unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Helix AroundCurve");
    assert!(app.picking_spiral_rail());
    assert!(!app.document.is_selected(id));
    app.pick_spiral_rail(Some(id), None);
    assert!(app.document.is_selected(id));
    enter(&mut app, "NumPointsPerTurn");
    enter(&mut app, "4");
    assert_eq!(
        app.spiral_session
            .as_ref()
            .unwrap()
            .input
            .options()
            .points_per_turn,
        12
    );
    enter(&mut app, "8");
    assert_eq!(
        app.spiral_session
            .as_ref()
            .unwrap()
            .input
            .options()
            .points_per_turn,
        8
    );
    app.document.set_objects_locked([id], true).unwrap();
    assert!(!app.validate_spiral_rail());
    assert!(app.picking_spiral_rail());
    assert_eq!(
        app.spiral_session.as_ref().unwrap().input.slot(),
        Slot::Rail
    );
    assert_eq!(app.document.objects().len(), 1);
    app.document.set_objects_locked([id], false).unwrap();
    app.pick_spiral_rail(Some(id), None);
    enter(&mut app, "Cancel");
    assert!(app.spiral_session.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_ne!(app.document.undo_label(), Some("Helix"));
}

#[test]
fn spiral_relative_equal_flat_radius_fails_then_retries_without_document_edits() {
    let mut app = test_app();
    app.last_point = Some(p(10., 10., 0.));
    enter(&mut app, "Spiral Flat");
    for word in [
        "w5,2,0",
        "w6,2,0",
        "Mode=Turns",
        "Turns=1",
        "ReverseTwist=No",
        "r1,0,0",
    ] {
        enter(&mut app, word);
    }
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
    assert_eq!(
        app.spiral_session.as_ref().unwrap().input.slot(),
        Slot::EndRadius
    );
    enter(&mut app, "r2,0,0");
    assert!(app.spiral_session.is_none());
    assert_eq!(app.document.objects().len(), 1);
    let mut doc = Document::default();
    assert!(
        app.commands
            .execute(&mut doc, "Spiral Flat 5,2,0 6,2,0 1 Turns=1")
            .is_err()
    );
    assert_eq!(doc.objects().len(), 0);
    assert!(doc.undo_label().is_none());
}
