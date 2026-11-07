#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/arc_blend_preferences.rs"]
mod preferences;
#[path = "support/arc_blend_radius.rs"]
#[allow(dead_code)]
mod radius;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{
    CommandRegistry,
    arc_blend::{Input, Preferences},
};
use viboceros_document::Document;
#[test]
fn ordered_settings_survive_accept_cancel_history_and_document_changes() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in preferences::FAMILIES {
        let registry = CommandRegistry::with_builtins();
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if preferences::diagnostic(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            let v = &row["value"];
            let mut doc = reference_inputs::setup(v, &v["prescribed_case"]);
            let mut input =
                Input::with_preferences(doc.tolerance(), registry.arc_blend_preferences()).unwrap();
            assert_eq!(input.radius_difference(), None);
            doc.clear_selection();
            preferences::replay(&mut input, &doc, v, &registry);
            let accepted = v["success"].as_bool().unwrap();
            if accepted {
                registry
                    .execute(&mut doc, &input.arguments().unwrap())
                    .unwrap();
            }
            let saved = registry.arc_blend_preferences();
            assert_eq!(saved, preferences::expected(name), "{id}");
            let stations = preferences::native_stations(v);
            let samples = if stations > 0 {
                radius::independent_samples(&doc, v)
            } else {
                0.
            };
            let mut error = inputs::check(&doc, &v["after"], false);
            if accepted {
                doc.undo().unwrap();
                error = error.max(inputs::check(&doc, &v["undo"], false));
                assert!(!doc.can_undo());
                doc.redo().unwrap();
                error = error.max(inputs::check(&doc, &v["redo"], false));
            } else {
                assert!(!doc.can_undo());
                if v["prescribed_case"]["pending_redo"] == true {
                    doc.redo().unwrap();
                    error = error.max(inputs::check(&doc, &v["pending_redo"], false));
                    doc.undo().unwrap();
                }
            }
            let path = directory.path().join("settings.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &v["after"], true));
            assert_eq!(registry.arc_blend_preferences(), saved);
            println!(
                "ARC_BLEND_PREFERENCES_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 25);
}
#[test]
fn scripted_defaults_share_settings_but_rejected_input_and_other_registries_do_not() {
    let r = CommandRegistry::with_builtins();
    let other = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for s in ["Line -5,0,0 0,0,0", "Line 10,3,0 15,3,0"] {
        r.execute(&mut doc, s).unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    let base = format!("ArcBlend Curve={} 0,0,0 Curve={} 10,3,0", ids[0], ids[1]);
    r.execute(
        &mut doc,
        &format!("{base} RadiusDifference=5 Trim=Yes Join=No"),
    )
    .unwrap();
    let p = Preferences {
        radius_difference: 5.,
        trim: true,
        join: false,
    };
    assert_eq!(r.arc_blend_preferences(), p);
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for option in [
        "RadiusDifference=NaN",
        "Trim=No Join=Yes",
        "Ratio=-1",
        "RadiusDifference=2 Garbage=No",
    ] {
        assert!(r.execute(&mut doc, &format!("{base} {option}")).is_err());
        assert_eq!(r.arc_blend_preferences(), p);
        assert_eq!(format!("{doc:?}"), before);
    }
    assert!(!r.remember_arc_blend_preferences(Preferences {
        radius_difference: f64::NAN,
        ..p
    }));
    assert!(
        Input::with_preferences(
            doc.tolerance(),
            Preferences {
                radius_difference: f64::INFINITY,
                ..p
            }
        )
        .is_err()
    );
    r.execute(&mut doc, &format!("{base} Ratio=0")).unwrap();
    let target = doc.objects().last().unwrap().geometry().clone();
    doc.undo().unwrap();
    r.execute(
        &mut doc,
        &format!("{base} RadiusDifference=5 Trim=Yes Join=No"),
    )
    .unwrap();
    assert_eq!(doc.objects().last().unwrap().geometry(), &target);
    doc.undo().unwrap();
    r.execute(&mut doc, &base).unwrap();
    let manual = doc.objects().last().unwrap().geometry().clone();
    doc.undo().unwrap();
    r.execute(&mut doc, &format!("{base} Ratio=1")).unwrap();
    assert_eq!(doc.objects().last().unwrap().geometry(), &manual);
    assert_ne!(manual, target);
    assert_eq!(other.arc_blend_preferences(), Preferences::default());
}
