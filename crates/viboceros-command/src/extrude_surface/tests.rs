use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn patch(warped: bool) -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                p(0., 0., 0.),
                p(2., 0., 0.),
                p(0., 3., 0.),
                p(2., 3., if warped { 1. } else { 0. }),
            ],
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    )
}
#[test]
fn surface_extrusion_options_validate_atomically_and_face_targets_are_checked() {
    let mut options = SurfaceExtrusionOptions::default();
    options
        .update(&[
            "BothSides=Yes",
            "DeleteInput=Yes",
            "Solid=No",
            "Direction=0,0,2",
        ])
        .unwrap();
    assert_eq!(
        SurfaceExtrusionInput::parse(
            &options
                .command_options()
                .split_whitespace()
                .collect::<Vec<_>>()
        )
        .unwrap()
        .options,
        options
    );
    let before = options;
    assert!(options.update(&["Solid=Yes", "Direction=0,0,0"]).is_err());
    assert_eq!(options, before);
    assert!(SurfaceExtrusionInput::parse(&["Face=0", "Faces=1"]).is_err());
    let mut doc = Document::default();
    let id = doc.add_geometry(patch(false)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        surface_extrusion_targets(&doc, &SurfaceFaces::Indices(vec![0, 0])).unwrap(),
        vec![(id, 0)]
    );
    assert!(surface_extrusion_targets(&doc, &SurfaceFaces::Indices(vec![1])).is_err());
    doc.clear_selection();
    assert_eq!(
        surface_extrusion_targets(&doc, &SurfaceFaces::References(vec![(id, 0)])).unwrap(),
        vec![(id, 0)]
    );
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_extrusion_portable_decline_preserves_history_and_geometry() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let id = doc.add_geometry(patch(false)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "ExtrudeSrf 2"),
        Err(CommandError::SurfaceExtrusionRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_curved_caps_both_sides_delete_and_history() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let id = doc.add_geometry(patch(true)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let source = doc.object(id).unwrap().clone();
    registry
        .execute(&mut doc, "ExtrudeSrf 2 BothSides=Yes DeleteInput=Yes")
        .unwrap();
    assert!(doc.object(id).is_none());
    let result = doc.selected_objects().next().unwrap();
    let Geometry::Brep(brep) = result.geometry() else {
        panic!()
    };
    assert!(brep.is_solid());
    assert!((brep.signed_volume(doc.tolerance()).unwrap().abs() - 24.).abs() < 1e-6);
    assert!((brep.bounds().min().z() + 2.).abs() < 1e-8);
    assert!((brep.bounds().max().z() - 3.).abs() < 1e-8);
    assert_eq!(doc.undo_label(), Some("ExtrudeSrf"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&source));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(id).is_none());
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_selected_faces_remove_only_used_faces_and_replay_groups() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let brep = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 4.]],
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    let group = doc.add_group(None, [id]).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(id).unwrap().clone();
    registry
        .execute(&mut doc, "ExtrudeSrf 2 Faces=0,1,0 DeleteInput=Yes")
        .unwrap();
    assert_eq!(doc.objects().count(), 3);
    assert_eq!(doc.selected_object_count(), 2);
    let Geometry::Brep(remaining) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(remaining.faces().len(), 4);
    assert_eq!(doc.object(id).unwrap().attributes(), before.attributes());
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![id]
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 3);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_later_tangent_face_rolls_back_sources_picks_and_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    doc.add_geometry(patch(false)).unwrap();
    let rotated = Geometry::NurbsSurface(
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![p(0., 0., 0.), p(0., 2., 0.), p(0., 0., 3.), p(0., 2., 3.)],
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    );
    doc.add_geometry(rotated).unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(&mut doc, "ExtrudeSrf 2 Direction=0,0,1 DeleteInput=Yes")
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_explicit_face_refs_work_without_object_selection() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let id = doc.add_geometry(patch(false)).unwrap();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrf 0,0,0 1,0,2 Solid=No FaceRefs={id}:0,{id}:0"),
        )
        .unwrap();
    assert_eq!(doc.objects().count(), 2);
    assert_eq!(doc.selected_object_count(), 1);
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(!b.is_solid());
    assert_eq!(b.faces().len(), 4);
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_numeric_distance_uses_native_plane_normal_independently_of_cplane() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(0., 0., 0.), p(0., 2., 0.), p(0., 0., 3.), p(0., 2., 3.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let plane = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        doc.tolerance(),
    )
    .unwrap();
    registry
        .execute_in_context(
            &mut doc,
            "ExtrudeSrf 2",
            CommandContext {
                construction_plane: plane,
            },
        )
        .unwrap();
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.bounds().max().x() - 2.).abs() < 1e-8);
    assert!((b.signed_volume(doc.tolerance()).unwrap().abs() - 12.).abs() < 1e-6);
}
