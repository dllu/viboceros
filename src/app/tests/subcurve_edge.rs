use super::subcurve_direction::{enter, position};
use super::*;
use crate::viewport::{ComponentClick, ComponentPick, EdgePick};
use serde_json::Value;
use viboceros_command::ComponentSelectionKind;
fn setup_case(app: &mut VibocerosApp, case: &str) -> (viboceros_document::ObjectId, usize) {
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 4.], [0., 6.], [0., 2.]],
        app.document.tolerance(),
    )
    .unwrap();
    let surface = (case == "surface_point").then(|| {
        viboceros_geometry::NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                point(0., 0., 0.),
                point(4., 0., 0.),
                point(0., 6., 0.),
                point(4., 6., 0.),
            ],
            vec![0., 0., 4., 4.],
            vec![0., 0., 6., 6.],
        )
        .unwrap()
    });
    let brep = if let Some(surface) = &surface {
        viboceros_geometry::Brep::try_surface_face(surface.clone(), app.document.tolerance())
            .unwrap()
    } else if case.starts_with("curved_") {
        viboceros_geometry::Brep::try_cylinder(
            viboceros_command::CommandContext::default().construction_plane,
            2.,
            0.,
            3.,
            app.document.tolerance(),
        )
        .unwrap()
    } else {
        brep
    };
    let index = brep
        .edges()
        .iter()
        .position(|e| {
            let a = e.curve().evaluate(*e.curve().domain().start()).unwrap();
            let b = e.curve().evaluate(*e.curve().domain().end()).unwrap();
            if case.starts_with("curved_") {
                return e.curve().is_closed().unwrap() && a.z().abs() < 1e-9;
            }
            (a == point(0., 0., 0.) && b == point(4., 0., 0.))
                || (b == point(0., 0., 0.) && a == point(4., 0., 0.))
        })
        .unwrap();
    let geometry = surface
        .map(Geometry::NurbsSurface)
        .unwrap_or(Geometry::Brep(brep));
    let id = app.document.add_geometry(geometry).unwrap();
    (id, index)
}
#[test]
fn subcurve_edge_replays_native_forced_copy_metadata_markers_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_edge.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        let mut app = test_app();
        let (parent, index) = setup_case(&mut app, case);
        app.document
            .set_object_names([(parent, Some("parent".into()))])
            .unwrap();
        app.document
            .set_object_user_text([parent], "source", Some("original"))
            .unwrap();
        let group = app
            .document
            .add_group(Some("parent".into()), [parent])
            .unwrap();
        let original = app
            .document
            .object(parent)
            .unwrap()
            .geometry_snapshot()
            .clone();
        let input = app.document.object(parent).unwrap().attributes().layer_id();
        let output = app.document.add_layer("output", ColorRgb::BLACK).unwrap();
        app.document.set_current_layer(output).unwrap();
        app.document.clear_history().unwrap();
        app.handle_viewport_action(ViewportOutput {
            component_click: Some(ComponentClick {
                picks: vec![ComponentPick {
                    object: parent,
                    index,
                    kind: ComponentSelectionKind::BrepEdge,
                }],
                preselection: true,
                modifiers: egui::Modifiers::NONE,
            }),
            ..Default::default()
        });
        enter(
            &mut app,
            &format!(
                "SubCrv Copy={} Mode={} FromMidpoint={}",
                if case == "point_copy_yes" {
                    "Yes"
                } else {
                    "No"
                },
                if case == "mark_ends" || case == "curved_mark" {
                    "MarkEnds"
                } else {
                    "Shorten"
                },
                if case == "midpoint" || case == "curved_midpoint" {
                    "Yes"
                } else {
                    "No"
                }
            ),
        );
        assert_eq!(app.subcurve_prompt.as_ref().unwrap().edge, Some(index));
        for p in v["points"].as_array().unwrap() {
            assert!(app.accept_drafting_point(position(p)), "{case}");
        }
        assert!(app.subcurve_prompt.is_none(), "{case}");
        assert!(
            original.shares_storage_with(app.document.object(parent).unwrap().geometry_snapshot()),
            "{case}"
        );
        assert_eq!(
            app.document.objects().len(),
            v["after"].as_array().unwrap().len(),
            "{case}"
        );
        for (o, expected) in app
            .document
            .objects()
            .skip(1)
            .zip(v["after"].as_array().unwrap().iter().skip(1))
        {
            if let Some(p) = expected.get("point") {
                let Geometry::Point(actual) = o.geometry() else {
                    panic!()
                };
                assert!(actual.distance_to(position(p)).unwrap() < 1e-6);
                assert_eq!(o.attributes().layer_id(), output);
                assert!(o.group_ids().is_empty());
                assert!(o.attributes().name().is_none());
                assert!(o.attributes().user_text().is_empty());
                assert!(!app.document.is_selected(o.id()));
            } else {
                let c = o.geometry().curve_ref().unwrap();
                let samples = expected["samples"].as_array().unwrap();
                assert!(
                    c.start_point()
                        .unwrap()
                        .distance_to(position(&samples[0]))
                        .unwrap()
                        < 1e-6,
                    "{case}"
                );
                assert!(
                    c.end_point()
                        .unwrap()
                        .distance_to(position(samples.last().unwrap()))
                        .unwrap()
                        < 1e-6,
                    "{case}"
                );
                for p in expected["samples"].as_array().unwrap() {
                    let p = position(p);
                    let t = c.closest_parameter(p, app.document.tolerance()).unwrap();
                    assert!(
                        c.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                        "{case}"
                    );
                }
                assert_eq!(o.attributes().layer_id(), input);
                assert_eq!(o.attributes().name(), Some("parent"));
                assert_eq!(o.attributes().user_text()["source"], "original");
                assert_eq!(o.group_ids(), [group]);
                assert!(app.document.is_selected(o.id()));
            }
        }
        let result_objects = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.selected_object_count(), 0);
        enter(&mut app, "Redo");
        assert_eq!(
            app.document.objects().len(),
            v["redo"].as_array().unwrap().len()
        );
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            result_objects
        );
    }
}
#[test]
fn subcurve_edge_command_first_click_and_typed_reference_preserve_parent_until_commit() {
    for typed in [false, true] {
        let mut app = test_app();
        let (parent, index) = setup_case(&mut app, "point_default");
        app.document.clear_history().unwrap();
        enter(&mut app, "SubCrv Copy=No");
        assert!(app.picking_subcurve_edge());
        if typed {
            enter(&mut app, &format!("Edge={parent},{index}"));
        } else {
            app.handle_viewport_action(ViewportOutput {
                edge_click: Some(vec![EdgePick {
                    object: parent,
                    edge: index,
                }]),
                ..Default::default()
            });
        }
        assert_eq!(app.subcurve_prompt.as_ref().unwrap().edge, Some(index));
        enter(&mut app, "1,0");
        app.update_subcurve_hover(point(3., 0., 0.));
        let preview = app.subcurve_draft_preview().unwrap();
        assert!(
            (preview
                .geometry
                .curve
                .length(app.document.tolerance())
                .unwrap()
                - 2.)
                .abs()
                < 1e-8
        );
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.can_undo());
        enter(&mut app, "3,0");
        assert_eq!(app.document.objects().len(), 2);
    }
}

