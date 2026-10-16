use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn sources(d: &mut Document) -> Vec<ObjectId> {
    [
        (p(0., 0., 0.), p(0., 0., 5.)),
        (p(2., 0., 5.), p(2., 0., 0.)),
        (p(2., 0., 5.), p(0., 0., 5.)),
        (p(0., 0., 0.), p(2., 0., 0.)),
    ]
    .into_iter()
    .map(|(a, b)| {
        d.add_geometry(Geometry::Line(
            LineSegment::try_new(a, b, d.tolerance()).unwrap(),
        ))
        .unwrap()
    })
    .collect()
}
#[test]
fn sweep_two_rail_arguments_and_selection_failures_preserve_document_and_redo() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let ids = sources(&mut d);
    d.select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    d.add_geometry(Geometry::Point(p(8., 8., 8.))).unwrap();
    d.undo().unwrap();
    let before = format!("{d:?}");
    for args in [
        "Sweep2 Bad=Yes",
        "Sweep2 Rail1Name=Rail",
        "Sweep2 Rail1Name=A Rail1Name=B Rail2Name=C",
        "Sweep2 Rail1Name= Rail2Name=B",
        "Sweep2 Rail1Name=Missing Rail2Name=Missing",
    ] {
        assert!(r.execute(&mut d, args).is_err(), "{args}");
        assert_eq!(format!("{d:?}"), before);
        assert!(d.can_redo());
    }
    let prompt = r.object_selection_prompt("Sweep2").unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Curves);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::OptionsDuringSelection
    );
    assert!(r.command_names().contains(&"Sweep2"));
}

#[cfg(feature = "native-smlib")]
#[test]
fn sweep_two_rail_ordered_and_named_inputs_create_one_undoable_brep() {
    for named in [false, true] {
        let r = CommandRegistry::with_builtins();
        let mut d = Document::default();
        let ids = sources(&mut d);
        d.set_object_names([
            (ids[0], Some("Left".into())),
            (ids[1], Some("Right".into())),
        ])
        .unwrap();
        d.select_objects_direct(
            if named {
                ids[2..].to_vec()
            } else {
                ids.clone()
            },
            SelectionMode::Replace,
        )
        .unwrap();
        let before = d.objects().cloned().collect::<Vec<_>>();
        let selection = d.selected_object_ids().collect::<Vec<_>>();
        let output = r
            .execute(
                &mut d,
                if named {
                    "Sweep2 Rail1Name=left Rail2Name=RIGHT"
                } else {
                    "Sweep2"
                },
            )
            .unwrap();
        assert!(output.contains("2 sections"));
        assert_eq!(d.objects().count(), 5);
        assert_eq!(d.undo_label(), Some("Sweep2"));
        for source in &before {
            assert_eq!(d.object(source.id()), Some(source));
        }
        assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), selection);
        let result = d
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap()
            .clone();
        let Geometry::Brep(b) = result.geometry() else {
            panic!("expected a B-rep")
        };
        assert_eq!(b.faces().len(), 1);
        assert!((b.area(d.tolerance()).unwrap() - 10.).abs() < 1e-8);
        assert_eq!(
            crate::geometry_check::check_geometry(result.geometry()).status,
            crate::geometry_check::GeometryCheckStatus::Valid
        );
        r.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        r.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.object(result.id()), Some(&result));
    }
}

#[cfg(not(feature = "native-smlib"))]
#[test]
fn sweep_two_rail_without_native_leaves_selected_inputs_and_history_unchanged() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let ids = sources(&mut d);
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "Sweep2")
            .unwrap_err()
            .to_string()
            .contains("native-smlib")
    );
    assert_eq!(format!("{d:?}"), before);
}
