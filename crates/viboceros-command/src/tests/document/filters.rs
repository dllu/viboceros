//! document filters command regressions.
use super::*;

#[test]
fn attribute_selection_matches_names_layers_and_exact_case_sensitive_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let default = document.current_layer_id();
    let add_named_point = |document: &mut Document, x, attributes: ObjectAttributes| {
        document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(x, 0.0, 0.0).unwrap()),
                attributes,
            )
            .unwrap()
    };
    let unrelated = add_named_point(&mut document, 0.0, ObjectAttributes::on_layer(default));
    let upper = add_named_point(
        &mut document,
        1.0,
        ObjectAttributes::on_layer(default).with_name("BoltA"),
    );
    let lower = add_named_point(
        &mut document,
        2.0,
        ObjectAttributes::on_layer(default).with_name("bolta"),
    );
    let wildcard = add_named_point(
        &mut document,
        3.0,
        ObjectAttributes::on_layer(default).with_name("BoltLong"),
    );
    let peer = add_named_point(
        &mut document,
        4.0,
        ObjectAttributes::on_layer(default).with_name("Peer"),
    );
    let hidden_object = add_named_point(
        &mut document,
        5.0,
        ObjectAttributes::on_layer(default)
            .with_name("BoltA")
            .with_visibility(false),
    );
    let locked_object = add_named_point(
        &mut document,
        6.0,
        ObjectAttributes::on_layer(default)
            .with_name("BoltA")
            .with_locked(true),
    );

    let hidden_layer = document
        .add_layer("Hidden Parts", ColorRgb::new(10, 20, 30))
        .unwrap();
    let hidden_layer_match = add_named_point(
        &mut document,
        7.0,
        ObjectAttributes::on_layer(hidden_layer).with_name("BoltA"),
    );
    let hidden_on_hidden_layer = add_named_point(
        &mut document,
        8.0,
        ObjectAttributes::on_layer(hidden_layer)
            .with_name("BoltA")
            .with_visibility(false),
    );
    document.set_layer_visibility(hidden_layer, false).unwrap();

    let locked_layer = document
        .add_layer("Locked Parts", ColorRgb::new(40, 50, 60))
        .unwrap();
    let locked_layer_match = add_named_point(
        &mut document,
        9.0,
        ObjectAttributes::on_layer(locked_layer).with_name("BoltA"),
    );
    let locked_on_locked_layer = add_named_point(
        &mut document,
        10.0,
        ObjectAttributes::on_layer(locked_layer)
            .with_name("BoltA")
            .with_locked(true),
    );
    document.set_layer_locked(locked_layer, true).unwrap();
    document
        .add_group(Some("Team".to_owned()), [upper, peer, locked_object])
        .unwrap();
    document
        .add_group(Some("team".to_owned()), [lower])
        .unwrap();
    document
        .add_group(Some("Overlap".to_owned()), [upper, wildcard])
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    document
        .select_object(unrelated, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelName BOLT?").unwrap(),
        "Selected 3 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([unrelated, upper, lower])
    );
    assert!(!document.is_selected(wildcard));
    assert!(!document.is_selected(peer));
    assert!(!document.is_selected(hidden_object));
    assert!(!document.is_selected(locked_object));
    assert!(!document.is_selected(hidden_layer_match));
    assert!(!document.is_selected(locked_layer_match));

    registry.execute(&mut document, "SelName bolt*").unwrap();
    assert!(document.is_selected(wildcard));
    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelName \"\"").unwrap(),
        "Selected 1 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [unrelated]
    );

    assert_eq!(
        registry.execute(&mut document, "SelGroup Team").unwrap(),
        "Selected 3 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([unrelated, upper, peer])
    );
    assert!(!document.is_selected(wildcard));
    assert_eq!(
        registry.execute(&mut document, "SelGroup team").unwrap(),
        "Selected 4 object(s)"
    );
    assert!(document.is_selected(lower));
    assert_eq!(
        registry.execute(&mut document, "SelGroup TEAM").unwrap(),
        "Selected 4 object(s)"
    );

    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "SelLayer \"Hidden Parts\"")
            .unwrap(),
        "Selected 1 object(s)"
    );
    assert!(document.layer(hidden_layer).unwrap().is_visible());
    assert!(document.is_selected(hidden_layer_match));
    assert!(!document.is_selected(hidden_on_hidden_layer));
    assert!(!document.is_selected(peer));
    assert_eq!(
        registry.execute(&mut document, "SelLayer LOCKED*").unwrap(),
        "Selected 2 object(s)"
    );
    assert!(!document.layer(locked_layer).unwrap().is_locked());
    assert!(document.is_selected(locked_layer_match));
    assert!(!document.is_selected(locked_on_locked_layer));
    assert_eq!(document.undo_label(), history.as_deref());

    let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
    for command in ["SelName", "SelLayer", "SelGroup", "SelName \"unterminated"] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            selection
        );
    }
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn sel_color_adds_resolved_selectable_matches_and_skips_group_members() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let default = document.current_layer_id();
    let target = ColorRgb::new(200, 10, 20);
    let target_layer = document.add_layer("Target", target).unwrap();
    let add_point = |document: &mut Document, x, attributes: ObjectAttributes| {
        document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(x, 0.0, 0.0).unwrap()),
                attributes,
            )
            .unwrap()
    };

    let prior = add_point(&mut document, 0.0, ObjectAttributes::on_layer(default));
    let by_layer = add_point(&mut document, 1.0, ObjectAttributes::on_layer(target_layer));
    let by_object = add_point(
        &mut document,
        2.0,
        ObjectAttributes::on_layer(default).with_object_color(target),
    );
    let by_material = add_point(
        &mut document,
        3.0,
        ObjectAttributes::on_layer(target_layer)
            .with_object_color(ColorRgb::new(1, 2, 3))
            .with_color_source(ObjectColorSource::Material),
    );
    let by_parent = add_point(
        &mut document,
        4.0,
        ObjectAttributes::on_layer(target_layer)
            .with_object_color(ColorRgb::new(4, 5, 6))
            .with_color_source(ObjectColorSource::Parent),
    );
    let grouped_match = add_point(
        &mut document,
        5.0,
        ObjectAttributes::on_layer(default).with_object_color(target),
    );
    let grouped_peer = add_point(&mut document, 6.0, ObjectAttributes::on_layer(default));
    let hidden = add_point(
        &mut document,
        7.0,
        ObjectAttributes::on_layer(default)
            .with_object_color(target)
            .with_visibility(false),
    );
    let locked = add_point(
        &mut document,
        8.0,
        ObjectAttributes::on_layer(default)
            .with_object_color(target)
            .with_locked(true),
    );
    document
        .add_group(Some("Mixed".to_owned()), [grouped_match, grouped_peer])
        .unwrap();
    document
        .select_objects_direct([prior], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    assert_eq!(
        registry
            .execute(&mut document, "_SelColor 200,10,20")
            .unwrap(),
        "Selected 5 object(s) with display color 200,10,20"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([prior, by_layer, by_object, by_material, by_parent])
    );
    assert!(!document.is_selected(grouped_match));
    assert!(!document.is_selected(grouped_peer));
    assert!(!document.is_selected(hidden));
    assert!(!document.is_selected(locked));
    assert_eq!(document.undo_label(), history.as_deref());

    let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
    for command in [
        "SelColor",
        "SelColor 1,2",
        "SelColor 256,0,0",
        "SelColor ByLayer",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            selection
        );
    }
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn duplicate_selection_commands_include_or_retain_originals_without_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let unique = document
        .add_geometry(Geometry::Point(Point3::try_new(9.0, 0.0, 0.0).unwrap()))
        .unwrap();
    let original = document
        .add_geometry(Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()))
        .unwrap();
    let duplicate = document
        .add_geometry(Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()))
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    document
        .select_object(unique, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelDupAll").unwrap(),
        "Selected 3 object(s)"
    );
    assert!(document.is_selected(original));
    assert!(document.is_selected(duplicate));

    document.clear_selection();
    document
        .select_object(unique, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelDup").unwrap(),
        "Selected 2 object(s)"
    );
    assert!(!document.is_selected(original));
    assert!(document.is_selected(duplicate));
    assert_eq!(document.undo_label(), history.as_deref());

    let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
    assert!(
        registry
            .execute(&mut document, "SelDup unexpected")
            .is_err()
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        selection
    );
}

