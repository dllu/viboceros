use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn patch() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![p(0., 0., 0.), p(2., 0., 0.), p(0., 3., 0.), p(2., 3., 1.)],
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    )
}
#[test]
fn surface_apex_options_parse_strictly_and_update_atomically() {
    let mut options = SurfaceApexOptions::default();
    assert!(!options.split_at_tangents);
    options
        .update(&["Solid=No", "DeleteInput=Yes", "SplitAtTangents=Yes"])
        .unwrap();
    assert!(options.split_at_tangents);
    let before = options;
    assert!(options.update(&["Solid=Yes", "DeleteInput=bad"]).is_err());
    assert_eq!(options, before);
    assert!(SurfaceApexInput::parse(&["1,2,3", "Direction=0,0,1"]).is_err());
    assert!(SurfaceApexInput::parse(&["Solid=Yes", "Solid=No"]).is_err());
    assert!(SurfaceApexInput::parse(&["Face=0", "Faces=All"]).is_err());
    assert!(SurfaceApexInput::parse(&["SplitAtTangents=Yes", "SplitAtTangents=No"]).is_err());
    assert!(
        options
            .update(&["Solid=Yes", "SplitAtTangents=bad"])
            .is_err()
    );
    assert_eq!(options, before);
    assert_eq!(
        SurfaceApexInput::parse(&["1,2,3"]).unwrap().apex,
        Some(p(1., 2., 3.))
    );
    assert!(SurfaceApexInput::parse(&["1,2,3", "4,5,6"]).is_err());
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_apex_portable_decline_preserves_the_document() {
    let mut doc = Document::default();
    let s = doc.add_geometry(patch()).unwrap();
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, "ExtrudeSrfToPoint 1,1.5,6 DeleteInput=Yes"),
        Err(CommandError::SurfaceApexExtrusionRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_curved_cap_delete_history_and_uncapped_output() {
    let mut doc = Document::default();
    let s = doc.add_geometry(patch()).unwrap();
    doc.select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    let source = doc.object(s).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "ExtrudeSrfToPoint 1,1.5,6 DeleteInput=Yes")
        .unwrap();
    assert!(doc.object(s).is_none());
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.signed_volume(doc.tolerance()).unwrap().abs() - 11.5).abs() < 1e-5);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&source));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(s).is_none());
    registry.execute(&mut doc, "Undo").unwrap();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfToPoint 1,1.5,-6 Solid=No FaceRefs={s}:0"),
        )
        .unwrap();
    assert!(
        doc.selected_objects()
            .all(|o| matches!(o.geometry(),Geometry::Brep(b)if !b.is_solid()))
    );
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_face_delete_preserves_remainder_identity_attributes_and_groups() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 1.], [0., 1.], [0., 1.]],
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let group = doc.add_group(None, [s]).unwrap();
    let source = doc.object(s).unwrap().clone();
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfToPoint 2,2,2 FaceRefs={s}:0,{s}:0 DeleteInput=Yes"),
        )
        .unwrap();
    let remaining = doc.object(s).unwrap();
    assert_eq!(remaining.attributes(), source.attributes());
    let Geometry::Brep(b) = remaining.geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 5);
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![s]
    );
    assert_eq!(doc.selected_object_count(), 1);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&source));
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_later_coplanar_face_failure_preserves_batch_and_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let circle = |radius| {
        Circle3::try_new(
            p(0., 0., 0.),
            radius,
            UnitVector3::try_new(0., 0., 1., doc.tolerance()).unwrap(),
            doc.tolerance(),
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let annulus =
        Brep::try_planar_face_with_holes(&circle(3.), &[circle(1.)], doc.tolerance()).unwrap();
    let s = doc.add_geometry(Geometry::Brep(annulus)).unwrap();
    let other = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![p(0., 0., 0.), p(2., 0., 0.), p(0., 0., 3.), p(2., 0., 3.)],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfToPoint 1,0,6 FaceRefs={s}:0,{other}:0 Solid=No DeleteInput=Yes"
                )
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
