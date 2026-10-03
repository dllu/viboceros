//! Replay independently prescribed command input against unmodified native snapshots.
use super::*;
use serde_json::{Value, json};
use viboceros_document::ObjectColorSource;
use viboceros_geometry::Vector3;

#[path = "../../../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}

#[test]
fn rejected_targets_and_escape_keep_accepted_edits_and_last_successful_copy_choice() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    app.document.clear_history().unwrap();
    enter(&mut app, "Scale _Copy=_Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "2");
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.active_command.is_some());
    let accepted = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Copy=No");
    enter(&mut app, "0");
    enter(&mut app, "NaN");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        accepted
    );
    assert!(app.active_command.is_some());
    assert_eq!(app.commands.copy_default("Scale"), Some(true));
    enter(&mut app, "Copy=Maybe");
    assert!(app.active_command.is_some());
    enter(&mut app, "Copy Yes");
    enter(&mut app, "3");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Copy=No");
    app.cancel_current_prompt_or_selection(); // The real GUI Escape route.
    assert!(app.active_command.is_none());
    assert!(app.transform_session.is_none());
    assert_eq!(app.commands.copy_default("Scale"), Some(true));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn empty_command_input_accepts_scalar_defaults_and_shows_the_pending_value() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    enter(&mut app, "Scale 0,0,0 3");
    enter(&mut app, "Undo");
    enter(&mut app, "Scale");
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.transform_default_hint().as_deref(),
        Some("Enter accepts the default: 3")
    );
    enter(&mut app, ""); // The command field submitted by the real Enter key.
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("Scale"));
    let Geometry::Point(point) = app.document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert_eq!(point.to_array(), [6., 9., 12.]);
}

fn snapshot(app: &VibocerosApp, sources: &[ObjectId]) -> Value {
    let objects = app.document.objects().collect::<Vec<_>>();
    let groups = app.document.groups().collect::<Vec<_>>();
    json!({
        "objects": objects.iter().map(|object| {
            let Geometry::Point(point) = object.geometry() else { panic!("unexpected geometry") };
            let attributes = object.attributes();
            let color = attributes.object_color();
            json!({
                "source": sources.iter().position(|id| *id == object.id()),
                "selected": app.document.is_selected(object.id()),
                "point": point.to_array(),
                "name": attributes.name().unwrap_or(""),
                "color": [color.red, color.green, color.blue],
                "color_source": match attributes.color_source() {
                    ObjectColorSource::Object => "ColorFromObject",
                    ObjectColorSource::Layer => "ColorFromLayer",
                    ObjectColorSource::Material => "ColorFromMaterial",
                    ObjectColorSource::Parent => "ColorFromParent",
                },
                "current_layer": attributes.layer_id() == app.document.current_layer_id(),
                "groups": object.group_ids().iter().map(|id| groups.iter().position(|group| group.id() == *id).unwrap()).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "groups": groups.iter().map(|group| json!({
            "members": objects.iter().enumerate().filter_map(|(i, obj)| obj.group_ids().contains(&group.id()).then_some(i)).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })
}

#[test]
fn repeated_transform_input_geometry_selection_groups_and_history_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy.json"),
        49,
        Invocation::Prompt,
    );
}

#[test]
fn complete_transform_invocations_geometry_selection_groups_and_history_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_script.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy_script.json"),
        56,
        Invocation::Registry,
    );
}

#[test]
fn transform_prompts_with_partial_groups_and_untouched_peers_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_script.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy_script.json"),
        56,
        Invocation::Prompt,
    );
}

#[test]
fn automatic_scale_centers_with_rotated_and_tilted_planes_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_center.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy_center.json"),
        18,
        Invocation::Prompt,
    );
}

#[test]
fn exact_and_near_identity_transform_prompts_and_invocations_match_native() {
    for invocation in [Invocation::Prompt, Invocation::Registry] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_identity.json"),
            include_str!("../../../tools/rhino_oracle/observations/transform_copy_identity.json"),
            64,
            invocation,
        );
    }
}

#[test]
fn transform_scalar_defaults_and_cancellation_match_self_seeded_native_sessions() {
    for invocation in [Invocation::Prompt, Invocation::RegistrySeeds] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_default.json"),
            include_str!("../../../tools/rhino_oracle/observations/transform_copy_default.json"),
            80,
            invocation,
        );
    }
}

#[derive(Clone, Copy)]
enum Invocation {
    Prompt,
    Registry,
    RegistrySeeds,
}

