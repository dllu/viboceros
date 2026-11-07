use viboceros_command::{CommandRegistry, chamfer};
#[path = "support/chamfer_getter.rs"]
#[allow(dead_code)]
mod chamfer_inputs;
#[path = "support/fillet_getter.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;

#[test]
fn ordered_sources_preserve_native_geometry_attributes_identity_history_and_archives() {
    let mut count = 0;
    for family in chamfer_inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            if !inputs::success(v) {
                continue;
            }
            let mut doc = inputs::setup(v);
            let state = inputs::State::capture(&doc);
            inputs::check(&doc, &v["before"], false);
            let picks = inputs::picks(&doc, v);
            let options = chamfer_inputs::final_options(v);
            let before = format!("{doc:?}");
            if chamfer_inputs::unsupported(id) {
                assert!(chamfer::prepare(&doc, picks, options).is_err());
                assert_eq!(before, format!("{doc:?}"));
                assert!(doc.can_redo());
                println!("CHAMFER_REJECTION {{\"id\":\"{family}/{id}\",\"redo_preserved\":true}}");
                continue;
            }
            let prepared = chamfer::prepare(&doc, picks, options).unwrap();
            assert_eq!(before, format!("{doc:?}"));
            let mut cached = doc.clone();
            prepared.commit(&mut cached).unwrap();
            inputs::check(&cached, &v["after"], false);
            let point =
                |p: viboceros_geometry::Point3| p.to_array().map(|v| v.to_string()).join(",");
            CommandRegistry::with_builtins()
                .execute(
                    &mut doc,
                    &format!(
                        "Chamfer Sources={},{} Pick1={} Pick2={} {}",
                        picks[0].0,
                        picks[1].0,
                        point(picks[0].1),
                        point(picks[1].1),
                        options.arguments()
                    ),
                )
                .unwrap();
            let mut result = state.verify(&mut doc, v);
            result["id"] = serde_json::json!(format!("{family}/{id}"));
            println!("CHAMFER_COMMAND {result}");
            count += 1;
        }
    }
    assert_eq!(count, 20);
}

#[test]
fn invalid_options_and_stale_preparations_are_atomic() {
    let rows = native::read("chamfer_getter_pilot");
    let v = &rows["results"][1]["value"];
    let mut doc = inputs::setup(v);
    let picks = inputs::picks(&doc, v);
    let options = chamfer_inputs::final_options(v);
    let prepared = chamfer::prepare(&doc, picks, options).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 90,91,92").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for text in [
        "Distances=-1,1",
        "Distances=NaN,1",
        "Join=Yes Join=No",
        "Sources=bad",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("Chamfer {text}"))
                .is_err()
        );
        assert_eq!(before, format!("{doc:?}"));
    }
    assert!(chamfer::prepare(&doc, [picks[0]; 2], options).is_err());
    registry
        .execute(&mut doc, &format!("SelID {}", picks[0].0))
        .unwrap();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    let before = format!("{doc:?}");
    assert!(!prepared.is_current(&doc));
    assert!(prepared.commit(&mut doc).is_err());
    assert_eq!(before, format!("{doc:?}"));
}
