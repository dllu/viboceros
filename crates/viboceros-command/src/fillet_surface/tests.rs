use super::*;
fn sources() -> (CommandRegistry, Document, [ObjectId; 2]) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 10,0,0 10,10,0 0,10,0")
        .unwrap();
    registry
        .execute(&mut doc, "SrfPt 5,0,-5 5,0,5 5,10,5 5,10,-5")
        .unwrap();
    let ids = doc
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    doc.clear_history().unwrap();
    (registry, doc, ids)
}
#[test]
fn parsing_rejects_ambiguous_or_invalid_inputs() {
    for args in [
        "Radius=0",
        "Radius=-2",
        "Radius=nan",
        "Pick1=0,inf",
        "Pick1=1",
        "Trim=Maybe",
        "Join=Yes Trim=Split",
        "Join=Yes Trim=No",
        "Extend=Maybe",
        "Radius=1 Radius=2",
        "BlendType=G2",
        "wat=1",
    ] {
        assert!(
            parse(&args.split_whitespace().collect::<Vec<_>>()).is_err(),
            "{args}"
        );
    }
    let options = parse(&[
        "_Trim=_Split",
        "_Extend=_No",
        "Radius=0.25",
        "Pick1=0.2,0.5",
        "Pick2=0.8,0.5",
    ])
    .unwrap();
    assert_eq!(options.trim, Trim::Split);
    assert!(!options.extend);
    assert_eq!(options.radius, 0.25);
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn missing_native_kernel_is_explicit_and_does_not_change_document() {
    let (registry, mut doc, ids) = sources();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let error = registry
        .execute(
            &mut doc,
            &format!(
                "FilletSrf Sources={},{} Pick1=0.2,0.5 Pick2=0.2,0.5",
                ids[0], ids[1]
            ),
        )
        .unwrap_err();
    assert!(error.to_string().contains("native-smlib"));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
}
#[cfg(feature = "native-smlib")]
mod native {
    use super::*;
    fn command(ids: [ObjectId; 2], opts: &str) -> String {
        format!(
            "FilletSrf Sources={},{} Pick1=0.2,0.5 Pick2=0.2,0.5 Radius=1 {opts}",
            ids[0], ids[1]
        )
    }
    #[test]
    fn trim_split_join_and_no_trim_accept_with_one_undo_and_roundtrip() {
        for (opts, count, faces) in [
            ("Trim=No", 3, 1),
            ("Trim=Yes", 3, 1),
            ("Trim=Split", 5, 1),
            ("Trim=Yes Join=Yes", 1, 3),
        ] {
            let (registry, mut doc, ids) = sources();
            doc.set_object_names([(ids[0], Some("first parent".into()))])
                .unwrap();
            doc.set_object_user_text([ids[0]], "tag", Some("attribute"))
                .unwrap();
            doc.set_object_geometry_user_text([ids[0]], "tag", Some("geometry"))
                .unwrap();
            let group = doc.add_group(Some("parents".into()), ids).unwrap();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, &command(ids, opts)).unwrap();
            assert_eq!(doc.objects().len(), count, "{opts}");
            assert_eq!(doc.selected_object_count(), 1);
            let selected = doc.selected_objects().next().unwrap();
            let Geometry::Brep(b) = selected.geometry() else {
                panic!()
            };
            assert_eq!(b.faces().len(), faces);
            assert!(b.is_manifold());
            let owner = doc.object(ids[0]).unwrap();
            assert_eq!(owner.attributes().name(), Some("first parent"));
            assert!(owner.group_ids().contains(&group));
            assert_eq!(
                owner.geometry_user_text().get("tag").map(String::as_str),
                Some("geometry")
            );
            if opts == "Trim=No" {
                for old in &before {
                    assert_eq!(doc.object(old.id()).unwrap().geometry(), old.geometry());
                }
            }
            if opts == "Trim=Split" {
                assert_eq!(
                    doc.objects()
                        .filter(|o| o.attributes().name() == Some("first parent"))
                        .count(),
                    2
                );
                for o in doc
                    .objects()
                    .filter(|o| o.attributes().name() == Some("first parent"))
                {
                    assert!(o.group_ids().contains(&group));
                    assert_eq!(o.geometry_user_text(), owner.geometry_user_text());
                    assert_eq!(o.attributes(), owner.attributes());
                }
            }
            let after = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("fillet.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let model = viboceros_io::read_3dm_file_with_model_tolerance(&path).unwrap();
            assert_eq!(model.unsupported_object_count(), 0);
            assert_eq!(model.objects.len(), count);
        }
    }
    #[test]
    fn preview_is_readonly_and_stale_geometry_or_metadata_cannot_be_committed() {
        let (registry, mut doc, ids) = sources();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, &command(ids, "Preview=Yes"))
            .unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        let options = parse(&["Pick1=0.2,0.5", "Pick2=0.2,0.5"]).unwrap();
        let prepared = prepare(&doc, ids, &options).unwrap();
        for mode in 0..4 {
            let mut changed = doc.clone();
            match mode {
                0 => {
                    changed.delete_object(ids[0]).unwrap();
                }
                1 => {
                    changed
                        .set_object_names([(ids[0], Some("changed".into()))])
                        .unwrap();
                }
                2 => {
                    changed
                        .set_object_geometry_user_text([ids[1]], "new", Some("value"))
                        .unwrap();
                }
                _ => {
                    changed.add_group(None, [ids[0]]).unwrap();
                }
            }
            let snapshot = changed.objects().cloned().collect::<Vec<_>>();
            assert!(prepared.commit(&mut changed).is_err());
            assert_eq!(changed.objects().cloned().collect::<Vec<_>>(), snapshot);
        }
    }
    #[test]
    fn bad_sources_or_picks_leave_sources_and_history_unchanged() {
        let (registry, mut doc, ids) = sources();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        for args in [
            format!("Sources={},{} Pick1=0.5,0.5 Pick2=0.5,0.5", ids[0], ids[1]),
            format!("Sources={},{} Pick1=2,0.5 Pick2=0.2,0.5", ids[0], ids[1]),
            format!("Sources={},{} Pick1=0.2,0.5", ids[0], ids[1]),
            format!("Sources={},{} Pick1=0.2,0.5 Pick2=0.2,0.5", ids[0], ids[0]),
        ] {
            assert!(
                registry
                    .execute(&mut doc, &format!("FilletSrf {args}"))
                    .is_err()
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
        }
    }
}

#[cfg(feature = "native-smlib")]
#[path = "recorded.rs"]
mod recorded;