fn replay_native(request: &str, observed: &str, count: usize, invocation: Invocation) {
    let request: Value = serde_json::from_str(request).unwrap();
    let observed: Value = serde_json::from_str(observed).unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), count);
    assert_eq!(results.len(), operations.len());
    assert_eq!(observed["engine"], "rhino");
    let mut failures = Vec::new();
    let mut commands = None;
    for (operation, row) in operations.iter().zip(results) {
        let label = operation["id"].as_str().unwrap();
        assert_eq!(row["id"], label);
        let expected = &row["value"];
        let mut app = test_app();
        if let Some(registry) = commands.take() {
            // The native capture cleans up owned sources between cases while
            // retaining its command-instance preferences. Use fresh documents
            // with the same registry; no measured defaults seed this state.
            app.commands = registry;
        }
        // Match the native SDK source construction record. Keeping this
        // baseline also exercises SelLast when the tested transform is a no-op.
        app.document
            .begin_transaction("Transform source setup")
            .unwrap();
        app.active_viewport = 1; // Top; the native probe explicitly uses WorldXY.
        if let Some(plane) = operation.get("cplane") {
            let coords = |value: &Value| {
                [
                    value[0].as_f64().unwrap(),
                    value[1].as_f64().unwrap(),
                    value[2].as_f64().unwrap(),
                ]
            };
            app.viewports[app.active_viewport].set_construction_plane(
                viboceros_geometry::Frame3::try_from_directions(
                    Point3::try_from(coords(&plane["origin"])).unwrap(),
                    Vector3::try_from(coords(&plane["x_axis"])).unwrap(),
                    Vector3::try_from(coords(&plane["y_axis"])).unwrap(),
                    app.document.tolerance(),
                )
                .unwrap(),
            );
        }
        let sources = operation["sources"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, coords)| {
                let point = point(
                    coords[0].as_f64().unwrap(),
                    coords[1].as_f64().unwrap(),
                    coords[2].as_f64().unwrap(),
                );
                let attributes =
                    viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                        .with_name(format!("source-{index}"))
                        .with_object_color(ColorRgb::new(10 + index as u8, 30, 50));
                let id = app
                    .document
                    .add_geometry_with_attributes(Geometry::Point(point), attributes)
                    .unwrap();
                id
            })
            .collect::<Vec<_>>();
        if operation["grouped"].as_bool().unwrap() {
            app.document
                .add_group(
                    Some(format!("RepeatSource_{label}")),
                    sources.iter().copied(),
                )
                .unwrap();
        }
        let selected = operation
            .get("selected")
            .map(|values| {
                values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|index| sources[index.as_u64().unwrap() as usize])
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| sources.clone());
        app.document
            .select_objects_direct(selected, SelectionMode::Replace)
            .unwrap();
        app.document.commit_transaction().unwrap();
        let compare = |actual: Value, phase: &str, failures: &mut Vec<String>| {
            let path = format!("{label}/{phase}");
            if std::panic::catch_unwind(|| {
                test_json::close(&actual, &expected[phase], &path, 1e-9, 0.)
            })
            .is_err()
            {
                failures.push(path);
            }
        };
        compare(snapshot(&app, &sources), "before", &mut failures);
        let use_registry = match invocation {
            Invocation::Prompt => false,
            Invocation::Registry => true,
            Invocation::RegistrySeeds => {
                operation["finish"] == "Automatic"
                    && !operation["inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|input| input == "Enter")
            }
        };
        match use_registry {
            false => {
                enter(&mut app, operation["command"].as_str().unwrap());
                assert!(
                    app.transform_session.is_some(),
                    "{label}: {:?}",
                    app.command_log
                );
                for input in operation["inputs"].as_array().unwrap() {
                    enter(&mut app, input.as_str().unwrap());
                }
                if operation["finish"] != "Automatic" {
                    enter(&mut app, operation["finish"].as_str().unwrap());
                }
            }
            true => {
                let input = std::iter::once(operation["command"].as_str().unwrap())
                    .chain(
                        operation["inputs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| value.as_str().unwrap().trim_start_matches('w')),
                    )
                    .collect::<Vec<_>>()
                    .join(" ");
                let context = viboceros_command::CommandContext {
                    construction_plane: app.viewports[app.active_viewport].construction_plane(),
                };
                app.commands
                    .execute_in_context(&mut app.document, &input, context)
                    .unwrap();
            }
        }
        assert!(
            app.active_command.is_none(),
            "{label}: {:?}",
            app.command_log
        );
        compare(snapshot(&app, &sources), "after", &mut failures);
        if operation["sel_last"].as_bool().unwrap() {
            enter(&mut app, "SelLast");
            compare(snapshot(&app, &sources), "last", &mut failures);
        }
        if !expected["undo"].is_null() {
            enter(&mut app, "Undo");
            assert!(
                app.document.undo_label() == Some("Transform source setup"),
                "{label}: more than one transform Undo entry"
            );
            compare(snapshot(&app, &sources), "undo", &mut failures);
            enter(&mut app, "Redo");
            compare(snapshot(&app, &sources), "redo", &mut failures);
        } else {
            assert!(
                app.document.undo_label() == Some("Transform source setup"),
                "{label}: canceled or identity command recorded history"
            );
        }
        commands = Some(app.commands);
    }
    assert!(failures.is_empty(), "native differences: {failures:?}");
}
