#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "support/merge_surface_command.rs"]
#[allow(dead_code)]
mod workflow;
use viboceros_command::{
    CommandRegistry,
    match_surface::{self, Edge, Options},
};

#[test]
fn captured_match_command_geometry_metadata_order_identity_history_and_archives() {
    let mut count = 0;
    for family in ["pilot", "options", "qualified"] {
        for row in captured::read(&format!("match_surface_command_{family}"))["results"]
            .as_array()
            .unwrap()
        {
            let id = row["id"].as_str().unwrap();
            if workflow::outcome_command(&row["value"], "MatchSrf") != "Success"
                || [
                    "match_cmd_position",
                    "match_cmd_option_menus",
                    "match_cmd_tolerances_set",
                    "match_cmd_after_cancel",
                ]
                .contains(&id)
            {
                continue;
            }
            println!("MATCH_SURFACE_COMMAND_CASE {id}");
            let v = &row["value"];
            let mut doc = workflow::setup_named(v, "match", "Match");
            let state = workflow::State::capture(&doc);
            workflow::check(&doc, &v["before"], false);
            let original = doc.objects().cloned().collect::<Vec<_>>();
            let actions = v["prescribed_case"]["actions"].as_array().unwrap();
            let mut options = Options::default();
            for action in actions {
                if let Some(text) = action.as_str().filter(|text| *text != "Enter") {
                    options = match_surface::parse(&[text], options).unwrap();
                }
            }
            let ids = original.iter().skip(1).map(|o| o.id()).collect::<Vec<_>>();
            let edges = actions
                .iter()
                .filter(|a| a.get("curve").is_some())
                .map(|a| {
                    Edge::capture(
                        &doc,
                        ids[a["curve"].as_u64().unwrap() as usize],
                        a["edge"].as_u64().unwrap() as usize,
                        captured::point(&a["pick"]),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let prepared =
                match_surface::prepare(&doc, edges.try_into().unwrap(), options).unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
            assert!(!doc.can_undo());
            assert!(doc.can_redo());
            // Exercise the public script adapter as well as readonly staging.
            let registry = CommandRegistry::with_builtins();
            let mut script = "MatchSrf".to_owned();
            for a in actions.iter().filter(|a| a.get("curve").is_some()) {
                let point = a["pick"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                script += &format!(
                    " {} {} {point}",
                    ids[a["curve"].as_u64().unwrap() as usize],
                    a["edge"]
                );
            }
            script += &format!(" {}", options.arguments());
            registry.execute(&mut doc, &script).unwrap();
            let viboceros_document::Geometry::Brep(result) = doc.object(ids[0]).unwrap().geometry()
            else {
                panic!()
            };
            assert_eq!(result, prepared.geometry());
            let mut record = state.verify_command(&mut doc, v, "MatchSrf");
            record["id"] = id.into();
            println!("MATCH_SURFACE_COMMAND {record}");
            count += 1;
        }
    }
    assert_eq!(count, 16);
}

#[test]
fn stale_match_inputs_and_invalid_options_do_not_consume_pending_redo() {
    let row = &captured::read("match_surface_command_pilot")["results"][0];
    let mut doc = workflow::setup_named(&row["value"], "match", "Match");
    let ids = doc.objects().skip(1).map(|o| o.id()).collect::<Vec<_>>();
    let edges = [
        Edge::capture(
            &doc,
            ids[0],
            1,
            captured::point(&serde_json::json!([1., 0.25, 0.0975])),
        )
        .unwrap(),
        Edge::capture(
            &doc,
            ids[1],
            3,
            captured::point(&serde_json::json!([1.12, 0.25, 0.2375])),
        )
        .unwrap(),
    ];
    let prepared = match_surface::prepare(&doc, edges, Options::default()).unwrap();
    doc.set_tolerance(viboceros_geometry::Tolerance::DEFAULT);
    let history = (doc.can_undo(), doc.can_redo());
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(prepared.commit(&mut doc).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!((doc.can_undo(), doc.can_redo()), history);
    for invalid in [
        "AverageSurfaces=Maybe",
        "Mode=G3",
        "RefineMatch=Maybe",
        "Continuity=Position",
    ] {
        assert!(match_surface::parse(&[invalid], Options::default()).is_err());
    }
}
