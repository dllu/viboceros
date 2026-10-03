//! Replays prescribed native input cameras, rather than deriving picks from output geometry.
use super::*;
use serde_json::{Value, json};

#[path = "../../../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn normalized(mut rows: Vec<Value>) -> Value {
    rows.sort_by(|a, b| {
        let key = |v: &Value| {
            (
                v["type"].as_str().unwrap().to_owned(),
                v["witness"].as_bool().unwrap(),
                v["selected"].as_bool().unwrap(),
            )
        };
        key(a).cmp(&key(b)).then_with(|| {
            let bounds =
                |v: &Value| serde_json::from_value::<[[f64; 3]; 2]>(v["bounds"].clone()).unwrap();
            bounds(a).partial_cmp(&bounds(b)).unwrap()
        })
    });
    json!(rows)
}

fn native(value: &Value) -> Value {
    normalized(value.as_array().unwrap().iter().map(|v|json!({"type":v["type"],"witness":v["witness"],"selected":v["selected"],"bounds":v["bounds"],"vertices":v["vertices"],"points":v["points"]})).collect())
}

fn objects(document: &Document, witness: Option<ObjectId>) -> Value {
    normalized(document.objects().map(|object| {
        let geometry=object.geometry();let bounds=geometry.bounds();
        let (kind,vertices,points)=match geometry {
            Geometry::Point(p)=>("Point",Value::Null,json!([p.to_array()])),
            Geometry::Polyline(p)=>("Curve",Value::Null,json!(p.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>())),
            Geometry::Brep(b)=>("Brep",json!(b.vertices().iter().map(|v|v.point().to_array()).collect::<Vec<_>>()),Value::Null),
            _=>panic!("unexpected affine witness"),
        };
        json!({"type":kind,"witness":Some(object.id())==witness,"selected":document.is_selected(object.id()),
            "bounds":[bounds.min().to_array(),bounds.max().to_array()],"vertices":vertices,"points":points})
    }).collect())
}

fn assert_objects(actual: &Value, expected: &Value, label: &str) {
    let (actual, expected) = (actual.as_array().unwrap(), expected.as_array().unwrap());
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (a, b) in actual.iter().zip(expected) {
        let (mut a, mut b) = (a.clone(), b.clone());
        let vertices = |v: &mut Value| v.as_object_mut().unwrap().remove("vertices").unwrap();
        let (av, bv) = (vertices(&mut a), vertices(&mut b));
        if !av.is_null() {
            let mut remaining: Vec<[f64; 3]> = serde_json::from_value(bv).unwrap();
            for p in serde_json::from_value::<Vec<[f64; 3]>>(av).unwrap() {
                let index = remaining
                    .iter()
                    .position(|q| p.iter().zip(q).all(|(x, y)| (x - y).abs() < 1e-9))
                    .unwrap_or_else(|| panic!("{label}: missing vertex {p:?}: {remaining:?}"));
                remaining.remove(index);
            }
            assert!(remaining.is_empty(), "{label}");
        } else {
            assert!(bv.is_null());
        }
        let bounds = |v: &mut Value| v.as_object_mut().unwrap().remove("bounds").unwrap();
        let (ab, bb) = (bounds(&mut a), bounds(&mut b));
        test_json::close(
            &ab,
            &bb,
            label,
            if a["type"] == "Brep" { 5e-7 } else { 1e-9 },
            1e-12,
        );
        test_json::close(&a, &b, label, 1e-9, 1e-12);
    }
}

