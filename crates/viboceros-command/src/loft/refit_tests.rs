use super::*;

mod multispan;

#[test]
fn refit_options_round_trip_switch_modes_and_validate_atomically() {
    let rebuilt = Options::parse(&["Rebuild=6"]).unwrap();
    let refitted = rebuilt.updated(&["Refit=0.00012345678901234567"]).unwrap();
    assert_eq!(refitted.rebuild_point_count, 0);
    assert_eq!(
        Options::parse(&refitted.arguments().split_whitespace().collect::<Vec<_>>()).unwrap(),
        refitted
    );
    assert_eq!(
        refitted.updated(&["Rebuild=8"]).unwrap().refit_tolerance,
        None
    );
    assert_eq!(
        refitted
            .updated(&["Simplify=None"])
            .unwrap()
            .refit_tolerance,
        None
    );
    assert_eq!(refitted.updated(&["_Simplify=_Refit"]).unwrap(), refitted);
    for arguments in [
        vec!["Refit=-1"],
        vec!["Refit=NaN"],
        vec!["Refit=inf"],
        vec!["Refit=1", "RefitTolerance=2"],
        vec!["Refit=1", "Rebuild=4"],
        vec!["Rebuild=0", "Refit=0"],
        vec!["Simplify=Refit", "Rebuild=4"],
        vec!["Simplify=Rebuild", "Refit=0.01"],
    ] {
        assert!(refitted.updated(&arguments).is_err(), "{arguments:?}");
    }
}

#[test]
fn straight_refit_retains_originals_selection_and_single_undo_redo() {
    let registry = CommandRegistry::with_builtins();
    for ends in ["", "StartPoint=0,0,-2", "EndPoint=0,0,7"] {
        let mut document = Document::default();
        for command in ["Line 0,0,0 2,0,0", "Line 0,0,3 4,0,3"] {
            registry.execute(&mut document, command).unwrap();
        }
        let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        document.clear_history().unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut document,
                &format!("Loft Type=Normal Refit=0.001 {ends}"),
            )
            .unwrap();
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
        for source in &before {
            assert_eq!(document.object(source.id()).unwrap(), source);
        }
        let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("Loft BRep")
        };
        assert_eq!(brep.faces()[0].surface().degree_v(), 3);
        assert_eq!(brep.faces()[0].surface().control_point_count_v(), 4);
        assert_eq!(document.undo_label(), Some("Loft"));
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn unsupported_curved_refit_never_commits_or_changes_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in ["Line 0,0,0 2,0,0", "Circle 0,0,3 2"] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let error = registry
        .execute(&mut document, "Loft Refit=0.001")
        .unwrap_err();
    assert!(error.to_string().contains("Loft Refit currently requires"));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), None);
}

