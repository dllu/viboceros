//! surfaces direction command regressions.
use super::*;

#[test]
fn reverses_curve_direction_without_changing_identity_or_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 1,2 7,5").unwrap();
    registry.execute(&mut document, "Circle 10,0 3").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let Geometry::Line(original_line) = document.object(ids[0]).unwrap().geometry() else {
        panic!("expected source line")
    };
    let original_line = *original_line;
    let Geometry::Circle(original_circle) = document.object(ids[1]).unwrap().geometry() else {
        panic!("expected source circle")
    };
    let original_circle = *original_circle;
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Pair").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Rev").unwrap(),
        "Flipped 2 object(s)"
    );
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(
        document
            .group_by_name("Pair")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        ids.iter().copied().collect()
    );
    assert!(matches!(
        document.object(ids[0]).unwrap().geometry(),
        Geometry::Line(line) if line.start() == original_line.end() && line.end() == original_line.start()
    ));
    let Geometry::Circle(reversed_circle) = document.object(ids[1]).unwrap().geometry() else {
        panic!("expected reversed circle")
    };
    assert_eq!(
        reversed_circle.point_at_angle(0.0).unwrap(),
        original_circle.point_at_angle(0.0).unwrap()
    );
    assert!(
        document.tolerance().approx_eq(
            reversed_circle
                .normal()
                .unwrap()
                .as_vector()
                .dot(original_circle.normal().unwrap().as_vector())
                .unwrap(),
            -1.0
        )
    );
    assert_eq!(document.undo_label(), Some("Flip"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(ids[0]).unwrap().geometry(),
        &Geometry::Line(original_line)
    );
    assert_eq!(
        document.object(ids[1]).unwrap().geometry(),
        &Geometry::Circle(original_circle)
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert!(matches!(
        document.object(ids[0]).unwrap().geometry(),
        Geometry::Line(line) if *line == original_line.reversed()
    ));

    registry.execute(&mut document, "Point 99,99").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let point = document.objects().last().unwrap().clone();
    assert_eq!(
        registry.execute(&mut document, "Reverse").unwrap(),
        "Flipped 2 object(s)"
    );
    assert_eq!(document.object(point.id()), Some(&point));
    assert_eq!(document.selected_object_count(), 3);
    assert_eq!(
        document.object(ids[0]).unwrap().geometry(),
        &Geometry::Line(original_line)
    );
    assert_eq!(
        document.object(ids[1]).unwrap().geometry(),
        &Geometry::Circle(original_circle)
    );
}

#[test]
fn edits_surface_directions_exactly_without_changing_identity_attributes_or_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New DirectionTests")
        .unwrap();
    let source = rational_multi_span_surface();
    let expected_swapped = source.try_swapped_uv().unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Asymmetric rational surface");
    let id = document
        .add_geometry_with_attributes(Geometry::NurbsSurface(source.clone()), attributes.clone())
        .unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "Group DirectionSet")
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "_Direction _SwapUV")
            .unwrap(),
        "Edited 1 object direction(s) with SwapUV"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.attributes(), &attributes);
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("DirectionSet")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    let Geometry::Brep(brep) = object.geometry() else {
        panic!("surface face sense requires a B-rep")
    };
    assert!(brep.faces()[0].is_reversed());
    let swapped = brep.faces()[0].surface();
    assert_eq!(swapped, &expected_swapped);
    assert_eq!(
        (swapped.degree_u(), swapped.degree_v()),
        (source.degree_v(), source.degree_u())
    );
    assert_eq!(
        (
            swapped.control_point_count_u(),
            swapped.control_point_count_v()
        ),
        (
            source.control_point_count_v(),
            source.control_point_count_u()
        )
    );
    assert_eq!(swapped.knots_u(), source.knots_v());
    assert_eq!(swapped.knots_v(), source.knots_u());
    assert_eq!(document.undo_label(), Some("Dir"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(source.clone())
    );
    registry.execute(&mut document, "Redo").unwrap();
    let Geometry::Brep(redone) = document.object(id).unwrap().geometry() else {
        panic!("B-rep")
    };
    assert_eq!(redone.faces()[0].surface(), &expected_swapped);
    assert!(redone.faces()[0].is_reversed());
}

