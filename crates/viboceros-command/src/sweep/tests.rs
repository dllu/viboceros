use super::*;

#[test]
fn circular_rail_sweep_creates_a_seam_joined_brep_and_undoes_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Circle 0,0,0 3",
        "SelLast",
        "SetObjectName Rail",
        "Line 3,0,-1 3,0,1",
        "SelLast",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Sweep1 RailName=Rail Parameters=0")
        .unwrap();
    let output = document
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
        .unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        unreachable!()
    };
    assert_eq!(brep.faces().len(), 1);
    assert_eq!(
        brep.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        1
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

    registry
        .execute(
            &mut document,
            "Sweep1 RailName=Rail Parameters=0 RefitRail=Yes",
        )
        .unwrap();
    let output = document
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
        .unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        unreachable!()
    };
    assert_eq!(brep.faces().len(), 1);
    assert_eq!(
        brep.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        1
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn large_circular_rail_can_be_refitted_into_a_closed_brep() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Circle 0,0,0 100",
        "SelLast",
        "SetObjectName Rail",
        "Line 100,0,-1 100,0,1",
        "SelLast",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    registry
        .execute(
            &mut document,
            "Sweep1 RailName=Rail Parameters=0 RefitRail=Yes",
        )
        .unwrap();
    let output = document
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
        .unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        unreachable!()
    };
    assert_eq!(brep.faces().len(), 1);
    assert_eq!(
        brep.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        1
    );
}

#[test]
fn closed_and_kinked_profiles_produce_valid_shared_brep_topology() {
    let registry = CommandRegistry::with_builtins();
    for (profile, expected_faces) in [("Circle 0,0,0 1", 1), ("Polyline 0,0,0 1,0,0 1,1,0", 2)] {
        let mut document = Document::default();
        for command in [
            "Line 0,0,0 0,0,5",
            "SelLast",
            "SetObjectName Rail",
            profile,
            "SelLast",
            "Sweep1 RailName=Rail Parameters=0",
        ] {
            registry.execute(&mut document, command).unwrap();
        }
        let object = document
            .objects()
            .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
            .unwrap();
        let Geometry::Brep(brep) = object.geometry() else {
            unreachable!()
        };
        assert_eq!(brep.faces().len(), expected_faces);
        assert!(!document.is_selected(object.id()));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 2);
    }
}

#[test]
fn multiple_selected_sections_obey_refit_and_blend_options() {
    let registry = CommandRegistry::with_builtins();
    for (blend, width) in [("No", 1.84375), ("Yes", 1.75)] {
        for style in ["FrameStyle=Freeform", "FrameStyle=Roadlike Axis=1,0,0"] {
            let mut document = Document::default();
            for command in [
                "Line 0,0,0 0,0,5",
                "SelLast",
                "SetObjectName Rail",
                "Line 0,0,0 2,0,0",
                "SelLast",
                "Line 0,0,5 1,0,5",
                "SelLast DeselectOthersBeforeSelect=No",
            ] {
                registry.execute(&mut document, command).unwrap();
            }
            let before = document.objects().cloned().collect::<Vec<_>>();
            let selected = document
                .selected_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            registry
                .execute(
                    &mut document,
                    "Sweep1 RailName=Rail Parameters=0,5 RefitRail=No",
                )
                .unwrap();
            let output = document
                .objects()
                .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
                .unwrap();
            let Geometry::Brep(brep) = output.geometry() else {
                unreachable!()
            };
            let point = brep.faces()[0].surface().evaluate(1.25, 1.).unwrap();
            assert_eq!(brep.faces()[0].surface().degree_u(), 1);
            assert!((point.x() - 1.75).abs() < 1e-12);
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut document, &format!(
                "Sweep1 RailName=Rail Parameters=0,5 RefitRail=Yes GlobalShapeBlending={blend} {style}"
            )).unwrap();
            let object = document
                .objects()
                .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
                .unwrap();
            let Geometry::Brep(brep) = object.geometry() else {
                unreachable!()
            };
            let point = brep.faces()[0].surface().evaluate(1.25, 1.).unwrap();
            assert!((point.x() - width).abs() < 1e-12);
            assert!(point.y().abs() < 1e-12);
            assert!((point.z() - 1.25).abs() < 1e-12);
            assert_eq!(
                document
                    .selected_objects()
                    .map(|o| o.id())
                    .collect::<Vec<_>>(),
                selected
            );
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
}

#[test]
fn selected_profiles_and_named_rail_create_one_undoable_brep() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Line 0,0,0 0,0,5",
        "SelLast",
        "SetObjectName Rail",
        "Line 0,0,0 2,0,0",
        "SelLast",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let before = document.objects().cloned().collect::<Vec<_>>();
    let selected = document
        .selected_objects()
        .map(|o| o.id())
        .collect::<Vec<_>>();
    registry
        .execute(&mut document, "Sweep1 RailName=Rail Parameters=0")
        .unwrap();
    assert_eq!(document.objects().count(), 3);
    assert_eq!(
        document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        selected
    );
    let output = document
        .objects()
        .find(|o| !before.iter().any(|b| b.id() == o.id()))
        .unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("sweep must produce a BRep");
    };
    assert_eq!(brep.faces().len(), 1);
    assert_eq!(output.attributes().name(), None);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    for invalid in [
        "Sweep1 RailName=Rail",
        "Sweep1 RailName=Rail Parameters=0,5",
        "Sweep1 RailName=Rail Parameters=5",
        "Sweep1 RailName=Rail Parameters=0 Axis=0,0,1",
        "Sweep1 RailName=Rail Parameters=0 FrameStyle=Roadlike Axis=0,0,1",
        "Sweep1 RailName=Rail Parameters=0 RefitRail=Maybe",
        "Sweep1 RailName=Rail Parameters=0 Parameters=0",
        "Sweep1 RailName=Rail Parameters=0 RefitRail=Yes RefitRail=No",
        "Sweep1 RailName=Rail Parameters=0 Unknown=Yes",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}
