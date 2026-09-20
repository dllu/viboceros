use super::*;

fn rectangle() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_bilinear(
            [[0., 0., 2.], [4., 0., 2.], [4., 3., 2.], [0., 3., 2.]]
                .map(|p| Point3::try_from(p).unwrap()),
        )
        .unwrap(),
    )
}

#[test]
fn input_attributes_and_groups_do_not_reselect_retained_sources_or_peers() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let attrs = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Source")
        .with_object_color(ColorRgb::new(11, 22, 33));
    let source = document
        .add_geometry_with_attributes(rectangle(), attrs.clone())
        .unwrap();
    let peer = document
        .add_geometry(Geometry::Point(Point3::try_new(10., 0., 0.).unwrap()))
        .unwrap();
    let group = document
        .add_group(Some("Source group".into()), [source, peer])
        .unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "DupBorder OutputLayer=Input")
        .unwrap();
    let output = document.objects().last().unwrap();
    let id = output.id();
    assert_eq!(output.attributes(), &attrs);
    assert_eq!(output.group_ids(), [group]);
    assert_eq!(
        document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        [id]
    );
    assert_eq!(document.groups().len(), 1);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.object(source).unwrap().attributes(), &attrs);
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, peer])
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        [id]
    );
}

#[test]
fn late_unsupported_source_discards_already_staged_borders_and_preserves_redo() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    let source = document.add_geometry(rectangle()).unwrap();
    registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
    let line = document.objects().last().unwrap().id();
    registry.execute(&mut document, "Point 9,9,9").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([source, line], SelectionMode::Replace)
        .unwrap();
    let undo = document.undo_label().map(str::to_owned);
    let redo = document.redo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupBorder"),
        Err(CommandError::UnsupportedDuplicateBorderGeometry)
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 0);
    assert_eq!(
        document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        [source, line]
    );
    assert_eq!(document.undo_label(), undo.as_deref());
    assert_eq!(document.redo_label(), redo.as_deref());
}

#[test]
fn border_assembly_never_moves_endpoints_to_bridge_a_modelling_tolerance_gap() {
    let tolerance = Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap();
    let curves = [[0., 1.], [1.001, 2.]].map(|[a, b]| {
        LineSegment::try_new(
            Point3::try_new(a, 0., 0.).unwrap(),
            Point3::try_new(b, 0., 0.).unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    });
    assert!(assemble(curves.to_vec(), tolerance).is_err());
    assert_eq!(
        curves[1].evaluate(*curves[1].domain().start()).unwrap().x(),
        1.001
    );
}