#[test]
fn edits_multiple_surface_axes_and_exchanges_periodicity() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let rational = rational_multi_span_surface();
    let periodic = periodic_cubic_u_surface();
    let rational_id = document
        .add_geometry(Geometry::NurbsSurface(rational.clone()))
        .unwrap();
    let periodic_id = document
        .add_geometry(Geometry::NurbsSurface(periodic.clone()))
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Dir Mode _FlipU").unwrap(),
        "Edited 2 object direction(s) with UReverse"
    );
    assert_eq!(
        direction_surface(document.object(rational_id).unwrap().geometry()),
        &rational.try_reversed_u().unwrap()
    );
    let reversed_periodic = direction_surface(document.object(periodic_id).unwrap().geometry());
    assert_eq!(reversed_periodic, &periodic.try_reversed_u().unwrap());
    assert!(reversed_periodic.is_periodic_u());
    assert!(!reversed_periodic.is_periodic_v());

    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    assert_eq!(
        registry.execute(&mut document, "Dir Mode=FlipV").unwrap(),
        "Edited 2 object direction(s) with VReverse"
    );
    assert_eq!(
        direction_surface(document.object(rational_id).unwrap().geometry()),
        &rational.try_reversed_v().unwrap()
    );
    assert_eq!(
        direction_surface(document.object(periodic_id).unwrap().geometry()),
        &periodic.try_reversed_v().unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    document.clear_selection();
    document
        .select_object(periodic_id, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Dir SwapUV").unwrap();
    let swapped_periodic = direction_surface(document.object(periodic_id).unwrap().geometry());
    assert_eq!(swapped_periodic, &periodic.try_swapped_uv().unwrap());
    assert!(!swapped_periodic.is_periodic_u());
    assert!(swapped_periodic.is_periodic_v());
}

#[test]
fn direction_flip_reuses_curve_reversal_and_rejects_invalid_edits_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut curve_document = Document::default();
    registry
        .execute(&mut curve_document, "Line 1,2,3 7,5,4")
        .unwrap();
    let line_id = curve_document.objects().next().unwrap().id();
    let Geometry::Line(source_line) = curve_document.object(line_id).unwrap().geometry() else {
        panic!("expected source line")
    };
    let source_line = *source_line;
    registry.execute(&mut curve_document, "SelAll").unwrap();
    assert_eq!(
        registry
            .execute(&mut curve_document, "Dir Mode=FlipNormal")
            .unwrap(),
        "Edited 1 object direction(s) with Flip"
    );
    assert_eq!(
        curve_document.object(line_id).unwrap().geometry(),
        &Geometry::Line(source_line.reversed())
    );

    let mut empty = Document::default();
    for command in [
        "Dir",
        "Dir Unknown",
        "Dir Flip extra",
        "Dir Mode",
        "Dir Mode=Flip Mode=Flip",
    ] {
        assert!(matches!(
            registry.execute(&mut empty, command),
            Err(CommandError::Usage(DIRECTION_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut empty, "Dir Flip"),
        Err(CommandError::NoObjectsSelected)
    ));

    let mut mixed = Document::default();
    let surface = rational_multi_span_surface();
    let surface_id = mixed
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let curve_id = mixed.add_geometry(Geometry::Line(source_line)).unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    let history = mixed.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut mixed, "Dir UReverse"),
        Err(CommandError::UnsupportedDirectionEditGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(mixed.undo_label(), history.as_deref());

    mixed.clear_selection();
    mixed
        .select_object(surface_id, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut mixed, "Dir Flip").unwrap();
    let Geometry::Brep(flipped) = mixed.object(surface_id).unwrap().geometry() else {
        panic!("surface face sense requires a B-rep")
    };
    assert_eq!(flipped.faces()[0].surface(), &surface);
    assert!(flipped.faces()[0].is_reversed());
    registry.execute(&mut mixed, "Undo").unwrap();
    assert_eq!(
        mixed.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert!(mixed.object(curve_id).is_some());
    assert_eq!(mixed.undo_label(), history.as_deref());
}

fn direction_surface(geometry: &Geometry) -> &NurbsSurface {
    let Geometry::Brep(brep) = geometry else {
        panic!("direction-edited face")
    };
    assert_eq!(brep.faces().len(), 1);
    assert!(brep.faces()[0].is_reversed());
    brep.faces()[0].surface()
}

fn box_brep() -> Brep {
    Brep::try_box(
        CommandContext::default().construction_plane,
        [[-2., 3.], [-1., 4.], [0., 5.]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn brep_chart_edits_retain_shared_edges_identity_metadata_and_atomic_history() {
    for command in [
        "Dir UReverse Faces=All",
        "Dir VReverse Faces=All",
        "Dir SwapUV Faces=All",
    ] {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let source = box_brep();
        let id = doc
            .add_geometry_with_attributes(
                Geometry::Brep(source.clone()),
                ObjectAttributes::on_layer(doc.current_layer_id()).with_name("solid"),
            )
            .unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut doc, "Group Solid").unwrap();
        doc.clear_history().unwrap();
        let before = doc.object(id).unwrap().clone();
        registry.execute(&mut doc, command).unwrap();
        let after = doc.object(id).unwrap().clone();
        assert_eq!(before.attributes(), after.attributes());
        assert!(
            doc.group_by_name("Solid")
                .unwrap()
                .members()
                .any(|member| member == id)
        );
        assert!(doc.is_selected(id));
        let Geometry::Brep(result) = after.geometry() else {
            panic!("B-rep")
        };
        Brep::try_new(
            result.vertices().to_vec(),
            result.edges().to_vec(),
            result.faces().to_vec(),
            doc.tolerance(),
        )
        .unwrap();
        assert!(result.is_solid());
        assert_eq!(result.vertices(), source.vertices());
        assert_eq!(result.edges(), source.edges());
        assert_ne!(result.faces(), source.faces());
        assert_eq!(doc.undo_label(), Some("Dir"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id), Some(&before));
        assert_eq!(doc.undo_label(), None);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id), Some(&after));
    }
}

#[test]
fn normal_flip_skips_solids_and_all_skipped_selection_preserves_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let solid = box_brep();
    let open = solid.duplicate_faces(&[0], doc.tolerance()).unwrap();
    let solid_id = doc.add_geometry(Geometry::Brep(solid.clone())).unwrap();
    let open_id = doc.add_geometry(Geometry::Brep(open)).unwrap();
    doc.select_objects_direct([solid_id, open_id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.object(open_id).unwrap().clone();
    registry.execute(&mut doc, "Dir Flip").unwrap();
    assert_eq!(
        doc.object(solid_id).unwrap().geometry(),
        &Geometry::Brep(solid)
    );
    let Geometry::Brep(a) = before.geometry() else {
        panic!("B-rep")
    };
    let Geometry::Brep(b) = doc.object(open_id).unwrap().geometry() else {
        panic!("B-rep")
    };
    assert_eq!(a.reversed(), *b);
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([solid_id], SelectionMode::Replace)
        .unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    registry.execute(&mut doc, "Dir FlipAll").unwrap();
    assert_eq!(doc.undo_label(), None);
    assert_eq!(doc.redo_label(), redo.as_deref());
}

#[test]
fn invalid_face_batches_leave_geometry_selection_groups_and_redo_untouched() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let id = doc.add_geometry(Geometry::Brep(box_brep())).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Group Source").unwrap();
    registry.execute(&mut doc, "Dir SwapUV Faces=0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    for command in [
        "Dir UReverse",
        "Dir UReverse Faces=6",
        "Dir VReverse Faces=0,6",
        "Dir SwapUV Faces=1,1",
        "Dir Flip Faces=0",
        "Dir UReverse Faces=All Faces=0",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before, "{command}");
    }
}
