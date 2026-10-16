use super::*;
use serde_json::{Value, json};
use viboceros_geometry::{Circle3, LineSegment, NurbsCurve, PointCloud3, Vector3, WeightedPoint3};

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn token(v: &Value) -> String {
    format!(
        "w{}",
        v.as_array()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn geometry(v: &Value) -> Geometry {
    match v["type"].as_str().unwrap() {
        "line" => Geometry::Line(
            LineSegment::try_new(p(&v["start"]), p(&v["end"]), Tolerance::DEFAULT).unwrap(),
        ),
        "circle" => Geometry::Circle(
            Circle3::try_new(
                p(&v["center"]),
                v["radius"].as_f64().unwrap(),
                Vector3::try_from(p(&v["normal"]).to_array())
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "nurbs" => Geometry::NurbsCurve(
            NurbsCurve::try_new_rational(
                v["degree"].as_u64().unwrap() as usize,
                v["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        WeightedPoint3::try_new(p(&v["point"]), v["weight"].as_f64().unwrap())
                            .unwrap()
                    })
                    .collect(),
                serde_json::from_value(v["knots"].clone()).unwrap(),
            )
            .unwrap(),
        ),
        _ => panic!("fixture source"),
    }
}

#[test]
fn orient_curve_subcurve_interaction_matches_all_26_native_placements_and_undo_redo() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/orient_on_curve_subcurve.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/orient_on_curve_subcurve_rhino_20261016.json"
    ))
    .unwrap();
    let ops = request["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), 26);
    assert_eq!(rows.len(), 26);
    for (op, row) in ops.iter().zip(rows) {
        let mut app = test_app();
        let guide_geometry = geometry(&op["source"]);
        let guide = app.document.add_geometry(guide_geometry.clone()).unwrap();
        let base = p(&op["base"]).to_array();
        let points = (0..4)
            .map(|i| {
                Point3::try_from(std::array::from_fn(|j| {
                    base[j] + if i == j + 1 { 1. } else { 0. }
                }))
                .unwrap()
            })
            .collect();
        let original = Geometry::PointCloud(PointCloud3::try_new(points).unwrap());
        let source = app.document.add_geometry(original.clone()).unwrap();
        let group = app
            .document
            .add_group(Some("guide".into()), [guide])
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let o = &op["options"];
        let options = viboceros_command::orient_curve::Options {
            copy: o["copy"].as_bool().unwrap_or(false),
            perpendicular: o["perpendicular"].as_bool().unwrap_or(false),
            on_curve: o["on_curve"].as_bool().unwrap_or(false),
            flip: o["flip"].as_bool().unwrap_or(false),
            ..Default::default()
        };
        enter(&mut app, &format!("OrientOnCrv {}", options.arguments()));
        if !options.on_curve {
            enter(&mut app, &token(&op["base"]));
        }
        enter(&mut app, "SubCrv");
        app.pick_orient_curve(Some(guide));
        enter(&mut app, &token(&op["subcurve"]["start"]));
        if let Some(length) = op["subcurve"]["length"].as_f64() {
            enter(&mut app, &length.to_string());
        }
        enter(&mut app, &token(&op["subcurve"]["end"]));
        assert!(
            app.document.object(source).unwrap().geometry() == &original,
            "{}",
            op["id"]
        );
        assert!(app.document.undo_label().is_none());
        if options.on_curve {
            enter(&mut app, &token(&op["base"]));
        }
        let preview = app.affine_preview().unwrap();
        let map = preview
            .definition
            .transform_at(
                preview.frame.unwrap(),
                p(&op["target"]),
                app.document.tolerance(),
            )
            .unwrap();
        let expected = row["value"]["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| !options.copy || v["original"] == false)
            .unwrap();
        let Geometry::PointCloud(cloud) = &original else {
            unreachable!()
        };
        for (actual, expected) in cloud
            .points()
            .iter()
            .map(|v| map.transform_point(*v).unwrap())
            .zip(expected["points"].as_array().unwrap())
        {
            assert!(
                actual.distance_to(p(expected)).unwrap() < 1.001e-6,
                "{}",
                op["id"]
            );
        }
        enter(&mut app, &token(&op["target"]));
        if options.copy {
            enter(&mut app, "");
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let mut actual=app.document.objects().filter_map(|obj|{let Geometry::PointCloud(c)=obj.geometry()else{return None};Some(json!({"original":obj.id()==source,"selected":app.document.is_selected(obj.id()),"points":c.points().iter().map(|p|p.to_array()).collect::<Vec<_>>()}))}).collect::<Vec<_>>();
        actual.sort_by_key(|v| v["original"].as_bool().unwrap());
        let mut expected = row["value"]["outputs"].as_array().unwrap().clone();
        expected.sort_by_key(|v| v["original"].as_bool().unwrap());
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            assert_eq!(a["original"], b["original"]);
            assert_eq!(a["selected"], b["selected"]);
            for (a, b) in a["points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["points"].as_array().unwrap())
            {
                assert!(p(a).distance_to(p(b)).unwrap() < 1.001e-6, "{}", op["id"]);
            }
        }
        assert_eq!(
            app.document.object(guide).unwrap().geometry(),
            &guide_geometry
        );
        assert!(app.document.group(group).is_some());
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnCrv"));
        assert_eq!(app.document.objects().count(), 2);
        assert_eq!(app.document.object(source).unwrap().geometry(), &original);
        app.document.redo().unwrap();
        assert_eq!(
            app.document.objects().count(),
            if options.copy { 3 } else { 2 }
        );
        assert_eq!(
            app.document.object(guide).unwrap().geometry(),
            &guide_geometry
        );
    }
}

#[test]
fn orient_curve_subcurve_length_direction_cancel_and_preview_leave_the_model_unchanged() {
    let mut app = test_app();
    let guide = app
        .document
        .add_geometry(geometry(
            &json!({"type":"line","start":[0,0,0],"end":[0,10,0]}),
        ))
        .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::Point(point(1., 0., 0.)))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "OrientOnCrv Copy=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "SubCrv");
    app.pick_orient_curve(Some(guide));
    enter(&mut app, "w0,8,0");
    app.update_subcurve_hover(point(0., 2., 0.));
    enter(&mut app, "D");
    enter(&mut app, "3");
    let preview = app.affine_preview().unwrap();
    let map = preview
        .definition
        .transform_at(
            preview.frame.unwrap(),
            point(0., 20., 0.),
            app.document.tolerance(),
        )
        .unwrap();
    assert!(
        map.transform_point(point(1., 0., 0.))
            .unwrap()
            .distance_to(point(0., 9., 0.))
            .unwrap()
            < 1e-12
    );
    enter(&mut app, "w0,6,0");
    enter(&mut app, "w0,7,0");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnCrv"));
    assert_eq!(app.document.objects().count(), 2);
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "OrientOnCrv");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "SubCrv");
    app.pick_orient_curve(Some(guide));
    enter(&mut app, "w0,8,0");
    app.update_subcurve_hover(point(0., 2., 0.));
    let preview = app.subcurve_draft_preview().unwrap();
    assert_eq!(
        preview.geometry.endpoints,
        [point(0., 8., 0.), point(0., 2., 0.)]
    );
    enter(&mut app, "20");
    assert!(app.affine_preview().is_none());
    enter(&mut app, "0");
    assert!(app.picking_orient_curve());
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.document.undo().unwrap().is_none());
}

