//! Native output policy, creators, and transactional history.
#[path = "support/use_extrusions.rs"]
mod support;
use viboceros_command::CommandRegistry;

#[test]
fn use_extrusions_creator_geometry_and_history_match_native() {
    for (family, count) in [
        ("use_extrusions_creators", 162),
        ("use_extrusions_creators_limits", 6),
        ("use_extrusions_creators_domains", 4),
    ] {
        replay(family, count);
    }
}
fn replay(family: &str, count: usize) {
    let native = support::read(family);
    assert_eq!(native["results"].as_array().unwrap().len(), count);
    for row in native["results"].as_array().unwrap() {
        let v = &row["value"];
        let label = row["id"].as_str().unwrap();
        eprintln!("checking {label}");
        let (mut doc, source, context) = support::setup(v);
        let commands = CommandRegistry::with_builtins();
        let original = doc.objects().cloned().collect::<Vec<_>>();
        let mode = label.split_once('_').unwrap().0;
        commands
            .execute(&mut doc, &format!("UseExtrusions {mode}"))
            .unwrap();
        assert_eq!(doc.undo_label(), None);
        if !cfg!(feature = "native-smlib") && v["input"]["command"] == "ExtrudeSrf" {
            assert!(
                commands
                    .execute_in_context(
                        &mut doc,
                        v["input"]["application"].as_str().unwrap(),
                        context
                    )
                    .is_err()
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
            assert_eq!(doc.undo_label(), None);
            continue;
        }
        commands
            .execute_in_context(
                &mut doc,
                v["input"]["application"].as_str().unwrap(),
                context,
            )
            .unwrap();
        support::check(&doc, source, v, label);
        if let Some(id) = source {
            assert_eq!(doc.object(id).unwrap(), &original[0], "{label} source");
        }
        let after = doc.objects().cloned().collect::<Vec<_>>();
        assert_eq!(doc.undo_label(), v["input"]["command"].as_str(), "{label}");
        commands.execute(&mut doc, "Undo").unwrap();
        assert_eq!(
            doc.objects().cloned().collect::<Vec<_>>(),
            original,
            "{label} undo"
        );
        assert_eq!(commands.use_extrusions(), mode == "Extrusion");
        commands.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.objects().cloned().collect::<Vec<_>>(),
            after,
            "{label} redo"
        );
    }
}
