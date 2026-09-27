use super::*;

#[test]
fn qualified_isocurve_pick_uses_one_brep_face_and_selected_object() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let brep = Brep::try_box(
        frame,
        [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
        document.tolerance(),
    )
    .unwrap();
    let top_face = brep
        .faces()
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let height = |face: &BrepFace| {
                let u = face.surface().domain_u();
                let v = face.surface().domain_v();
                face.surface()
                    .evaluate((u.start() + u.end()) / 2.0, (v.start() + v.end()) / 2.0)
                    .unwrap()
                    .z()
            };
            height(a).total_cmp(&height(b))
        })
        .unwrap()
        .0;
    let box_id = document.add_geometry(Geometry::Brep(brep)).unwrap();
    let front_surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 10.0).unwrap(),
        Point3::try_new(2.0, 0.0, 10.0).unwrap(),
        Point3::try_new(2.0, 3.0, 10.0).unwrap(),
        Point3::try_new(0.0, 3.0, 10.0).unwrap(),
    ])
    .unwrap();
    let front_id = document
        .add_geometry(Geometry::NurbsSurface(front_surface))
        .unwrap();
    document
        .select_objects_direct([box_id, front_id], SelectionMode::Replace)
        .unwrap();
    let command =
        format!("ExtractIsocurve 0.5,0.5,10 Face={top_face} Object={box_id} Direction=Both");
    assert_eq!(
        registry.execute(&mut document, &command).unwrap(),
        "Extracted 2 exact U/V isocurve(s) from 1 surface(s)"
    );
    assert_eq!(document.objects().len(), 4);
    for object in document.selected_objects() {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("expected exact isocurve")
        };
        assert!((curve.evaluate(0.5).unwrap().z() - 4.0).abs() < 1e-10);
    }

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([box_id, front_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(
            &mut document,
            &format!("ExtractIsocurve 0.5,0.5,4 Face=99 Object={box_id}")
        ),
        Err(CommandError::ExtractIsocurveFaceIndexOutOfRange { .. })
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
    for command in [
        format!("ExtractIsocurve ExtractAll Face={top_face} Object={box_id}"),
        format!("ExtractIsocurve 0.5,0.5,4 Face={top_face}"),
        format!("ExtractIsocurve 0.5,0.5,4 Object={box_id}"),
    ] {
        assert!(matches!(
            registry.execute(&mut document, &command),
            Err(CommandError::Usage(_))
        ));
        assert_eq!(document.objects().len(), 2);
    }
    document
        .select_objects_direct([front_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, &command),
        Err(CommandError::Usage(_))
    ));
    assert_eq!(document.objects().len(), 2);
}

#[test]
fn extract_all_stations_are_not_rounded_to_the_native_uv_grid() {
    let registry = CommandRegistry::with_builtins();
    for offset in [[0., 0.], [1e12, -2e12], [-1e12, 2e12]] {
        for (as_brep, ignore_trims) in [(false, false), (true, false), (true, true)] {
            for (direction, axes) in [("U", vec![0]), ("V", vec![1]), ("Both", vec![0, 1])] {
                let mut document = Document::default();
                let surface = NurbsSurface::try_bilinear([
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(1., 0., 0.).unwrap(),
                    Point3::try_new(1., 1., 1.).unwrap(),
                    Point3::try_new(0., 1., 0.).unwrap(),
                ])
                .unwrap();
                let surface = surface
                    .try_reparameterized(offset[0]..=offset[0] + 1., offset[1]..=offset[1] + 1.)
                    .unwrap();
                let geometry = if as_brep {
                    Geometry::Brep(Brep::try_surface_face(surface, document.tolerance()).unwrap())
                } else {
                    Geometry::NurbsSurface(surface)
                };
                let attributes = ObjectAttributes::on_layer(document.current_layer_id())
                    .try_with_wire_density(3)
                    .unwrap();
                let source = document
                    .add_geometry_with_attributes(geometry.clone(), attributes)
                    .unwrap();
                document
                    .select_objects_direct([source], SelectionMode::Replace)
                    .unwrap();
                registry
                    .execute(
                        &mut document,
                        &format!(
                            "ExtractIsocurve ExtractAll Direction={direction} IgnoreTrims={}",
                            if ignore_trims { "Yes" } else { "No" }
                        ),
                    )
                    .unwrap();
                let curves = document
                    .selected_objects()
                    .map(|o| {
                        let Geometry::NurbsCurve(curve) = o.geometry() else {
                            panic!("expected exact curve")
                        };
                        curve.clone()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(curves.len(), axes.len() * 4);
                for (curves, axis) in curves.chunks_exact(4).zip(axes) {
                    for (curve, fixed) in curves.iter().zip([0., 1. / 3., 2. / 3., 1.]) {
                        let curve = curve.try_reparameterized(0.0..=1.0).unwrap();
                        for t in [0., 0.1, 0.5, 0.9, 1.] {
                            let mut xy = [fixed; 2];
                            xy[axis] = t;
                            let expected = Point3::try_new(xy[0], xy[1], xy[0] * xy[1]).unwrap();
                            let actual = curve.evaluate(t).unwrap();
                            assert!(
                                actual.distance_to(expected).unwrap() < 2e-12,
                                "offset={offset:?}, direction={direction}, {actual:?} != {expected:?}"
                            );
                        }
                    }
                }
                assert!(!document.is_selected(source));
                assert_eq!(document.object(source).unwrap().geometry(), &geometry);
                assert_eq!(document.undo_label(), Some("ExtractIsocurve"));
                registry.execute(&mut document, "Undo").unwrap();
                assert_eq!(document.objects().len(), 1);
                assert_eq!(document.object(source).unwrap().geometry(), &geometry);
                registry.execute(&mut document, "Redo").unwrap();
                assert_eq!(document.objects().len(), 1 + curves.len());
                assert_eq!(
                    document
                        .selected_objects()
                        .map(|o| o.geometry().clone())
                        .collect::<Vec<_>>(),
                    curves
                        .into_iter()
                        .map(Geometry::NurbsCurve)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
