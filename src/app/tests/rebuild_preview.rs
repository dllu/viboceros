use super::boolean_intersection::enter;
use super::*;
use viboceros_document::{ColorRgb, ObjectAttributes, SelectionMode};
use viboceros_geometry::{NurbsSurface, Tolerance, WeightedPoint3};

fn fixture() -> (VibocerosApp, viboceros_document::ObjectId) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,2 0,6,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, id)
}

#[test]
fn rebuild_preview_reuses_geometry_for_stationary_and_output_policy_edits() {
    let (mut app, id) = fixture();
    let layer = app.document.add_layer("Source", ColorRgb::BLACK).unwrap();
    app.document.set_objects_layer([id], layer).unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Rebuild");
    let prepared = app
        .rebuild_preview
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap();
    let geometry = prepared.outputs().next().unwrap().clone();
    let scene_geometry = app
        .rebuild_preview
        .as_ref()
        .unwrap()
        .scene()
        .unwrap()
        .objects()
        .next()
        .unwrap()
        .geometry_snapshot()
        .clone();
    for edit in ["Preview", "UDegree=3", "UDegree", ""] {
        enter(&mut app, edit);
        assert!(
            geometry.shares_storage_with(
                app.rebuild_preview
                    .as_ref()
                    .unwrap()
                    .prepared
                    .as_ref()
                    .unwrap()
                    .outputs()
                    .next()
                    .unwrap()
            )
        );
        assert!(
            scene_geometry.shares_storage_with(
                app.rebuild_preview
                    .as_ref()
                    .unwrap()
                    .scene()
                    .unwrap()
                    .objects()
                    .next()
                    .unwrap()
                    .geometry_snapshot()
            )
        );
    }
    enter(&mut app, "DeleteInput=No OutputLayer=Current");
    assert!(
        geometry.shares_storage_with(
            app.rebuild_preview
                .as_ref()
                .unwrap()
                .prepared
                .as_ref()
                .unwrap()
                .outputs()
                .next()
                .unwrap()
        )
    );
    assert_eq!(
        app.rebuild_preview
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        2
    );
    enter(&mut app, "UPointCount=2");
    assert!(
        geometry.shares_storage_with(
            app.rebuild_preview
                .as_ref()
                .unwrap()
                .prepared
                .as_ref()
                .unwrap()
                .outputs()
                .next()
                .unwrap()
        )
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "UPointCount=6");
    assert!(
        !geometry.shares_storage_with(
            app.rebuild_preview
                .as_ref()
                .unwrap()
                .prepared
                .as_ref()
                .unwrap()
                .outputs()
                .next()
                .unwrap()
        )
    );
    let ready = app
        .rebuild_preview
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .outputs()
        .next()
        .unwrap()
        .clone();
    enter(&mut app, "");
    assert!(app.rebuild_preview.is_none());
    assert_eq!(app.document.objects().last().unwrap().geometry(), &*ready);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().last().unwrap().geometry(), &*ready);
}

#[test]
fn stale_rebuild_events_reject_sources_metadata_selection_and_settings() {
    for change in 0..7 {
        let (mut app, id) = fixture();
        enter(&mut app, "Rebuild");
        let preferences = app.commands.surface_rebuild_defaults();
        match change {
            0 => {
                app.document
                    .set_object_names([(id, Some("Changed".into()))])
                    .unwrap();
            }
            1 => {
                app.document.add_group(None, [id]).unwrap();
            }
            2 => {
                app.document
                    .set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-10).unwrap());
            }
            3 => {
                app.document.clear_selection();
            }
            4 => {
                app.document
                    .set_object_geometry_user_text([id], "Code", Some("Changed"))
                    .unwrap();
            }
            5 => {
                let layer = app.document.add_layer("Other", ColorRgb::BLACK).unwrap();
                app.document.set_current_layer(layer).unwrap();
            }
            _ => {
                app.document.delete_object(id).unwrap();
            }
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "UDegree=4");
        assert!(app.object_prompt.is_none());
        assert!(app.rebuild_preview.is_none());
        assert_eq!(app.commands.surface_rebuild_defaults(), preferences);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn rebuild_refreshes_background_without_recomputing_geometry_and_cancellation_discards_scene() {
    let (mut app, _) = fixture();
    enter(&mut app, "Rebuild");
    let geometry = app
        .rebuild_preview
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .outputs()
        .next()
        .unwrap()
        .clone();
    let peer = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(9., 8., 7.).unwrap()))
        .unwrap();
    app.validate_rebuild_preview();
    let preview = app.rebuild_preview.as_ref().unwrap();
    assert!(
        geometry.shares_storage_with(preview.prepared.as_ref().unwrap().outputs().next().unwrap())
    );
    assert!(preview.scene().unwrap().object(peer).is_some());
    app.document
        .set_objects_color([peer], Some(ColorRgb::new(255, 0, 0)))
        .unwrap();
    app.validate_rebuild_preview();
    assert_eq!(
        app.rebuild_preview
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .object(peer)
            .unwrap()
            .attributes()
            .object_color(),
        ColorRgb::new(255, 0, 0)
    );
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    for mode in ["Wireframe", "Shaded", "Ghosted"] {
        enter(
            &mut app,
            &format!("SetDisplayMode Viewport=All Mode={mode}"),
        );
        assert!(app.rebuild_preview.is_some());
    }
    enter(&mut app, "Cancel");
    assert!(app.rebuild_preview.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn failed_rebuild_preparation_drops_scene_blocks_acceptance_and_recovers() {
    let mut app = test_app();
    for i in 0..16 {
        enter(
            &mut app,
            &format!(
                "SrfPt {},0,0 {},0,0 {},6,0 {},6,0",
                i * 10,
                i * 10 + 4,
                i * 10 + 4,
                i * 10
            ),
        );
    }
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Rebuild");
    assert!(app.rebuild_preview.as_ref().unwrap().scene().is_some());
    enter(&mut app, "UPointCount=256 VPointCount=256");
    let preview = app.rebuild_preview.as_ref().unwrap();
    assert!(preview.scene().is_none());
    assert!(preview.prepared.is_none());
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "UPointCount=10 VPointCount=10");
    assert!(app.rebuild_preview.as_ref().unwrap().scene().is_some());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.rebuild_preview.is_none());
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