#[test]
fn orient_curve_relative_input_after_clamping_matches_both_native_copy_batches() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/orient_on_curve_relative_input.json"
    ))
    .unwrap();
    let native:Value=serde_json::from_str(include_str!("../../../tools/rhino_oracle/observations/orient_on_curve_relative_input_rhino_20261016.json")).unwrap();
    for (op, row) in input["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let mut app = test_app();
        let guide = app.document.add_geometry(geometry(&op["source"])).unwrap();
        let cloud = Geometry::PointCloud(
            PointCloud3::try_new(vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(0., 1., 0.),
                point(0., 0., 1.),
            ])
            .unwrap(),
        );
        let source = app.document.add_geometry(cloud.clone()).unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "OrientOnCrv Copy=Yes");
        enter(&mut app, "w0,0,0");
        if op["subcurve"].is_object() {
            enter(&mut app, "SubCrv");
            app.pick_orient_curve(Some(guide));
            enter(&mut app, &token(&op["subcurve"]["start"]));
            enter(&mut app, &token(&op["subcurve"]["end"]));
        } else {
            app.pick_orient_curve(Some(guide));
        }
        enter(&mut app, &token(&op["target"]));
        let first = if op["subcurve"].is_object() { 4. } else { 5. };
        assert_eq!(app.last_point, Some(point(0., first, 0.)));
        enter(&mut app, "rw0,-1,0");
        enter(&mut app, "");
        let mut actual = app
            .document
            .objects()
            .filter_map(|o| {
                let Geometry::PointCloud(c) = o.geometry() else {
                    return None;
                };
                Some((
                    o.id() == source,
                    app.document.is_selected(o.id()),
                    c.points().to_vec(),
                ))
            })
            .collect::<Vec<_>>();
        actual.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.2[0].y().total_cmp(&b.2[0].y()))
        });
        let mut expected = row["value"]["outputs"].as_array().unwrap().clone();
        expected.sort_by(|a, b| {
            a["original"]
                .as_bool()
                .cmp(&b["original"].as_bool())
                .then_with(|| p(&a["points"][0]).y().total_cmp(&p(&b["points"][0]).y()))
        });
        assert_eq!(actual.len(), 3);
        for ((original, selected, points), b) in actual.iter().zip(expected) {
            assert_eq!(Some(*original), b["original"].as_bool());
            assert_eq!(Some(*selected), b["selected"].as_bool());
            for (a, b) in points.iter().zip(b["points"].as_array().unwrap()) {
                assert!(a.distance_to(p(b)).unwrap() < 1e-12);
            }
        }
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().count(), 2);
        assert_eq!(app.document.object(source).unwrap().geometry(), &cloud);
    }
}
