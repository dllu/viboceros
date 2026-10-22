use super::*;
mod legacy;

#[test]
fn signed_numeric_plane_extrusion_preserves_metadata_selection_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "Polyline 0,0,0 2,0,3 4,0,0")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Group Profile").unwrap();
    registry.execute(&mut doc, "SetObjectName Input").unwrap();
    registry.execute(&mut doc, "Layer New Output").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut doc, "ExtrudeCrv -5 BothSides=Yes DeleteInput=Yes")
        .unwrap();
    assert!(doc.object(id).is_none());
    let output = doc.objects().next().unwrap();
    assert_eq!(
        output.attributes(),
        &ObjectAttributes::on_layer(doc.current_layer_id())
    );
    let surface = output
        .geometry()
        .nurbs_surface_representation()
        .unwrap()
        .unwrap();
    let u = *surface.domain_u().start();
    assert!(
        surface
            .evaluate(u, 0.)
            .unwrap()
            .distance_to(Point3::try_new(0., 5., 0.).unwrap())
            .unwrap()
            < 1e-10
    );
    assert!(
        surface
            .evaluate(u, 10.)
            .unwrap()
            .distance_to(Point3::try_new(0., -5., 0.).unwrap())
            .unwrap()
            < 1e-10
    );
    let after = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.is_selected(id));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn nonplanar_numeric_input_uses_active_cplane_and_point_pairs_keep_their_vector() {
    let t = Tolerance::DEFAULT;
    let mut doc = Document::default();
    let curve = NurbsCurve::try_clamped_uniform(
        3,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 2., 1.).unwrap(),
            Point3::try_new(2., 1., 3.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
        ],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let context = CommandContext {
        construction_plane: Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            t,
        )
        .unwrap(),
    };
    let registry = CommandRegistry::with_builtins();
    for (command, end) in [
        ("ExtrudeCrv 5", [0., -5., 0.]),
        ("ExtrudeCrv 0,0,0 3,0,4", [3., 0., 4.]),
    ] {
        registry
            .execute_in_context(&mut doc, command, context)
            .unwrap();
        let surface = doc
            .objects()
            .find(|o| o.id() != id)
            .unwrap()
            .geometry()
            .nurbs_surface_representation()
            .unwrap()
            .unwrap();
        let point = surface
            .evaluate(*surface.domain_u().start(), *surface.domain_v().end())
            .unwrap();
        assert!(
            point
                .distance_to(Point3::try_new(end[0], end[1], end[2]).unwrap())
                .unwrap()
                < 1e-10
        );
        registry.execute(&mut doc, "Undo").unwrap();
    }
}

#[test]
fn degenerate_line_extrusions_retain_sources_groups_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 0,0,4").unwrap();
    let vertical = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Line 0,0,0 4,0,0").unwrap();
    let horizontal = doc.objects().find(|o| o.id() != vertical).unwrap().id();
    doc.select_objects_direct([vertical, horizontal], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Group Inputs").unwrap();
    let group = doc.group_by_name("Inputs").unwrap().id();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut doc, "ExtrudeCrv 0,0,0 0,0,5 DeleteInput=Yes")
        .unwrap();
    assert!(doc.object(vertical).is_some());
    assert!(doc.object(horizontal).is_none());
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![vertical]
    );
    assert!(doc.is_selected(vertical));
    let output = doc.objects().find(|o| o.id() != vertical).unwrap();
    assert!(!doc.is_selected(output.id()));
    let surface = output
        .geometry()
        .nurbs_surface_representation()
        .unwrap()
        .unwrap();
    assert_eq!(
        surface.evaluate(4., 5.).unwrap(),
        Point3::try_new(4., 0., 5.).unwrap()
    );
    let after = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.group(group).unwrap().members().len(), 2);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);

    let history = doc.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut doc, "ExtrudeCrv 5 DeleteInput=Yes"),
        Err(CommandError::NoExtrudableCurves)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    assert_eq!(doc.undo_label(), history.as_deref());
    assert!(doc.is_selected(vertical));
}
