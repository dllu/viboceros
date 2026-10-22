//! extraction face border command regressions.
use super::*;

#[test]
fn duplicate_face_border_preserves_face_order_and_rhino_output_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let box_brep = Brep::try_box(
        frame,
        [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry_with_attributes(
            Geometry::Brep(box_brep),
            ObjectAttributes::on_layer(input_layer)
                .with_name("Colored shell")
                .with_object_color(ColorRgb::new(17, 83, 149)),
        )
        .unwrap();
    let source_group = document
        .add_group(Some("Source group".to_owned()), [source])
        .unwrap();
    registry.execute(&mut document, "Layer New Output").unwrap();
    let output_layer = document.current_layer_id();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DupFaceBorder Faces=2,0 OutputLayer=Current",)
            .unwrap(),
        "Duplicated 2 curve object(s) in 2 face border(s) from 1 object(s)"
    );
    assert_eq!(document.objects().len(), 3);
    assert!(!document.is_selected(source));
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert!(outputs.iter().all(|output| {
        document.is_selected(output.id())
            && output.attributes().layer_id() == output_layer
            && output.attributes().name().is_none()
            && output.attributes().color_source() == ObjectColorSource::Layer
            && document
                .group(source_group)
                .unwrap()
                .members()
                .all(|member| member != output.id())
    }));
    let boundaries = outputs
        .iter()
        .map(|output| match output.geometry() {
            Geometry::Polyline(polyline) => polyline.vertices(),
            _ => panic!("linear face borders should join into one polyline"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        boundaries[0],
        &[
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 4.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        ]
    );
    assert_eq!(
        boundaries[1],
        &[
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        ]
    );
    assert_eq!(document.undo_label(), Some("DupFaceBorder"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
    assert!(!document.is_selected(source));
}

#[test]
fn duplicate_face_border_point_mode_joins_a_natural_surface_boundary() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(20.0, 0.0, 0.0).unwrap(),
        Point3::try_new(24.0, 0.0, 0.0).unwrap(),
        Point3::try_new(24.0, 3.0, 0.0).unwrap(),
        Point3::try_new(20.0, 3.0, 0.0).unwrap(),
    ])
    .unwrap();
    let source = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface),
            ObjectAttributes::on_layer(input_layer).with_name("Input surface"),
        )
        .unwrap();
    registry.execute(&mut document, "Layer New Output").unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "DuplicateFaceBorder 22,1,7 OutputLayer=Input",
            )
            .unwrap(),
        "Duplicated 1 curve object(s) in 1 face border(s) from 1 object(s)"
    );
    let output = document.objects().last().unwrap();
    assert_eq!(output.attributes().layer_id(), input_layer);
    assert!(output.attributes().name().is_none());
    let Geometry::Polyline(polyline) = output.geometry() else {
        panic!("a bilinear surface border should join into one polyline")
    };
    assert_eq!(
        polyline.vertices(),
        &[
            Point3::try_new(20.0, 3.0, 0.0).unwrap(),
            Point3::try_new(20.0, 0.0, 0.0).unwrap(),
            Point3::try_new(24.0, 0.0, 0.0).unwrap(),
            Point3::try_new(24.0, 3.0, 0.0).unwrap(),
            Point3::try_new(20.0, 3.0, 0.0).unwrap(),
        ]
    );

    let mut curved_document = Document::default();
    let curved_surface = rational_multi_span_surface();
    let mut expected = curved_surface.natural_boundary_curve_loops().unwrap();
    expected.reverse();
    let curved_source = curved_document
        .add_geometry(Geometry::NurbsSurface(curved_surface))
        .unwrap();
    curved_document
        .select_objects_direct([curved_source], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut curved_document, "DupFaceBorder Faces=0")
            .unwrap(),
        "Duplicated 1 curve object(s) in 1 face border(s) from 1 object(s)"
    );
    assert_eq!(curved_document.groups().len(), 0);
    assert_eq!(curved_document.objects().len(), 2);
    let Geometry::PolyCurve(curve) = curved_document.objects().last().unwrap().geometry() else {
        panic!("curved border must be one exact polycurve")
    };
    assert_eq!(curve.segments().len(), 4);
    for (segment, expected) in curve.segments().iter().zip(expected.remove(0)) {
        let CurveRef::NurbsCurve(actual) = segment.as_ref() else {
            panic!("exact NURBS child")
        };
        assert_eq!(actual.control_points(), expected.control_points());
        for i in 0..=32 {
            let fraction = i as f64 / 32.0;
            assert!(
                actual
                    .evaluate(actual.parameter_at(fraction).unwrap())
                    .unwrap()
                    .distance_to(
                        expected
                            .evaluate(expected.parameter_at(fraction).unwrap())
                            .unwrap()
                    )
                    .unwrap()
                    < 1.0e-12
            );
        }
    }
}