#[test]
fn affine_preview_pending_and_completed_geometry_match_60_native_captures() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/affine_preview.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/affine_preview.json"
    ))
    .unwrap();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 60);
    assert_eq!(observed["results"].as_array().unwrap().len(), 60);
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let label = op["id"].as_str().unwrap();
        assert_eq!(row["id"], label);
        let mut app = test_app();
        app.active_viewport = 1;
        for input in [
            "Point 3,3,0",
            "Polyline 2,1,0 4,1,0 3,-1,0 2,1,0",
            "Box 2,-5,0 4,-3,0 2",
            "SelAll",
        ] {
            enter(&mut app, input);
        }
        let witness = (op["cursor"] == "Degenerate").then(|| {
            app.document
                .add_geometry(Geometry::Point(point(0., 0., 0.)))
                .unwrap()
        });
        app.document.clear_history().unwrap();
        app.viewports[1] =
            crate::viewport::clip_tests::captured_view(&row["value"]["pending"]["camera"]);
        assert_objects(
            &objects(&app.document, witness),
            &native(&row["value"]["before"]),
            label,
        );
        let command = op["command"].as_str().unwrap();
        if command == "Rotate3D" {
            enter(&mut app, command);
        } else {
            enter(
                &mut app,
                &format!(
                    "{command} Copy={}",
                    if op["copy"] == true { "Yes" } else { "No" }
                ),
            );
        }
        assert!(app.affine_preview().is_none());
        enter(&mut app, "w0,0,0");
        if command == "Rotate3D" {
            enter(&mut app, "w1,1,2");
            enter(
                &mut app,
                if op["copy"] == true {
                    "Copy=Yes"
                } else {
                    "Copy=No"
                },
            );
        }
        if op["phase"] == "Direction" || op["phase"] == "ZeroDirection" {
            enter(
                &mut app,
                if op["phase"] == "ZeroDirection" {
                    "0"
                } else {
                    "-2"
                },
            );
        } else {
            enter(
                &mut app,
                match command {
                    "Scale" | "Scale1D" => "w4,0,0",
                    "Scale2D" => "w4,0,2",
                    "Rotate" => "w5,0,0",
                    "Rotate3D" => "w5,-1,0",
                    "Shear" => "w4,1,2",
                    _ => unreachable!(),
                },
            );
        }
        if op["phase"] == "Repeat" {
            enter(&mut app, "w2,2,0");
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let last = app.last_point;
        let defaults = (
            app.commands.copy_default(command),
            app.commands.transform_scalar_default(command),
        );
        let (destination, map) = crate::viewport::affine_preview::tests::captured_destination(
            &app.document,
            &row["value"]["pending"],
            crate::viewport::ViewportInput {
                drafting: crate::viewport::DraftingInput {
                    active: true,
                    osnap: if witness.is_some() {
                        viboceros_drafting::ObjectSnapModes::only(
                            viboceros_drafting::ObjectSnapKind::Point,
                        )
                    } else {
                        viboceros_drafting::ObjectSnapModes::NONE
                    },
                    ..Default::default()
                },
                affine_preview: Some(app.affine_preview().unwrap_or_else(|| {
                    panic!("{label}: {:?} {:?}", app.active_command, app.command_log)
                })),
                ..Default::default()
            },
        );
        assert!(app.update_affine_preview(Some(map)));
        assert!(!app.update_affine_preview(Some(map)));
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.last_point, last);
        assert_eq!(
            (
                app.commands.copy_default(command),
                app.commands.transform_scalar_default(command)
            ),
            defaults
        );
        assert_objects(
            &objects(&app.document, witness),
            &native(&row["value"]["pending"]["objects"]),
            label,
        );
        if witness.is_some() {
            assert_eq!(map, viboceros_geometry::AffineTransform3::identity());
        }
        if op["finish"] == "Cancel" {
            app.cancel_current_prompt_or_selection();
        } else {
            assert!(
                app.accept_filtered_drafting_point(destination, false),
                "{label}: {:?}",
                app.command_log
            );
            if op["copy"] == true {
                enter(&mut app, "");
            }
        }
        assert_objects(
            &objects(&app.document, witness),
            &native(&row["value"]["after"]),
            label,
        );
        assert!(app.active_command.is_none());
        assert!(app.transform_session.is_none());
        assert!(!app.update_affine_preview(None));
        if op["finish"] == "Click" {
            assert_eq!(app.document.undo_label(), Some(command));
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().count(), 3);
        } else {
            assert!(!app.document.can_undo());
        }
    }
}

#[test]
fn affine_preview_and_commit_retain_rotate_shear_plane_and_use_scale2d_destination_plane() {
    use viboceros_command::construction_plane::WorldPlane;
    for (command, reference, target, expected) in [
        ("Rotate", "w1,0,0", point(0., 1., 0.), point(-1., 2., 3.)),
        ("Shear", "w1,0,0", point(1., 1., 0.), point(2., 3., 3.)),
        ("Scale2D", "w1,0,0", point(2., 0., 0.), point(4., 1., 6.)),
    ] {
        let mut app = test_app();
        app.active_viewport = 1;
        enter(&mut app, "Point 2,1,3");
        enter(&mut app, "SelAll");
        enter(&mut app, command);
        enter(&mut app, "w0,0,0");
        enter(&mut app, reference);
        app.active_viewport = 2;
        app.viewports[2].set_construction_plane(WorldPlane::Front.frame());
        let preview = app.affine_preview().unwrap();
        let map = preview
            .definition
            .transform_at(
                preview
                    .frame
                    .unwrap_or(app.viewports[2].construction_plane()),
                target,
                app.document.tolerance(),
            )
            .unwrap();
        for (a, b) in map
            .transform_point(point(2., 1., 3.))
            .unwrap()
            .to_array()
            .into_iter()
            .zip(expected.to_array())
        {
            assert!((a - b).abs() < 1e-12);
        }
        assert!(app.accept_filtered_drafting_point(target, false));
        let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
            panic!("point")
        };
        for (a, b) in p.to_array().into_iter().zip(expected.to_array()) {
            assert!((a - b).abs() < 1e-12);
        }
    }
}
