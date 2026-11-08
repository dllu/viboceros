use viboceros_command::{
    CommandRegistry,
    fillet::{self, Preferences},
};
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
fn ordered_command_and_prepared_geometry_match_native_identity_metadata_and_history() {
    let mut count = 0;
    for family in inputs::FAMILIES {
        let mut previous = Preferences::default();
        for row in inputs::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let options = inputs::final_options(value, previous);
            previous = Preferences {
                dynamic: false,
                ..options
            };
            if inputs::diagnostic(value) || !inputs::success(value) {
                continue;
            }
            let mut doc = inputs::setup(value);
            let state = inputs::State::capture(&doc);
            let picks = inputs::picks(&doc, value);
            inputs::check(&doc, &value["before"], false);
            let before = format!("{doc:?}");
            let prepared = fillet::prepare(&doc, picks, options).unwrap();
            assert_eq!(format!("{doc:?}"), before);
            let mut cached = doc.clone();
            prepared.commit(&mut cached).unwrap();
            inputs::check(&cached, &value["after"], false);
            let registry = CommandRegistry::with_builtins();
            let point =
                |p: viboceros_geometry::Point3| p.to_array().map(|v| v.to_string()).join(",");
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "Fillet Sources={},{} Pick1={} Pick2={} {}",
                        picks[0].0,
                        picks[1].0,
                        point(picks[0].1),
                        point(picks[1].1),
                        options.arguments()
                    ),
                )
                .unwrap();
            let mut result = state.verify(&mut doc, value);
            result["id"] = serde_json::json!(format!("{family}/{}", row["id"].as_str().unwrap()));
            println!("FILLET_GETTER_COMMAND {result}");
            count += 1;
        }
    }
    assert!(count >= 10);
}

#[test]
fn cached_candidates_reject_stale_sources_and_invalid_options_without_consuming_redo() {
    let family = native::read("fillet_getter_discovery");
    let value = &family["results"][0]["value"];
    let mut doc = inputs::setup(value);
    let picks = inputs::picks(&doc, value);
    let prepared = fillet::prepare(
        &doc,
        picks,
        inputs::final_options(value, Preferences::default()),
    )
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 90,91,92").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    assert!(
        fillet::prepare(
            &doc,
            picks,
            Preferences {
                radius: -1.,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        Preferences::default()
            .updated(&["Radius=NaN"], doc.units())
            .is_err()
    );
    assert!(
        Preferences::default()
            .updated(&["Join=Yes", "Join=No"], doc.units())
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    doc.clear_selection();
    registry
        .execute(&mut doc, &format!("SelID {}", picks[0].0))
        .unwrap();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    let before = format!("{doc:?}");
    assert!(!prepared.is_current(&doc));
    assert!(prepared.commit(&mut doc).is_err());
    assert_eq!(format!("{doc:?}"), before);
}