#[test]
fn polygon_refit_preserves_sources_and_supports_crease_splitting_and_undo() {
    for split in [false, true] {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "CreaseSplitting Disable")
            .unwrap();
        for z in [0., 3.] {
            let c = NurbsCurve::try_new(
                1,
                [[0., 0.], [0.5, 0.], [2., 0.], [2., 1.]]
                    .map(|p| Point3::try_new(p[0], p[1], z).unwrap())
                    .to_vec(),
                vec![0., 0., 1., 2., 3., 3.],
            )
            .unwrap();
            document.add_geometry(Geometry::NurbsCurve(c)).unwrap();
        }
        let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        document.clear_history().unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let option = if split { "Yes" } else { "No" };
        registry
            .execute(
                &mut document,
                &format!("Loft Type=Straight Refit=0.001 SplitAtTangents={option}"),
            )
            .unwrap();
        for original in &before {
            assert_eq!(document.object(original.id()).unwrap(), original);
        }
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
        let Geometry::Brep(brep) = document
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap()
            .geometry()
        else {
            panic!("Loft BRep")
        };
        assert_eq!(brep.faces().len(), if split { 2 } else { 1 });
        assert!(brep.faces().iter().all(|f| f.surface().degree_v() == 3));
        assert_eq!(document.undo_label(), Some("Loft"));
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn unclamped_rational_profiles_preserve_sources_and_create_one_undo_step() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = (0..3)
        .map(|i| {
            let z = f64::from(i) * 5.;
            let curve = NurbsCurve::try_new_rational(
                2,
                [
                    ([0., 0., z], 1.),
                    ([3., 2., z + 1.], 2.),
                    ([12., 8., z + 4.], 0.5),
                ]
                .into_iter()
                .map(|(point, weight)| {
                    viboceros_geometry::WeightedPoint3::try_new(
                        Point3::try_from(point).unwrap(),
                        weight,
                    )
                    .unwrap()
                })
                .collect(),
                vec![-1., -1., 0., 1., 2., 2.],
            )
            .unwrap();
            document.add_geometry(Geometry::NurbsCurve(curve)).unwrap()
        })
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Loft Refit=0.001").unwrap();
    for source in &before {
        assert_eq!(document.object(source.id()).unwrap(), source);
    }
    let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("Loft BRep")
    };
    assert_eq!(brep.faces()[0].surface().degree_v(), 3);
    assert_eq!(brep.faces()[0].surface().control_point_count_v(), 4);
    assert_eq!(document.undo_label(), Some("Loft"));
    let after = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn certified_smooth_refit_preserves_sources_and_has_atomic_undo_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for z in [0., 3.] {
        let curve = NurbsCurve::try_new(
            3,
            [[0., 0.], [1., 2.], [3., -1.], [4., 1.]]
                .map(|p| Point3::try_new(p[0], p[1], z).unwrap())
                .to_vec(),
            [vec![0.; 4], vec![1.; 4]].concat(),
        )
        .unwrap();
        document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Loft Type=Straight Refit=0.1")
        .unwrap();
    for source in &before {
        assert_eq!(document.object(source.id()).unwrap(), source);
    }
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
    let Geometry::Brep(brep) = document
        .objects()
        .find(|o| !ids.contains(&o.id()))
        .unwrap()
        .geometry()
    else {
        panic!("Loft BRep")
    };
    let surface = brep.faces()[0].surface();
    assert_eq!(surface.control_point_count_v(), 4);
    // Public native initial proposal, distinct from the unrefitted source.
    let control = surface.control_points()[surface.control_point_count_u()].point();
    assert!((control.x() - 0.968956044923331).abs() < 1e-12);
    assert!((control.y() - 1.937912089846662).abs() < 1e-12);
    assert_eq!(document.undo_label(), Some("Loft"));
    let after = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    // A tighter fit needs adaptive smooth fitting and cannot mutate the document.
    assert!(registry.execute(&mut document, "Loft Refit=0.001").is_err());
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn adjusted_smooth_refit_preserves_sources_selection_and_redo_on_failure() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for z in [0., 3.] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [
                ([2., 0., z], 1.),
                ([2., 2., z], 0.5_f64.sqrt()),
                ([0., 2., z], 1.),
            ]
            .map(|(p, w)| {
                viboceros_geometry::WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w)
                    .unwrap()
            })
            .to_vec(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Loft Type=Straight Refit=0.1")
        .unwrap();
    for source in &before {
        assert_eq!(document.object(source.id()).unwrap(), source);
    }
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
    let Geometry::Brep(brep) = document
        .objects()
        .find(|o| !ids.contains(&o.id()))
        .unwrap()
        .geometry()
    else {
        panic!("Loft BRep")
    };
    let surface = brep.faces()[0].surface();
    assert_eq!(surface.control_point_count_v(), 4);
    let point = surface.control_points()[surface.control_point_count_u()].point();
    assert!((point.x() - 2.).abs() < 1e-12);
    assert!((point.y() - 1.1045694997383313).abs() < 1e-9);
    let after = document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(document.undo_label(), Some("Loft"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(registry.execute(&mut document, "Loft Refit=0.001").is_err());
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}