#[test]
fn duplicate_face_border_orders_holes_and_omits_wall_seams() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let normal = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance()).unwrap();
    let outer = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(8.0, 0.0, 0.0).unwrap(),
            Point3::try_new(8.0, 6.0, 0.0).unwrap(),
            Point3::try_new(0.0, 6.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        ],
        document.tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let holes = [
        Circle3::try_new(
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            1.0,
            normal,
            document.tolerance(),
        )
        .unwrap()
        .to_nurbs()
        .unwrap(),
        Circle3::try_new(
            Point3::try_new(6.0, 3.0, 0.0).unwrap(),
            0.5,
            normal,
            document.tolerance(),
        )
        .unwrap()
        .to_nurbs()
        .unwrap(),
    ];
    let face = Brep::try_planar_face_with_holes(&outer, &holes, document.tolerance()).unwrap();
    let expected = [
        face.edges()[0].curve().clone(),
        face.edges()[1].curve().reversed().unwrap(),
        face.edges()[2].curve().reversed().unwrap(),
    ];
    let face_id = document.add_geometry(Geometry::Brep(face)).unwrap();
    document
        .select_objects_direct([face_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "DupFaceBorder Faces=0")
            .unwrap(),
        "Duplicated 3 curve object(s) in 3 face border(s) from 1 object(s)"
    );
    assert_eq!(document.groups().len(), 0);
    let actual = document
        .objects()
        .skip(1)
        .map(|output| match output.geometry() {
            Geometry::NurbsCurve(curve) => curve.clone(),
            _ => panic!("single-edge face borders must retain exact NURBS curves"),
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);

    let mut cylinder_document = Document::default();
    let cylinder = Brep::try_extruded_curve(
        &holes[0],
        Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 5.0).unwrap(),
        cylinder_document.tolerance(),
    )
    .unwrap();
    let expected_rims = [
        cylinder.edges()[0].curve().clone(),
        cylinder.edges()[1].curve().reversed().unwrap(),
    ];
    let cylinder_id = cylinder_document
        .add_geometry(Geometry::Brep(cylinder))
        .unwrap();
    cylinder_document
        .select_objects_direct([cylinder_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut cylinder_document, "DupFaceBorder Faces=0")
        .unwrap();
    let actual_rims = cylinder_document
        .objects()
        .skip(1)
        .map(|output| match output.geometry() {
            Geometry::NurbsCurve(curve) => curve.clone(),
            _ => panic!("cylinder rims must remain exact NURBS curves"),
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_rims, expected_rims);
}

#[test]
fn duplicate_face_border_rejects_invalid_or_borderless_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Torus 0,0,0 4 1").unwrap();
    let torus = document.objects().next().unwrap().id();
    document
        .select_objects_direct([torus], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupFaceBorder Faces=0"),
        Err(CommandError::NoDuplicateFaceBorders)
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());
    assert!(matches!(
        registry.execute(&mut document, "DupFaceBorder Faces=1"),
        Err(CommandError::DuplicateFaceBorderFaceIndexOutOfRange {
            face: 1,
            face_count: 1,
        })
    ));

    for invalid in [
        "DupFaceBorder Faces=0,0",
        "DupFaceBorder Faces=",
        "DupFaceBorder Faces=0 1,2,3",
        "DupFaceBorder 1,2,3 OutputLayer=Other",
        "DupFaceBorder 1,2,3 OutputLayer=Input OutputLayer=Current",
        "DupFaceBorder FaceIndices=0 Faces=0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    registry.execute(&mut document, "Point 10,10,10").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupFaceBorder Faces=0"),
        Err(CommandError::UnsupportedDuplicateFaceBorderGeometry)
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), history.as_deref());

    registry.execute(&mut document, "SelNone").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "DupFaceBorder Faces=0"),
        Err(CommandError::NoObjectsSelected)
    ));
}
