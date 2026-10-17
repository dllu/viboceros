use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn setup() -> (CommandRegistry, Document, [ObjectId; 2], Vec<ObjectId>) {
    let mut d = Document::default();
    let a = d
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), d.tolerance()).unwrap(),
        ))
        .unwrap();
    let b = d
        .add_geometry(Geometry::Arc(
            viboceros_geometry::CircularArc3::try_from_circle_sweep(
                viboceros_geometry::Circle3::try_new(
                    p(0., 0., 0.),
                    10.,
                    viboceros_geometry::Vector3::try_new(0., 0., 1.)
                        .unwrap()
                        .normalized_nonzero()
                        .unwrap(),
                    d.tolerance(),
                )
                .unwrap(),
                std::f64::consts::FRAC_PI_2,
            )
            .unwrap(),
        ))
        .unwrap();
    let ids = [p(2., 1., 0.), p(7., 2., 1.)]
        .map(|p| d.add_geometry(Geometry::Point(p)).unwrap())
        .to_vec();
    d.add_group(Some("source group".into()), ids.iter().copied())
        .unwrap();
    d.set_object_names(ids.iter().map(|id| (*id, Some(format!("source-{id}")))))
        .unwrap();
    d.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    d.clear_history().unwrap();
    (CommandRegistry::with_builtins(), d, [a, b], ids)
}
#[test]
fn flow_target_surface_preserves_reference_excludes_selected_parent_and_releases_selection_on_replay()
 {
    let (r, mut d, [a, _], ids) = setup();
    let b = d
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(20., 0., 0.), p(30., 0., 0.), d.tolerance()).unwrap(),
        ))
        .unwrap();
    let surface = NurbsSurface::try_bilinear([
        p(18., 0., -4.),
        p(32., 0., -4.),
        p(32., 0., 4.),
        p(18., 0., 4.),
    ])
    .unwrap();
    let s = d
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let original = ids
        .iter()
        .map(|id| (*id, d.object(*id).unwrap().geometry().clone()))
        .collect::<Vec<_>>();
    d.select_objects_direct([s], SelectionMode::Add).unwrap();
    d.clear_history().unwrap();
    let options = Options {
        target_surface: Some(SurfaceReference {
            id: s,
            face: Some(0),
        }),
        copy: false,
        ..Default::default()
    };
    assert_eq!(
        super::options(
            &options.arguments().split_whitespace().collect::<Vec<_>>(),
            Options::default()
        )
        .unwrap()
        .0,
        options
    );
    r.execute(
        &mut d,
        &format!("Flow Base={a} Target={b} {}", options.arguments()),
    )
    .unwrap();
    for (id, expected) in ids.iter().zip([p(22., 0., 1.), p(27., -1., 2.)]) {
        let Geometry::Point(actual) = d.object(*id).unwrap().geometry() else {
            panic!()
        };
        assert!(actual.distance_to(expected).unwrap() < 1e-12);
    }
    assert_eq!(
        d.object(s).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert_eq!(d.selected_object_ids().count(), 0);
    assert!(r.flow_options().target_surface.is_none());
    assert_eq!(d.undo().unwrap().as_deref(), Some("Flow"));
    assert!(d.undo_label().is_none());
    for (id, geometry) in original {
        assert_eq!(d.object(id).unwrap().geometry(), &geometry);
    }
    assert_eq!(d.selected_object_ids().count(), 0);
    assert_eq!(d.redo().unwrap().as_deref(), Some("Flow"));
    assert_eq!(d.selected_object_ids().count(), 0);
}
#[test]
fn invalid_surface_references_and_duplicate_surface_forms_fail_atomically() {
    let (r, mut d, [a, b], _) = setup();
    let before = format!("{d:?}");
    for args in [
        format!("TargetSurface={a}"),
        format!("TargetSurfaceFace={a},999"),
        format!("TargetSurface={a} TargetSurfaceFace={a},0"),
        "TargetSurface=999999999".into(),
    ] {
        assert!(
            r.execute(&mut d, &format!("Flow Base={a} Target={b} {args}"))
                .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
        assert!(d.undo_label().is_none());
    }
}
#[test]
fn flow_copy_and_replace_retain_attributes_groups_and_one_history_entry() {
    for copy in [false, true] {
        let (r, mut d, [a, b], ids) = setup();
        let geometry = d
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        r.execute(
            &mut d,
            &format!(
                "Flow Base={a} Target={b} Copy={} Stretch=Yes",
                if copy { "Yes" } else { "No" }
            ),
        )
        .unwrap();
        assert_eq!(d.objects().count(), if copy { 6 } else { 4 });
        let after = d
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        for (id, g, attrs, groups) in &geometry {
            let o = d.object(*id).unwrap();
            assert_eq!(o.attributes(), attrs);
            assert_eq!(o.group_ids(), groups);
            if copy || [a, b].contains(id) {
                assert_eq!(o.geometry(), g);
            }
        }
        assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), ids);
        assert_eq!(d.undo().unwrap().as_deref(), Some("Flow"));
        assert_eq!(
            d.objects()
                .map(|o| (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec()
                ))
                .collect::<Vec<_>>(),
            geometry
        );
        assert!(d.undo().unwrap().is_none());
        assert_eq!(d.redo().unwrap().as_deref(), Some("Flow"));
        assert_eq!(
            d.objects()
                .map(|o| (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec()
                ))
                .collect::<Vec<_>>(),
            after
        );
    }
}
#[test]
fn flow_invalid_inputs_are_atomic_and_do_not_change_successful_preferences() {
    let (r, mut d, [a, b], ids) = setup();
    let before = format!("{d:?}");
    for args in [
        format!("Base={a}"),
        format!("Base={a} Target={}", ids[0]),
        format!("Base={a} Target={b} Copy=Maybe"),
        format!("Base={a} Base={b} Target={b}"),
        format!("Base={a} BaseEdge={a},0 Target={b}"),
        format!("Base={a} Target={b} Roadlike=Maybe"),
        format!("Base={a} Target={b} Roadlike=Yes"),
        format!("Base={a} Target={b} Roadlike=Yes BaseUp=Automatic TargetUp=0,0,1"),
        format!("Base={a} Target={b} Roadlike=Yes BaseUp=0,0,0"),
        format!("Base={a} Target={b} Roadlike=Yes BaseUp=1,0,0"),
        format!("Base={a} Target={b} Local=Yes"),
        format!("Base={a} Target={b} Local=Yes LocalRadii=1,1"),
        format!("Base={a} Target={b} Local=Yes LocalRadii=-1,3"),
        format!("Base={a} Target={b} Local=Yes LocalRadii=NaN,3"),
        format!("Base={a} Target={b} Local=Yes LocalRadii=1,inf"),
        format!("Base={a} Target={b} Local=Yes LocalRadii=1,3 LocalRadii=2,4"),
        format!("Base={a} Target={b} TargetSurface={a}"),
        format!("Base={a} TargetEdge={a},999"),
    ] {
        assert!(
            r.execute(&mut d, &format!("Flow {args}")).is_err(),
            "{args}"
        );
        assert_eq!(format!("{d:?}"), before, "{args}");
        assert_eq!(r.flow_options(), Options::default());
    }
    r.execute(
        &mut d,
        &format!(
            "Flow Base={a} Target={b} Copy=No Stretch=Yes RigidGroups=No PreserveStructure=Yes"
        ),
    )
    .unwrap();
    assert!(r.flow_options().stretch);
    assert!(!r.flow_options().rigid_groups);
    assert!(r.flow_options().preserve_structure);
    r.execute(&mut d, "Undo").unwrap();
    assert!(r.flow_options().stretch);
}
#[test]
fn flow_local_radii_round_trip_leave_exterior_points_fixed_and_are_not_remembered() {
    let (r, mut d, [a, b], ids) = setup();
    let original = ids
        .iter()
        .map(|id| d.object(*id).unwrap().geometry().clone())
        .collect::<Vec<_>>();
    let opts = Options {
        local: true,
        local_radii: Some(CurveFlowLocalRadii::try_new(0.5, 1.5).unwrap()),
        copy: false,
        stretch: true,
        ..Default::default()
    };
    assert_eq!(
        options(
            &opts.arguments().split_whitespace().collect::<Vec<_>>(),
            Options::default()
        )
        .unwrap()
        .0,
        opts
    );
    r.execute(
        &mut d,
        &format!("Flow Base={a} Target={b} {}", opts.arguments()),
    )
    .unwrap();
    assert_ne!(d.object(ids[0]).unwrap().geometry(), &original[0]);
    assert_eq!(d.object(ids[1]).unwrap().geometry(), &original[1]);
    assert!(r.flow_options().local);
    assert!(r.flow_options().local_radii.is_none());
    assert_eq!(d.undo().unwrap().as_deref(), Some("Flow"));
    assert!(d.undo().unwrap().is_none());
    for (id, g) in ids.iter().zip(original) {
        assert_eq!(d.object(*id).unwrap().geometry(), &g);
    }
}
#[test]
fn flow_local_outside_grips_do_not_copy_or_morph_a_simultaneously_selected_parent() {
    let (r, mut d, [a, b], _) = setup();
    d.clear_selection();
    let curve = NurbsCurve::try_new(
        3,
        vec![
            p(1., 100., 0.),
            p(3., 102., 1.),
            p(7., 101., 2.),
            p(9., 100., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let owner = d.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    d.select_objects_direct([owner], SelectionMode::Replace)
        .unwrap();
    d.enable_control_points([owner]).unwrap();
    d.select_control_points(
        [1, 2].map(|index| viboceros_document::ControlPointId {
            object: owner,
            index,
        }),
        SelectionMode::Add,
    )
    .unwrap();
    d.clear_history().unwrap();
    let before = format!("{d:?}");
    let message = r
        .execute(
            &mut d,
            &format!("Flow Base={a} Target={b} Local=Yes LocalRadii=3,1 Copy=Yes"),
        )
        .unwrap();
    assert!(message.contains("Flowed 0 object(s)"));
    assert_eq!(format!("{d:?}"), before);
    assert!(d.undo().unwrap().is_none());
}
#[test]
fn flow_roadlike_explicit_axes_round_trip_and_preserve_history() {
    let (r, mut d, [base, _], ids) = setup();
    let target = d
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(20., 0., 0.), p(30., 0., 0.), d.tolerance()).unwrap(),
        ))
        .unwrap();
    let up = Vector3::try_new(0., 1., 0.).unwrap();
    let input = format!(
        "Flow Base={base} Target={target} Copy=No Roadlike=Yes BaseUp=0,0,1 TargetUp=0,1,0"
    );
    let before = ids
        .iter()
        .map(|id| d.object(*id).unwrap().geometry().clone())
        .collect::<Vec<_>>();
    r.execute(&mut d, &input).unwrap();
    for (id, g) in ids.iter().zip(&before) {
        let Geometry::Point(q) = g else { panic!() };
        let expected = p(20. + q.x(), q.z(), -q.y());
        assert_eq!(
            d.object(*id).unwrap().geometry(),
            &Geometry::Point(expected)
        );
    }
    assert!(r.flow_options().roadlike);
    assert_eq!(r.flow_options().base_up, None);
    assert_eq!(r.flow_options().target_up, None);
    let opts = Options {
        roadlike: true,
        base_up: Some(up),
        target_up: Some(Vector3::try_new(0., 0., -1.).unwrap()),
        ..Options::default()
    };
    let (again, _) = options(
        &opts.arguments().split_whitespace().collect::<Vec<_>>(),
        Options::default(),
    )
    .unwrap();
    assert_eq!(again, opts);
    assert_eq!(d.undo().unwrap().as_deref(), Some("Flow"));
    for (id, g) in ids.iter().zip(before) {
        assert_eq!(d.object(*id).unwrap().geometry(), &g);
    }
}
#[test]
fn flow_rigid_groups_use_a_shared_pose_and_picked_sources_release_selection() {
    for grouped in [false, true] {
        let (r, mut d, [a, b], ids) = setup();
        let before = ids
            .iter()
            .map(|id| d.object(*id).unwrap().geometry().clone())
            .collect::<Vec<_>>();
        r.execute(&mut d,&format!("Flow Base={a} Target={b} Rigid=Yes RigidGroups={} Copy=No Stretch=Yes PickedSources={},{}",if grouped{"Yes"}else{"No"},ids[0],ids[1])).unwrap();
        assert_eq!(d.selected_object_count(), 0);
        let positions = ids
            .iter()
            .map(|id| match d.object(*id).unwrap().geometry() {
                Geometry::Point(p) => *p,
                _ => panic!(),
            })
            .collect::<Vec<_>>();
        let input = before
            .iter()
            .map(|g| match g {
                Geometry::Point(p) => *p,
                _ => panic!(),
            })
            .collect::<Vec<_>>();
        if grouped {
            assert!(
                (input[0].distance_to(input[1]).unwrap()
                    - positions[0].distance_to(positions[1]).unwrap())
                .abs()
                    < 1e-8
            );
        } else {
            assert!(
                (input[0].distance_to(input[1]).unwrap()
                    - positions[0].distance_to(positions[1]).unwrap())
                .abs()
                    > 0.1
            );
        }
        d.undo().unwrap();
        assert_eq!(d.selected_object_count(), 0);
        d.redo().unwrap();
        assert_eq!(d.selected_object_count(), 0);
    }
}