#[test]
fn subcurve_edge_ambiguous_choices_cancel_and_reject_changed_parent() {
    let mut app = test_app();
    let (parent, index) = setup_case(&mut app, "point_default");
    let (other, other_index) = setup_case(&mut app, "point_default");
    enter(&mut app, "SubCrv");
    let candidates = vec![
        EdgePick {
            object: parent,
            edge: index,
        },
        EdgePick {
            object: other,
            edge: other_index,
        },
    ];
    app.handle_viewport_action(ViewportOutput {
        edge_click: Some(candidates.clone()),
        ..Default::default()
    });
    assert!(app.component_selection.has_choices());
    assert!(app.picking_subcurve_edge());
    enter(&mut app, "Cancel");
    assert!(!app.component_selection.has_choices());
    assert!(app.picking_subcurve_edge());
    app.handle_viewport_action(ViewportOutput {
        edge_click: Some(candidates),
        ..Default::default()
    });
    enter(&mut app, "2");
    assert_eq!(app.subcurve_prompt.as_ref().unwrap().source, Some(other));
    enter(&mut app, "1,0");
    app.update_subcurve_hover(point(3., 0., 0.));
    assert!(app.subcurve_draft_preview().is_some());
    let changed = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 8.], [0., 6.], [0., 2.]],
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .replace_object_geometries([(other, Geometry::Brep(changed))])
        .unwrap();
    assert!(app.subcurve_draft_preview().is_none());
    assert!(app.picking_subcurve_edge());
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn subcurve_edge_stale_preselection_is_rejected_before_command_start() {
    let mut app = test_app();
    let (parent, index) = setup_case(&mut app, "point_default");
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: parent,
            index,
            kind: ComponentSelectionKind::BrepEdge,
        }],
        preselection: true,
        modifiers: egui::Modifiers::NONE,
    });
    app.document
        .replace_object_geometries([(parent, Geometry::Point(point(0., 0., 0.)))])
        .unwrap();
    enter(&mut app, "SubCrv");
    assert!(app.subcurve_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.command_log.back().unwrap().contains("source changed"));
}