fn native_surface(v: &serde_json::Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    p["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn rebuild_preview_and_acceptance_match_twelve_native_surface_outcomes() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_rebuild.json"
    ))
    .unwrap();
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let mut app = test_app();
        let layers = (0..3)
            .map(|i| {
                app.document
                    .add_layer(format!("Layer {i}"), ColorRgb::BLACK)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        app.document.set_current_layer(layers[2]).unwrap();
        let attrs = ObjectAttributes::on_layer(layers[0])
            .with_name("source-0")
            .with_object_color(ColorRgb::new(20, 40, 60))
            .try_with_user_text("Code", "attribute-0")
            .unwrap();
        let id = app
            .document
            .add_geometry_with_attributes(
                Geometry::NurbsSurface(native_surface(&v["before"][0]["definition"])),
                attrs,
            )
            .unwrap();
        app.document.add_group(None, [id]).unwrap();
        app.document.add_group(None, [id]).unwrap();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "Rebuild");
        enter(
            &mut app,
            &format!(
                "UDegree={} VDegree={} UPointCount={} VPointCount={} ReTrim=No DeleteInput={} OutputLayer={}",
                v["spec"]["degree"][0],
                v["spec"]["degree"][1],
                v["spec"]["count"][0],
                v["spec"]["count"][1],
                if v["spec"]["delete"] == true {
                    "Yes"
                } else {
                    "No"
                },
                if v["spec"]["current"] == true {
                    "Current"
                } else {
                    "Input"
                }
            ),
        );
        let preview = app.rebuild_preview.as_ref().unwrap();
        let scene = preview.scene().unwrap();
        let native = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(scene.objects().len(), native.len());
        for (object, record) in scene.objects().zip(native) {
            let actual = match object.geometry() {
                Geometry::NurbsSurface(s) => s,
                Geometry::Brep(b) => b.faces()[0].surface(),
                _ => panic!(),
            };
            let expected = native_surface(&record["definition"]);
            assert_eq!(actual.knots_u(), expected.knots_u());
            assert_eq!(actual.knots_v(), expected.knots_v());
            assert_eq!(
                actual.control_points().len(),
                expected.control_points().len()
            );
            for (a, b) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                assert!(a.point().distance_to(b.point()).unwrap() < 1e-6);
            }
            assert_eq!(
                object.attributes().layer_id(),
                layers[record["layer"].as_u64().unwrap() as usize]
            );
            assert_eq!(
                object.group_ids().len(),
                record["groups"].as_array().unwrap().len()
            );
        }
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        let prepared = preview
            .prepared
            .as_ref()
            .unwrap()
            .outputs()
            .next()
            .unwrap()
            .clone();
        enter(&mut app, "");
        assert_eq!(
            app.document.objects().last().unwrap().geometry(),
            &*prepared
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(
            app.document.objects().last().unwrap().geometry(),
            &*prepared
        );
    }
}

#[test]
fn rebuild_dialog_options_stay_independent_of_later_registry_defaults() {
    use viboceros_command::surface_rebuild::Options;
    let (mut app, _) = fixture();
    enter(&mut app, "Rebuild");
    let geometry = app
        .rebuild_preview
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .outputs()
        .next()
        .unwrap()
        .clone();
    app.commands.remember_surface_rebuild_options(Options {
        count: [2, 2],
        degree: [1, 1],
        ..Options::default()
    });
    enter(&mut app, "DeleteInput=No");
    let preview = app.rebuild_preview.as_ref().unwrap();
    assert_eq!(preview.prepared.as_ref().unwrap().options().count, [10, 10]);
    assert_eq!(app.commands.surface_rebuild_defaults().count, [10, 10]);
    assert!(
        geometry.shares_storage_with(preview.prepared.as_ref().unwrap().outputs().next().unwrap())
    );
    enter(&mut app, "Cancel");
    assert!(app.rebuild_preview.is_none());
}