#[test]
fn flow_spatial_guides_and_guide_grips_fail_before_document_changes() {
    let (r, mut d, [a, b], _) = setup();
    let spatial = d
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                3,
                vec![p(0., 0., 0.), p(3., 2., 3.), p(7., -1., 1.), p(10., 0., 4.)],
                vec![0., 0., 0., 0., 1., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    d.clear_history().unwrap();
    let before = format!("{d:?}");
    assert!(matches!(
        r.execute(&mut d, &format!("Flow Base={a} Target={spatial} Copy=No")),
        Err(CommandError::FlowSpatialFramesNotSupported)
    ));
    assert_eq!(format!("{d:?}"), before);
    d.enable_control_points([a]).unwrap();
    d.select_control_points(
        [viboceros_document::ControlPointId {
            object: a,
            index: 0,
        }],
        SelectionMode::Replace,
    )
    .unwrap();
    let before = format!("{d:?}");
    assert!(matches!(
        r.execute(&mut d, &format!("Flow Base={a} Target={b} Copy=No")),
        Err(CommandError::Usage(_))
    ));
    assert_eq!(format!("{d:?}"), before);
}

#[test]
fn flow_drawn_lines_share_argument_validation_and_leave_no_guide_objects() {
    for words in [
        "Line w0,0,0 w10,0,0 Line w5,2,3 w5,22,3",
        "BaseLine=0,0,0;10,0,0 TargetLine=5,2,3;5,22,3",
        "Line 0 0 0 10 0 0 TargetLine=5,2,3;5,22,3",
    ] {
        let r = CommandRegistry::with_builtins();
        let mut d = Document::default();
        let id = d.add_geometry(Geometry::Point(p(2., 1., 0.))).unwrap();
        d.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        d.clear_history().unwrap();
        r.execute(&mut d, &format!("Flow {words} Copy=No Stretch=Yes"))
            .unwrap();
        let Geometry::Point(out) = d.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(
            out.distance_to(p(4., 6., 3.)).unwrap() < 1e-12,
            "{words}: {out:?}"
        );
        assert_eq!(d.objects().count(), 1);
        assert_eq!(d.undo_label(), Some("Flow"));
        d.undo().unwrap();
        assert_eq!(
            d.object(id).unwrap().geometry(),
            &Geometry::Point(p(2., 1., 0.))
        );
        d.redo().unwrap();
        assert_eq!(d.objects().count(), 1);
    }
}
#[test]
fn flow_line_failures_are_atomic_and_canonical_reference_duplicates_are_rejected() {
    let (r, mut d, [a, b], _) = setup();
    let before = format!("{d:?}");
    for args in [
        format!("BaseLine=0,0,0;0,0,0 Target={b}"),
        format!("Base={a} TargetLine=1,2,3;1,2,3"),
        format!("Line 0,0,0 0.000000000001,0,0 Target={b}"),
        format!("BaseLine=0,0,0;NaN,0,0 Target={b}"),
        format!("BaseLine=0,0,0;inf,0,0 Target={b}"),
        format!("BaseLine=0,0,0;1,0,0 Base={a} Target={b}"),
        format!("Base={a} Target={b} TargetLine=1,2,3;2,3,4"),
        "Line 0,0,0 10,0,0 Line 0,0,0 10,0,0 Line 0,0,0 10,0,0".into(),
        format!("Line 0,0,0 Target={b}"),
        format!("BaseLine=0,0,0 Target={b}"),
    ] {
        assert!(
            r.execute(&mut d, &format!("Flow {args}")).is_err(),
            "{args}"
        );
        assert_eq!(format!("{d:?}"), before, "{args}");
        assert_eq!(r.flow_options(), Options::default());
    }
}
