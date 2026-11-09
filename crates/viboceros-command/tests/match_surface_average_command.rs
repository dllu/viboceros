#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "support/merge_surface_command.rs"]
#[allow(dead_code)]
mod workflow;
use viboceros_command::{
    CommandRegistry,
    match_surface::{self, Edge},
};

#[test]
fn averaging_replaces_both_owned_surfaces_in_one_history_entry() {
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for row in captured::read("match_surface_average_command")["results"]
        .as_array()
        .unwrap()
    {
        let id = row["id"].as_str().unwrap();
        println!("AVERAGE_SURFACE_COMMAND_CASE {id}");
        let v = &row["value"];
        let mut doc = workflow::setup_named(v, "match", "Match");
        let original = doc.objects().cloned().collect::<Vec<_>>();
        let state = workflow::State::capture(&doc);
        let ids = original.iter().skip(1).map(|o| o.id()).collect::<Vec<_>>();
        let actions = v["prescribed_case"]["actions"].as_array().unwrap();
        let text = actions
            .iter()
            .filter_map(|a| a.as_str())
            .filter(|s| !matches!(*s, "Enter" | "Cancel"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut options = registry.match_surface_options();
        let mut words = text.split_whitespace();
        while let Some(word) = words.next() {
            options = if word.contains('=') {
                match_surface::parse(&[word], options)
            } else {
                match_surface::parse(&[word, words.next().unwrap()], options)
            }
            .unwrap();
        }
        registry.remember_match_surface_options(options);
        let picks = actions
            .iter()
            .filter(|a| a.get("curve").is_some())
            .collect::<Vec<_>>();
        let edges = picks
            .iter()
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
        let prepared = match_surface::prepare(&doc, edges.try_into().unwrap(), options).unwrap();
        assert_eq!(prepared.target_geometry().is_some(), options.average);
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        if !actions.iter().any(|a| a == "Cancel") {
            let mut script = "MatchSrf".to_owned();
            for a in picks {
                let coords = a["pick"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                script += &format!(
                    " {} {} {coords}",
                    ids[a["curve"].as_u64().unwrap() as usize],
                    a["edge"]
                );
            }
            script += &format!(" {}", options.arguments());
            registry.execute(&mut doc, &script).unwrap();
        }
        let mut record = state.verify_command(&mut doc, v, "MatchSrf");
        record["id"] = id.into();
        println!("AVERAGE_SURFACE_COMMAND {record}");
        count += 1;
    }
    assert_eq!(count, 8);
}
