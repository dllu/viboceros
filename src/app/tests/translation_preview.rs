//! Native cursor inputs exercise pending previews, accepted copies, and cancellation.
use super::*;
use serde_json::{Value, json};
use viboceros_command::construction_plane::WorldPlane;

#[path = "../../../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn normalized(mut objects: Vec<Value>) -> Value {
    objects.sort_by(|a, b| {
        let key = |object: &Value| {
            (
                object["type"].as_str().unwrap().to_owned(),
                object["reference"].as_bool().unwrap(),
                object["selected"].as_bool().unwrap(),
            )
        };
        key(a).cmp(&key(b)).then_with(|| {
            let bounds = |object: &Value| {
                serde_json::from_value::<[[f64; 3]; 2]>(object["bounds"].clone()).unwrap()
            };
            bounds(a).partial_cmp(&bounds(b)).unwrap()
        })
    });
    Value::Array(objects)
}

fn native_objects(state: &Value) -> Value {
    normalized(state.as_array().unwrap().iter().map(|object|json!({
        "type":object["type"],"bounds":object["bounds"],"selected":object["selected"],"reference":object["reference"],
        "vertices":sorted_vertices(object["vertices"].clone())
    })).collect())
}

fn sorted_vertices(value: Value) -> Value {
    if value.is_null() {
        return value;
    }
    let mut points: Vec<[f64; 3]> = serde_json::from_value(value).unwrap();
    points.sort_by(|a, b| a.partial_cmp(b).unwrap());
    json!(points)
}

fn assert_objects(actual: &Value, expected: &Value, label: &str) {
    let actual = actual.as_array().unwrap();
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (a, b) in actual.iter().zip(expected) {
        let mut a = a.clone();
        let mut b = b.clone();
        let a_bounds = a.as_object_mut().unwrap().remove("bounds").unwrap();
        let b_bounds = b.as_object_mut().unwrap().remove("bounds").unwrap();
        // Public shaded box bounds can include rounding of cached render-mesh coordinates.
        // Compare the actual B-rep vertices, point coordinates, and curve bounds at 1e-9.
        test_json::close(
            &a_bounds,
            &b_bounds,
            label,
            if a["type"] == "Brep" { 5e-7 } else { 1e-9 },
            1e-12,
        );
        test_json::close(&a, &b, label, 1e-9, 1e-12);
    }
}

fn objects(document: &Document) -> Value {
    display_objects(document, None)
}

fn display_objects(document: &Document, preview_reference: Option<ObjectId>) -> Value {
    normalized(
        document
            .objects()
            .map(|object| {
                let bounds = object.geometry().bounds();
                let (kind, reference) = match object.geometry() {
                    Geometry::Point(_) => ("Point", false),
                    Geometry::Brep(_) => ("Brep", false),
                    Geometry::Polyline(_) => ("Curve", false),
                    Geometry::Circle(_) => ("Curve", true),
                    _ => panic!("unexpected translation preview witness"),
                };
                let vertices=match object.geometry() {
                    Geometry::Brep(brep)=>sorted_vertices(json!(brep.vertices().iter().map(|v|v.point().to_array()).collect::<Vec<_>>())),
                    _=>Value::Null,
                };
                json!({"type":kind,"bounds":[bounds.min().to_array(),bounds.max().to_array()],
            "selected":document.is_selected(object.id()) || preview_reference==Some(object.id()),"reference":reference,"vertices":vertices})
            })
            .collect(),
    )
}

#[test]
fn translation_preview_pending_completion_and_cancellation_match_36_native_cursor_captures() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/translation_preview.json"
    ))
    .unwrap();
    let captures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/translation_preview.json"
    ))
    .unwrap();
    assert_eq!(fixtures["operations"].as_array().unwrap().len(), 36);
    assert_eq!(captures["results"].as_array().unwrap().len(), 36);
    for (operation, capture) in fixtures["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(captures["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], capture["id"]);
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
        let normal = operation["placement"] == "Normal";
        let reference = normal.then(|| {
            app.document
                .add_geometry(Geometry::Circle(
                    viboceros_geometry::Circle3::try_new(
                        point(0., 0., 0.),
                        3.,
                        WorldPlane::Top.frame().z_axis(),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                ))
                .unwrap()
        });
        app.document.clear_history().unwrap();
        app.viewports[1] =
            crate::viewport::clip_tests::captured_view(&capture["value"]["pending"]["camera"]);
        assert_objects(
            &objects(&app.document),
            &native_objects(&capture["value"]["before"]),
            "before",
        );
        let command = operation["command"].as_str().unwrap();
        let placement = operation["placement"].as_str().unwrap();
        enter(
            &mut app,
            &format!(
                "{command}{}",
                if placement == "Vertical" {
                    " Vertical"
                } else {
                    ""
                }
            ),
        );
        assert!(
            app.translation_session
                .as_ref()
                .unwrap()
                .preview()
                .is_none()
        );
        if let Some(reference) = reference {
            enter(&mut app, "Normal");
            assert!(app.accept_move_normal_reference(reference, None));
        }
        enter(&mut app, if normal { "w3,0,0" } else { "w0,0,0" });
        let repeated = matches!(
            placement,
            "Repeat" | "FromLastPoint" | "UseLastDistance" | "UseLastDirection"
        );
        if repeated {
            enter(&mut app, "w-6,0,0");
            if placement != "Repeat" {
                enter(&mut app, &format!("{placement}=Yes"));
            }
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let last_point = app.last_point;
        let original_base = if normal {
            point(3., 0., 0.)
        } else {
            point(0., 0., 0.)
        };
        let preview = app.translation_session.as_ref().unwrap().preview().unwrap();
        assert_eq!(preview.base, original_base); // FromLastPoint must not change the map's source base.
        assert_eq!(preview.sources.len(), 3); // Repeated copies stay committed model objects.
        let (destination, map) = crate::viewport::translation_preview::tests::captured_destination(
            &app.document,
            &capture["value"]["pending"],
            crate::viewport::ViewportInput {
                drafting: crate::viewport::DraftingInput {
                    active: true,
                    ..Default::default()
                },
                translation_constraint: app.translation_constraint(),
                translation_preview: Some(preview),
                ..Default::default()
            },
        );
        assert!(app.update_translation_preview(Some(map)));
        assert!(!app.update_translation_preview(Some(map)));
        assert_eq!(map.transform_point(original_base).unwrap(), destination);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.last_point, last_point);
        assert_objects(
            &display_objects(&app.document, reference),
            &native_objects(&capture["value"]["pending"]["objects"]),
            "pending",
        );
        if operation["finish"] == "Cancel" {
            app.cancel_current_prompt_or_selection();
        } else {
            assert!(app.apply_translation_step(destination));
            if command == "Copy" {
                enter(&mut app, "");
            }
        }
        assert_objects(
            &objects(&app.document),
            &native_objects(&capture["value"]["after"]),
            operation["id"].as_str().unwrap(),
        );
        assert!(app.active_command.is_none());
        assert!(app.translation_session.is_none());
        assert!(!app.update_translation_preview(None));
        if repeated || operation["finish"] == "Click" {
            assert_eq!(app.document.undo_label(), Some(command));
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().count(), if normal { 4 } else { 3 });
        } else {
            assert!(!app.document.can_undo());
        }
    }
}
