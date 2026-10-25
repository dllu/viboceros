use super::*;

#[test]
fn options_round_trip_and_partial_updates_preserve_full_precision() {
    for style in ["Normal", "Loose", "Tight", "Straight", "Uniform"] {
        let input = format!("Type={style} Closed=No SplitAtTangents=Yes Natural");
        let options = Options::parse(&input.split_whitespace().collect::<Vec<_>>()).unwrap();
        assert_eq!(
            Options::parse(&options.arguments().split_whitespace().collect::<Vec<_>>()).unwrap(),
            options
        );
        if style != "Uniform" {
            let options = options
                .updated(&[
                    "StartPoint=-1.2345678901234567,2.3e-27,1e250",
                    "EndPoint=4,5,6",
                ])
                .unwrap();
            assert_eq!(
                Options::parse(&options.arguments().split_whitespace().collect::<Vec<_>>())
                    .unwrap(),
                options
            );
            assert!(options.updated(&["Closed=Yes"]).is_err());
            assert!(options.updated(&["Type=Normal", "_Type=Tight"]).is_err());
            let changed = options.updated(&["Type=Straight"]).unwrap();
            assert_eq!(changed.ends, options.ends);
            assert!(changed.natural_seams && changed.split_at_tangents);
        }
    }
}

fn sources(document: &mut Document) -> Vec<ObjectId> {
    let registry = CommandRegistry::with_builtins();
    for command in ["Line 0,0,0 1,0,0", "Line 0,0,3 2,0,3", "Line 0,0,5 1,0,5"] {
        registry.execute(document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    ids
}

#[test]
fn loft_uses_selection_order_and_fresh_attributes_with_undo_redo() {
    let registry = CommandRegistry::with_builtins();
    for closed in [false, true] {
        let mut document = Document::default();
        let ids = sources(&mut document);
        registry.execute(&mut document, "Group Profiles").unwrap();
        registry
            .execute(&mut document, "SetObjectName Input")
            .unwrap();
        registry.execute(&mut document, "Layer New Output").unwrap();
        registry
            .execute(&mut document, "Layer Current Output")
            .unwrap();
        document.clear_selection();
        for id in [ids[2], ids[0], ids[1]] {
            document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut document,
                if closed {
                    "Loft Type=Tight Closed=Yes"
                } else {
                    "Loft Type=Tight"
                },
            )
            .unwrap();
        let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("loft BRep");
        };
        let surface = brep.faces()[0].surface();
        assert_eq!(
            output.attributes(),
            &ObjectAttributes::on_layer(document.current_layer_id())
        );
        assert!(
            document
                .groups()
                .all(|g| !g.members().any(|id| id == output.id()))
        );
        let first = before
            .iter()
            .find(|o| o.id() == ids[2])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap();
        assert!(
            surface
                .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
                .unwrap()
                .distance_to(first)
                .unwrap()
                < 1e-12
        );
        assert_eq!(document.objects().count(), 4);
        assert_eq!(document.selected_object_count(), 3);
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![ids[2], ids[0], ids[1]]
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn invalid_lofts_are_atomic_and_do_not_add_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = sources(&mut document);
    for command in [
        "Loft Type=Wrong",
        "Loft Closed=Maybe",
        "Loft Type=Loose Type=Normal",
        "Loft DeleteInput=Yes Closed=Yes Closed=No",
        "Loft 1,2,3",
        "Loft Rebuild=10",
        "Loft SplitAtTangents=Maybe",
        "Loft SplitAtTangents=Yes SplitAtTangents=No",
        "Loft Natural Natural",
        "Loft Natural=Yes",
        "Loft StartPoint=0,0,-1 StartPoint=0,0,-2",
        "Loft EndPoint=0,0,4 EndPoint=0,0,5",
        "Loft StartPoint=NaN,0,0",
        "Loft EndPoint=1,2,3,4",
        "Loft StartPoint=0,0,-1 Closed=Yes",
        "Loft EndPoint=0,0,5 Type=Uniform",
    ] {
        let before = document.objects().cloned().collect::<Vec<_>>();
        let selection = document.selected_object_ids().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }
    document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    assert!(
        registry
            .execute(&mut document, "Loft DeleteInput=Yes")
            .is_err()
    );
    assert_eq!(document.objects().count(), 3);
    let p = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([ids[0], ids[1], p], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Loft"),
        Err(CommandError::LoftRequiresCurves)
    ));
    assert_eq!(document.objects().count(), 4);
}

#[test]
fn a_single_closed_profile_and_two_points_form_a_closed_mesh_with_atomic_history() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let circle = viboceros_geometry::Circle3::try_new(
        Point3::try_new(0., 0., 1.).unwrap(),
        1.,
        viboceros_geometry::UnitVector3::try_new(0., 0., 1., document.tolerance()).unwrap(),
        document.tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let id = document.add_geometry(Geometry::NurbsCurve(circle)).unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(
            &mut document,
            "Loft StartPoint=0,0,-1 EndPoint=0,0,3 Natural",
        )
        .unwrap();
    let output = document.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("point-ended Loft B-rep")
    };
    assert!(brep.is_solid());
    assert_eq!(
        (
            brep.faces().len(),
            brep.vertices().len(),
            brep.edges().len()
        ),
        (1, 2, 1)
    );
    for point in [
        Point3::try_new(0., 0., -1.).unwrap(),
        Point3::try_new(0., 0., 3.).unwrap(),
    ] {
        assert!(brep.vertices().iter().any(|v| v.point() == point));
    }
    let mesh = brep
        .polygon_mesh(0., false, false, document.tolerance())
        .unwrap();
    assert!(mesh.topology().is_manifold());
    assert!(mesh.topology().is_closed());
    assert!(document.is_selected(id));
    assert!(!document.is_selected(output.id()));
    let after = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

fn capsule_sources(document: &mut Document) -> Vec<ObjectId> {
    let points = [
        [-2., 1.],
        [-3., 1.],
        [-3., 0.],
        [-3., -1.],
        [-2., -1.],
        [0., -1.],
        [2., -1.],
        [3., -1.],
        [3., 0.],
        [3., 1.],
        [2., 1.],
        [0., 1.],
        [-2., 1.],
    ];
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let weights = [1., w, 1., w, 1., 1., 1., w, 1., w, 1., 1., 1.];
    let ids = [(1., 0.), (1.25, 2.), (0.75, 4.)]
        .into_iter()
        .map(|(scale, z)| {
            let controls = points
                .iter()
                .zip(weights)
                .map(|(p, w)| {
                    viboceros_geometry::WeightedPoint3::try_new(
                        Point3::try_new(p[0] * scale, p[1] * scale, z).unwrap(),
                        w,
                    )
                    .unwrap()
                })
                .collect();
            let curve = viboceros_geometry::NurbsCurve::try_new_rational(
                2,
                controls,
                vec![
                    0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 5., 5., 6., 6., 6.,
                ],
            )
            .unwrap();
            document.add_geometry(Geometry::NurbsCurve(curve)).unwrap()
        })
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    ids
}

#[test]
fn curvature_splitting_is_independent_of_the_global_crease_preference() {
    for crease in [false, true] {
        for split in [false, true] {
            let mut document = Document::default();
            let registry = CommandRegistry::with_builtins();
            let ids = capsule_sources(&mut document);
            let before = document.objects().cloned().collect::<Vec<_>>();
            registry
                .execute(
                    &mut document,
                    if crease {
                        "CreaseSplitting Enable"
                    } else {
                        "CreaseSplitting Disable"
                    },
                )
                .unwrap();
            registry
                .execute(
                    &mut document,
                    if split {
                        "Loft SplitAtTangents=Yes Natural"
                    } else {
                        "Loft SplitAtTangents=No Natural"
                    },
                )
                .unwrap();
            let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
            let Geometry::Brep(brep) = output.geometry() else {
                panic!("Loft B-rep")
            };
            assert_eq!(brep.faces().len(), if split { 4 } else { 1 });
            assert!(
                document
                    .objects()
                    .filter(|o| ids.contains(&o.id()))
                    .cloned()
                    .eq(before.iter().cloned())
            );
            let after = document.objects().cloned().collect::<Vec<_>>();
            registry
                .execute(&mut document, "CreaseSplitting Toggle")
                .unwrap();
            assert_eq!(document.undo_label(), Some("Loft"));
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(registry.crease_splitting(), !crease);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
            assert_eq!(registry.crease_splitting(), !crease);
        }
    }
}