#[test]
fn type_selection_commands_add_only_visible_unlocked_matches_without_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry
        .execute(&mut document, "Layer New Hidden Markers")
        .unwrap();
    registry.execute(&mut document, "Point 9,9,9").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry
        .execute(&mut document, "Layer Hide Hidden Markers")
        .unwrap();
    registry
        .execute(&mut document, "Layer New Locked Markers")
        .unwrap();
    registry.execute(&mut document, "Point 8,8,8").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry
        .execute(&mut document, "Layer Lock Locked Markers")
        .unwrap();
    registry.execute(&mut document, "Line 0,0 4,0").unwrap();
    registry.execute(&mut document, "Circle 10,0 2").unwrap();
    registry
        .execute(&mut document, "Polyline 0,0 2,0 2,2")
        .unwrap();
    registry
        .execute(&mut document, "Rectangle 20,0 24,3")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 3 30,0 31,2 33,2 34,0")
        .unwrap();
    registry
        .execute(&mut document, "SrfPt 0,10 2,10 2,12 0,12")
        .unwrap();
    document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![
                    Point3::try_new(10.0, 10.0, 0.0).unwrap(),
                    Point3::try_new(12.0, 10.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 12.0, 0.0).unwrap(),
                ],
                vec![[0, 1, 2]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![
                    Point3::try_new(20.0, 10.0, 0.0).unwrap(),
                    Point3::try_new(22.0, 10.0, 0.0).unwrap(),
                    Point3::try_new(20.0, 12.0, 0.0).unwrap(),
                    Point3::try_new(20.0, 10.0, 2.0).unwrap(),
                ],
                vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    assert_eq!(
        registry.execute(&mut document, "SelPt").unwrap(),
        "Selected 1 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelOpenCrv").unwrap(),
        "Selected 4 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelClosedCrv").unwrap(),
        "Selected 6 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelCrv").unwrap(),
        "Selected 6 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelSrf").unwrap(),
        "Selected 7 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelMesh").unwrap(),
        "Selected 9 object(s)"
    );
    assert_eq!(document.undo_label(), history.as_deref());
    assert_eq!(document.objects().len(), 11);
    assert_eq!(
        document
            .objects()
            .filter(|object| document.is_selected(object.id()))
            .count(),
        9
    );

    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelOpenMesh").unwrap(),
        "Selected 1 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelClosedMesh").unwrap(),
        "Selected 2 object(s)"
    );
    assert!(
        document
            .selected_objects()
            .all(|object| matches!(object.geometry(), Geometry::Mesh(_)))
    );
    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelLine").unwrap(),
        "Selected 1 object(s)"
    );
    assert!(
        document
            .selected_objects()
            .all(|object| matches!(object.geometry(), Geometry::Line(_)))
    );
    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelPolyline").unwrap(),
        "Selected 2 object(s)"
    );
    assert!(
        document
            .selected_objects()
            .all(|object| matches!(object.geometry(), Geometry::Polyline(_)))
    );
    assert!(registry.execute(&mut document, "SelMesh extra").is_err());
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn curve_shape_selection_matches_rhino_nurbs_span_and_planarity_rules() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 3,0").unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 3 0,1 1,1 2,1 3,1")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 3 0,2 1,2 2,2 3,2 4,2")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 2 0,3 1,3 2,3")
        .unwrap();
    registry
        .execute(&mut document, "Polyline 0,4 1,4 2,4")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 3 0,5,0 1,5,0 2,6,0 3,5,1")
        .unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);

    assert_eq!(
        registry.execute(&mut document, "SelLine").unwrap(),
        "Selected 3 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([ids[0], ids[1], ids[3]])
    );

    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelPlanarCrv").unwrap(),
        "Selected 5 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([ids[0], ids[1], ids[2], ids[3], ids[4]])
    );
    assert!(matches!(
        registry.execute(&mut document, "SelPlanarCrv extra"),
        Err(CommandError::Usage("SelPlanarCrv"))
    ));
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn polyline_and_short_curve_selection_match_rhino_representation_and_boundary_rules() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 9,9").unwrap();
    registry.execute(&mut document, "Line 0,0 0.5,0").unwrap();
    registry.execute(&mut document, "Line 0,1 1,1").unwrap();
    registry.execute(&mut document, "Line 0,2 1.5,2").unwrap();
    registry
        .execute(&mut document, "Polyline 0,3 0.25,3 0.75,3")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 1 0,4 0.3,4 0.8,4")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 1 0,5 0.6,5")
        .unwrap();
    registry
        .execute(&mut document, "ControlPointCurve 3 0,6 0.2,6 0.5,6 0.7,6")
        .unwrap();
    registry.execute(&mut document, "Line 0,7 0.4,7").unwrap();
    registry.execute(&mut document, "Line 0,8 0.4,8").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document.set_objects_visibility([ids[8]], false).unwrap();
    document.set_objects_locked([ids[9]], true).unwrap();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    assert_eq!(
        registry.execute(&mut document, "SelShortCrv 1").unwrap(),
        "Selected 7 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([ids[0], ids[1], ids[2], ids[4], ids[5], ids[6], ids[7]])
    );
    assert_eq!(document.undo_label(), history.as_deref());

    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelPolyline").unwrap(),
        "Selected 2 object(s)"
    );
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([ids[4], ids[5]])
    );
    let selection = document.selected_object_ids().collect::<BTreeSet<_>>();
    assert!(matches!(
        registry.execute(&mut document, "SelShortCrv"),
        Err(CommandError::Usage("SelShortCrv maximum-length"))
    ));
    assert!(matches!(
        registry.execute(&mut document, "SelShortCrv 1 extra"),
        Err(CommandError::Usage("SelShortCrv maximum-length"))
    ));
    assert!(matches!(
        registry.execute(&mut document, "SelShortCrv 0"),
        Err(CommandError::InvalidMaximumCurveLength(value)) if value == "0"
    ));
    assert!(matches!(
        registry.execute(&mut document, "SelShortCrv -1"),
        Err(CommandError::InvalidMaximumCurveLength(value)) if value == "-1"
    ));
    assert!(registry.execute(&mut document, "SelShortCrv NaN").is_err());
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        selection
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
