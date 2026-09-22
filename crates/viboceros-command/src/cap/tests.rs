use super::*;
mod orientation;

fn source(document: &mut Document) -> ObjectId {
    let solid = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 5.]],
        document.tolerance(),
    )
    .unwrap();
    document
        .add_geometry_with_attributes(
            Geometry::Brep(
                solid
                    .sub_brep(&[0, 1, 2, 3, 4], document.tolerance())
                    .unwrap(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_name("Source")
                .with_object_color(ColorRgb::new(11, 22, 33)),
        )
        .unwrap()
}

#[test]
fn cap_is_atomic_in_place_with_attributes_groups_selection_and_history() {
    for preselect in [false, true] {
        let mut doc = Document::default();
        let id = source(&mut doc);
        let group = doc.add_group(Some("source".into()), [id]).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = doc.object(id).unwrap().clone();
        let registry = CommandRegistry::with_builtins();
        if preselect {
            registry.execute(&mut doc, "Cap").unwrap();
        } else {
            registry
                .execute_postselected(&mut doc, "Cap", Default::default())
                .unwrap();
        }
        let after = doc.object(id).unwrap().clone();
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(after.attributes(), before.attributes());
        assert_eq!(after.group_ids(), &[group]);
        let Geometry::Brep(brep) = after.geometry() else {
            panic!("expected brep")
        };
        assert!(brep.is_solid());
        assert_eq!(doc.is_selected(id), preselect);
        assert_eq!(doc.undo_label(), Some("Cap"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id), Some(&before));
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id), Some(&after));
    }
}

#[test]
fn unsupported_inputs_and_arguments_roll_back_every_object() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    assert!(matches!(
        registry.execute(&mut doc, "Cap"),
        Err(CommandError::NoObjectsSelected)
    ));
    let id = source(&mut doc);
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.select_objects_direct([id, point], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut doc, "Cap"),
        Err(CommandError::UnsupportedCapGeometry)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), history.as_deref());
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut doc, "Cap DeleteInput=Yes"),
        Err(CommandError::Usage("Cap"))
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn planar_sheet_noop_preserves_surface_representation_and_creates_no_history() {
    let mut doc = Document::default();
    let solid = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 1.]; 3],
        doc.tolerance(),
    )
    .unwrap();
    let surface = solid.faces()[0].surface().clone();
    let id = doc
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    let history = doc.undo_label().map(str::to_owned);
    registry
        .execute_postselected(&mut doc, "Cap", Default::default())
        .unwrap();
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert!(!doc.is_selected(id));
    assert_eq!(doc.undo_label(), history.as_deref());
    let prompt = CapCommand.object_selection_prompt(&[]).unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::SurfaceComponents);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::OptionsDuringSelection
    );
    assert!(prompt.filter.accepts_object(doc.object(id).unwrap()));
    assert!(
        CapCommand
            .object_selection_prompt(&["Triangles=Yes"])
            .is_err()
    );
}

#[test]
fn kink_boundary_subdivision_is_part_of_one_identity_preserving_history_step() {
    let mut doc = Document::default();
    let profile = NurbsCurve::try_clamped_uniform(
        1,
        [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.], [0., 0., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
    )
    .unwrap();
    let solid = Brep::try_extruded_curve(
        &profile,
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 2., 5.).unwrap(),
        doc.tolerance(),
    )
    .unwrap();
    let wall = solid.sub_brep(&[0], doc.tolerance()).unwrap();
    let id = doc
        .add_geometry_with_attributes(
            Geometry::Brep(wall),
            ObjectAttributes::on_layer(doc.current_layer_id()).with_name("kinked wall"),
        )
        .unwrap();
    let group = doc.add_group(Some("profile".into()), [id]).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(id).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Cap").unwrap();
    let after = doc.object(id).unwrap().clone();
    assert_eq!(after.attributes(), before.attributes());
    assert_eq!(after.group_ids(), &[group]);
    assert!(doc.is_selected(id));
    assert_eq!(doc.objects().len(), 1);
    let Geometry::Brep(brep) = after.geometry() else {
        panic!("expected brep")
    };
    assert!(brep.is_solid());
    assert_eq!(brep.faces().len(), 3);
    assert_eq!(brep.edges().len(), 7);
    assert_eq!(brep.vertices().len(), 6);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&after));
}
