//! transforms orient command regressions.
use super::*;

#[test]
fn orient_matches_rhino_no_1d_and_3d_copy_scaling() {
    let registry = CommandRegistry::with_builtins();
    for (mode, expected) in [
        (
            "No",
            [
                Point3::try_new(10.0, 0.0, 4.0).unwrap(),
                Point3::try_new(9.0, -1.0, 4.0).unwrap(),
                Point3::try_new(10.0, -1.0, 5.0).unwrap(),
            ],
        ),
        (
            "1D",
            [
                Point3::try_new(10.0, 2.0, 4.0).unwrap(),
                Point3::try_new(9.0, -1.0, 4.0).unwrap(),
                Point3::try_new(10.0, -1.0, 5.0).unwrap(),
            ],
        ),
        (
            "3D",
            [
                Point3::try_new(10.0, 2.0, 4.0).unwrap(),
                Point3::try_new(7.0, -1.0, 4.0).unwrap(),
                Point3::try_new(10.0, -1.0, 7.0).unwrap(),
            ],
        ),
    ] {
        let mut document = Document::default();
        let originals = add_orient_triad(&mut document);
        let message = registry
            .execute(
                &mut document,
                &format!("Orient 1,2,3 3,2,3 10,-1,4 10,5,4 Copy=Yes Scale={mode}"),
            )
            .unwrap();
        assert!(message.contains("creating 3 copy object(s)"));
        assert_eq!(document.objects().len(), 6);
        assert_eq!(document.groups().len(), 2);
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            originals.into_iter().collect()
        );
        for ((name, expected_end), original) in
            ["x", "y", "z"].into_iter().zip(expected).zip(originals)
        {
            let copy = orient_line(&document, name, &[original]);
            assert!(copy.start().is_near(
                Point3::try_new(10.0, -1.0, 4.0).unwrap(),
                document.tolerance()
            ));
            assert!(copy.end().is_near(expected_end, document.tolerance()));
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.groups().len(), 1);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 6);
        assert_eq!(document.groups().len(), 2);
    }
}

#[test]
fn orient_defaults_to_unscaled_in_place_shortest_rotation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let originals = add_orient_triad(&mut document);
    registry
        .execute(&mut document, "Orient 1,2,3 3,2,3 10,-1,4 10,5,4")
        .unwrap();
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.groups().len(), 1);
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        originals.into_iter().collect()
    );
    for (name, expected_end) in [
        ("x", Point3::try_new(10.0, 0.0, 4.0).unwrap()),
        ("y", Point3::try_new(9.0, -1.0, 4.0).unwrap()),
        ("z", Point3::try_new(10.0, -1.0, 5.0).unwrap()),
    ] {
        let line = orient_line(&document, name, &[]);
        assert!(line.end().is_near(expected_end, document.tolerance()));
    }
    assert_eq!(document.undo_label(), Some("Orient"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        orient_line(&document, "x", &[]),
        LineSegment::try_new(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Point3::try_new(2.0, 2.0, 3.0).unwrap(),
            document.tolerance(),
        )
        .unwrap()
    );
}

#[test]
fn orient_three_point_matches_rhino_frames_scale_and_atomic_errors() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let originals = add_orient_triad(&mut document);
    registry
        .execute(
            &mut document,
            "Orient3Pt 1,2,3 3,2,3 1,3,4 10,-1,4 10,5,4 8,-1,8 Copy=Yes Scale=Yes",
        )
        .unwrap();
    let expected = [
        ("x", Point3::try_new(10.0, 2.0, 4.0).unwrap()),
        (
            "y",
            Point3::try_new(7.153950105848459, -1.0, 4.948683298050513).unwrap(),
        ),
        (
            "z",
            Point3::try_new(10.948683298050513, -1.0, 6.846049894151541).unwrap(),
        ),
    ];
    for ((name, expected_end), original) in expected.into_iter().zip(originals) {
        let copy = orient_line(&document, name, &[original]);
        assert!(copy.end().is_near(expected_end, document.tolerance()));
    }
    assert_eq!(document.groups().len(), 2);
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        originals.into_iter().collect()
    );

    registry.execute(&mut document, "Undo").unwrap();
    let object_count = document.objects().len();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Orient 0,0,0 0,0,0 1,1,1 2,1,1",
        "Orient 0,0,0 1,0,0 1,1,1 1,1,1",
        "Orient3Pt 0,0,0 1,0,0 2,0,0 0,0,0 0,1,0 0,0,1",
        "Orient3Pt 0,0,0 1,0,0 0,1,0 0,0,0 0,1,0 0,2,0",
        "Orient 0,0,0 1,0,0 0,0,0 0,1,0 Copy=Yes Copy=No",
        "Orient3Pt 0,0,0 1,0,0 0,1,0 0,0,0 0,1,0 0,0,1 Scale=1D",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.groups().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());
}
