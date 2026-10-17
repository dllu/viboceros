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
        format!("Base={a} Target={b} Roadlike=Yes"),
        format!("Base={a} Target={b} Local=Yes"),
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
