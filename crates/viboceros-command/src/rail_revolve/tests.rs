use super::*;
fn sources(doc: &mut Document) -> [ObjectId; 2] {
    let r = CommandRegistry::with_builtins();
    r.execute(doc, "Line 2,0,2 1,0,5").unwrap();
    let a = doc.objects().last().unwrap().id();
    r.execute(doc, "Circle 0,0,2 2").unwrap();
    let b = doc.objects().last().unwrap().id();
    doc.set_object_names([(b, Some("Rail".into()))]).unwrap();
    [a, b]
}
#[test]
fn rail_revolve_options_roles_and_failures_keep_geometry_history_and_redo() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let ids = sources(&mut doc);
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.add_geometry(Geometry::Point(Point3::try_new(9., 9., 9.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for input in [
        "RailRevolve",
        "RailRevolve 0,0,0",
        "RailRevolve 0,0,0 0,0,0",
        "RailRevolve 2,0,0 2,0,1",
        "RailRevolve 0,0,0 0,0,1 ScaleHeight=Maybe",
        "RailRevolve 0,0,0 0,0,1 ScaleHeight=Yes ScaleHeight=No",
        "RailRevolve 0,0,0 0,0,1 RailName=missing",
        "RailRevolve 0,0,0 0,0,1 RailId=bad",
        "RailRevolve Bad=No",
    ] {
        assert!(r.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(format!("{doc:?}"), before);
        assert!(doc.can_redo());
    }
    let prompt = r.object_selection_prompt("RailRevolve").unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Curves);
    assert_eq!(
        RailRevolveOptions::from_prompt(&prompt),
        RailRevolveOptions::default()
    );
}
#[cfg(feature = "native-smlib")]
#[test]
fn rail_revolve_named_and_ordered_closed_rails_produce_one_undoable_surface() {
    for named in [false, true] {
        for split in [false, true] {
            let r = CommandRegistry::with_builtins();
            let mut doc = Document::default();
            let ids = sources(&mut doc);
            doc.select_objects_direct(
                if named { vec![ids[0]] } else { ids.to_vec() },
                SelectionMode::Replace,
            )
            .unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let selected = doc.selected_object_ids().collect::<Vec<_>>();
            r.execute(
                &mut doc,
                &format!(
                    "RailRevolve 0,0,0 0,0,1 ScaleHeight=Yes SplitAtTangents={} {}",
                    if split { "Yes" } else { "No" },
                    if named { "RailName=rail" } else { "" }
                ),
            )
            .unwrap();
            assert_eq!(doc.objects().count(), 3);
            assert_eq!(doc.undo_label(), Some("RailRevolve"));
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
            for object in &before {
                assert_eq!(doc.object(object.id()), Some(object));
            }
            let output = doc
                .objects()
                .find(|o| !ids.contains(&o.id()))
                .unwrap()
                .clone();
            assert_eq!(
                geometry_check::check_geometry(output.geometry()).status,
                geometry_check::GeometryCheckStatus::Valid
            );
            assert!(if split {
                matches!(output.geometry(), Geometry::Brep(_))
            } else {
                matches!(output.geometry(), Geometry::NurbsSurface(_))
            });
            r.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            r.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.object(output.id()), Some(&output));
        }
    }
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn rail_revolve_without_native_rejects_without_losing_sources() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let ids = sources(&mut doc);
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(
        r.execute(&mut doc, "RailRevolve 0,0,0 0,0,1")
            .unwrap_err()
            .to_string()
            .contains("native-smlib")
    );
    assert_eq!(format!("{doc:?}"), before);
}
