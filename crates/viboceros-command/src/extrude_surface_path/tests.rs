use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn setup() -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let s = doc
        .add_geometry(Geometry::NurbsSurface(
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
        ))
        .unwrap();
    let path = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![p(10., 20., 30.), p(10., 21., 31.), p(10., 20., 32.)],
                vec![2., 2., 2., 5., 5., 5.],
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([s, path], SelectionMode::Replace)
        .unwrap();
    (doc, s, path)
}
#[test]
fn surface_path_options_are_strict_atomic_and_trim_from_prepared_start() {
    let mut options = SurfacePathOptions::default();
    options
        .update(&["Solid=No", "Reverse=Yes", "PathLength=1"])
        .unwrap();
    let before = options;
    assert!(options.update(&["Solid=Yes", "PathLength=0"]).is_err());
    assert_eq!(options, before);
    assert!(SurfacePathInput::parse(&["Path=1", "PathName=a"]).is_err());
    assert!(SurfacePathInput::parse(&["BothSides=No"]).is_err());
    let curve =
        NurbsCurve::try_new(1, vec![p(5., 6., 7.), p(5., 6., 11.)], vec![2., 2., 8., 8.]).unwrap();
    let partial = prepare_surface_path(&curve, options, Tolerance::default()).unwrap();
    assert!((partial.length(Tolerance::default()).unwrap() - 1.).abs() < 1e-8);
    assert!(
        partial
            .evaluate(*partial.domain().start())
            .unwrap()
            .distance_to(p(5., 6., 11.))
            .unwrap()
            < 1e-8
    );
    assert!(
        partial
            .evaluate(*partial.domain().end())
            .unwrap()
            .distance_to(p(5., 6., 10.))
            .unwrap()
            < 1e-8
    );
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_path_portable_decline_is_atomic() {
    let (mut doc, _, _) = setup();
    let before = format!("{doc:?}");
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut doc, "ExtrudeSrfAlongCrv DeleteInput=Yes"),
        Err(CommandError::SurfacePathExtrusionRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_curved_caps_delete_and_history_keep_the_path() {
    let (mut doc, s, path) = setup();
    let original = doc.object(s).unwrap().clone();
    let original_path = doc.object(path).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "ExtrudeSrfAlongCrv DeleteInput=Yes")
        .unwrap();
    assert!(doc.object(s).is_none());
    assert_eq!(doc.object(path), Some(&original_path));
    assert_eq!(doc.selected_object_count(), 1);
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.signed_volume(doc.tolerance()).unwrap().abs() - 12.).abs() < 1e-5);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(s), Some(&original));
    assert_eq!(doc.object(path), Some(&original_path));
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(s).is_none());
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_reverse_partial_explicit_refs_and_failure_preserve_sources() {
    let (mut doc, s, path) = setup();
    let registry = CommandRegistry::with_builtins();
    doc.clear_selection();
    registry
        .execute(
            &mut doc,
            &format!(
                "ExtrudeSrfAlongCrv Path={path} FaceRefs={s}:0 Reverse=Yes PathLength=1 Solid=No"
            ),
        )
        .unwrap();
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(!b.is_solid());
    assert!(b.bounds().min().z() < -0.5);
    assert!(b.bounds().max().z() < 1. + 1e-8);
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfAlongCrv Path={path} FaceRefs={s}:0 PathLength=100 DeleteInput=Yes"
                )
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_path_later_tangent_face_preserves_batch_selection_and_redo() {
    let (mut doc, s, path) = setup();
    let registry = CommandRegistry::with_builtins();
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
    doc.replace_object_geometries([(
        path,
        Geometry::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![p(10., 20., 30.), p(10., 20., 32.)],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ),
    )])
    .unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("ExtrudeSrfAlongCrv FaceRefs={s}:0,{other}:0 Path={path} DeleteInput=Yes")
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
