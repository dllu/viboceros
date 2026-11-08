//! Sharp OffsetSrf document admission, including identical copies and collapsed walls.
#[path = "support/offset_surface.rs"]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
use viboceros_command::{CommandContext, CommandRegistry, offset_surface};

#[test]
fn offsets_match_native_geometry_metadata_identity_history_and_archives() {
    let mut count = 0;
    for (id, value) in inputs::rows() {
        if inputs::diagnostic(&id, &value) {
            continue;
        }
        println!("OFFSET_SURFACE_COMMAND_CASE {id}");
        let mut doc = inputs::setup(&value);
        let state = inputs::State::capture(&doc);
        let mut error = inputs::check(&doc, &value["before"], false);
        let r = CommandRegistry::with_builtins();
        inputs::select_sources(&mut doc);
        if inputs::success(&value) {
            let text = inputs::command(&value);
            let result = if inputs::postselected(&value) {
                r.execute_postselected(&mut doc, &text, CommandContext::default())
            } else {
                r.execute(&mut doc, &text)
            };
            result.unwrap_or_else(|e| panic!("{id}: {text}: {e}"));
        } else {
            let before = format!("{doc:?}");
            let _ = offset_surface::prepare(&doc, inputs::initial(&value), &Default::default());
            assert_eq!(format!("{doc:?}"), before);
            if inputs::postselected(&value) {
                inputs::clear_command_selection(&mut doc);
            }
        }
        let mut record = state.verify(&mut doc, &value);
        error = error.max(record["maximum_geometry_error"].as_f64().unwrap());
        record["maximum_geometry_error"] = error.into();
        record["id"] = id.into();
        println!("OFFSET_SURFACE_COMMAND {record}");
        count += 1;
    }
    println!("OFFSET_SURFACE_COMMAND_COUNT {count}");
}

#[test]
fn candidates_are_pure_and_reject_stale_sources_and_model_settings_atomically() {
    let value = inputs::rows()
        .into_iter()
        .find(|(id, _)| id == "offset_q_plain")
        .unwrap()
        .1;
    for edit in [
        "geometry",
        "attributes",
        "groups",
        "text",
        "locked",
        "selection",
        "tolerance",
        "units",
        "layer",
    ] {
        let mut doc = inputs::setup(&value);
        let id = inputs::guide(&doc, 0);
        let before = format!("{doc:?}");
        let prepared =
            offset_surface::prepare(&doc, Default::default(), &Default::default()).unwrap();
        let scene = prepared.preview(&doc).unwrap();
        assert_eq!(scene.objects().len(), doc.objects().len() + 1);
        assert_eq!(format!("{doc:?}"), before);
        match edit {
            "geometry" => {
                let g = doc
                    .object(id)
                    .unwrap()
                    .geometry()
                    .transformed(
                        viboceros_geometry::AffineTransform3::from_translation(
                            viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
                        ),
                        doc.tolerance(),
                    )
                    .unwrap();
                doc.replace_object_geometries([(id, g)]).unwrap();
            }
            "attributes" => {
                doc.set_object_names([(id, Some("changed".into()))])
                    .unwrap();
            }
            "groups" => {
                doc.add_group(Some("another".into()), [id]).unwrap();
            }
            "text" => {
                doc.set_object_geometry_user_text([id], "changed", Some("yes"))
                    .unwrap();
            }
            "locked" => {
                doc.set_objects_locked([id], true).unwrap();
            }
            "selection" => {
                doc.clear_selection();
            }
            "tolerance" => {
                doc.set_tolerance(
                    viboceros_geometry::Tolerance::try_new(1e-5, 1e-9, 1e-6).unwrap(),
                );
            }
            "units" => {
                doc.set_units(viboceros_geometry::LengthUnitSystem::Meters, false)
                    .unwrap();
            }
            "layer" => {
                let layer = doc
                    .add_layer("changed", viboceros_document::ColorRgb::BLACK)
                    .unwrap();
                doc.set_current_layer(layer).unwrap();
            }
            _ => unreachable!(),
        }
        let changed = format!("{doc:?}");
        assert!(!prepared.is_current(&doc), "{edit}");
        assert!(matches!(
            prepared.commit(&mut doc, false),
            Err(viboceros_command::CommandError::OffsetSurfaceStale)
        ));
        assert_eq!(format!("{doc:?}"), changed, "{edit}");
    }
    let mut doc = inputs::setup(&value);
    let id = inputs::guide(&doc, 0);
    let sentinel = doc.objects().next().unwrap().id();
    doc.set_transient_selection([id, sentinel]).unwrap();
    let before = format!("{doc:?}");
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, "OffsetSrf 2")
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
    println!("OFFSET_SURFACE_ATOMIC_GUARDS 10");
}
