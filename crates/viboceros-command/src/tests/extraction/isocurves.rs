//! extraction isocurves command regressions.
use super::*;

#[test]
fn extract_isocurve_creates_exact_rational_u_and_v_curves_at_a_surface_point() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Cylinder 0,0,0 2 5 Solid=No")
        .unwrap();
    let source = document.objects().next().unwrap().id();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(&mut document, "ExtractIsocurve 2,0,2 Direction=Both")
        .unwrap();
    assert_eq!(
        message,
        "Extracted 2 exact U/V isocurve(s) from 1 surface(s)"
    );
    assert_eq!(document.objects().len(), 3);
    assert!(!document.is_selected(source));
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert!(
        outputs
            .iter()
            .all(|output| document.is_selected(output.id()))
    );

    let Geometry::NurbsCurve(u_curve) = outputs[0].geometry() else {
        panic!("the first Both result must run in U")
    };
    assert_eq!(u_curve.degree(), 2);
    assert!(u_curve.is_rational());
    assert!(u_curve.is_closed().unwrap());
    for parameter in [0.0, std::f64::consts::PI, std::f64::consts::TAU] {
        let point = u_curve.evaluate(parameter).unwrap();
        assert!((point.x().hypot(point.y()) - 2.0).abs() < 1.0e-12);
        assert!((point.z() - 2.0).abs() < 1.0e-12);
    }

    let Geometry::NurbsCurve(v_curve) = outputs[1].geometry() else {
        panic!("the second Both result must run in V")
    };
    assert_eq!(v_curve.degree(), 1);
    assert_eq!(
        v_curve.evaluate(*v_curve.domain().start()).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        v_curve.evaluate(*v_curve.domain().end()).unwrap(),
        Point3::try_new(2.0, 0.0, 5.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("ExtractIsocurve"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn extract_isocurve_batches_surfaces_and_rejects_invalid_inputs_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for offset in [0.0, 10.0] {
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    Point3::try_new(offset, 0.0, 0.0).unwrap(),
                    Point3::try_new(offset + 4.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(offset + 4.0, 3.0, 0.0).unwrap(),
                    Point3::try_new(offset, 3.0, 0.0).unwrap(),
                ])
                .unwrap(),
            ))
            .unwrap();
    }
    let surfaces = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(surfaces.iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "IsoCurve Direction _V 2 1 0")
        .unwrap();
    let outputs = document.objects().skip(2).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for output in outputs {
        let Geometry::NurbsCurve(curve) = output.geometry() else {
            panic!("ExtractIsocurve must output exact curves")
        };
        assert_eq!(curve.degree(), 1);
        assert!((curve.length(document.tolerance()).unwrap() - 3.0).abs() < 1.0e-12);
    }

    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    let line = document.objects().last().unwrap().id();
    document
        .select_objects_direct([surfaces[0], line], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "ExtractIsocurve 1,1,0"),
        Err(CommandError::UnsupportedExtractIsocurveGeometry)
    ));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.undo_label(), history.as_deref());
    for invalid in [
        "ExtractIsocurve",
        "ExtractIsocurve 1,1 Direction=Sideways",
        "ExtractIsocurve 1,1 Direction=U Direction=V",
        "ExtractIsocurve 1,1 IgnoreTrims=Yes IgnoreTrims=No",
        "ExtractIsocurve 1,1 Direction",
        "ExtractIsocurve 1,1 IgnoreTrims",
        "ExtractIsocurve 1,1,0 extra",
        "ExtractIsocurve ExtractAll 1,1,0",
        "ExtractIsocurve ExtractAll ExtractAll",
        "ExtractIsocurve ExtractAll=Yes",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
        assert_eq!(document.objects().len(), 3);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn extract_isocurve_clips_exact_brep_faces_around_holes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let closed_polyline = |coordinates: &[(Real, Real)]| {
        let mut points = coordinates
            .iter()
            .map(|(x, y)| Point3::try_new(*x, *y, 0.0).unwrap())
            .collect::<Vec<_>>();
        points.push(points[0]);
        Polyline3::try_new(points, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let outer = closed_polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
    let hole = closed_polyline(&[(4.0, 4.0), (4.0, 6.0), (6.0, 6.0), (6.0, 4.0)]);
    let source = document
        .add_geometry(Geometry::Brep(
            Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(&mut document, "ExtractIsocurve 2,5,0 Direction=Both")
        .unwrap();
    assert_eq!(
        message,
        "Extracted 3 exact U/V isocurve(s) from 1 surface(s)"
    );
    assert!(!document.is_selected(source));
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 3);
    let lengths = outputs
        .iter()
        .map(|object| {
            assert!(document.is_selected(object.id()));
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("trim-aware isocurves must remain exact NURBS curves")
            };
            assert_eq!(curve.degree(), 1);
            curve.length(document.tolerance()).unwrap()
        })
        .collect::<Vec<_>>();
    assert!(Tolerance::DEFAULT.approx_eq(lengths[0], 4.0));
    assert!(Tolerance::DEFAULT.approx_eq(lengths[1], 4.0));
    assert!(Tolerance::DEFAULT.approx_eq(lengths[2], 10.0));
    assert_eq!(document.undo_label(), Some("ExtractIsocurve"));

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "ExtractIsocurve 5,5,0 Direction=Both"),
        Err(CommandError::NoExtractableIsocurves)
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());

    registry
        .execute(
            &mut document,
            "ExtractIsocurve 5,5,0 Direction=Both IgnoreTrims=Yes",
        )
        .unwrap();
    let untrimmed_outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(untrimmed_outputs.len(), 2);
    assert!(untrimmed_outputs.iter().all(|object| {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            return false;
        };
        document.is_selected(object.id())
            && Tolerance::DEFAULT.approx_eq(curve.length(document.tolerance()).unwrap(), 10.0)
    }));
    registry.execute(&mut document, "Undo").unwrap();

    let mut solid_document = Document::default();
    registry
        .execute(&mut solid_document, "Cylinder 0,0,0 2 5 Solid=Yes")
        .unwrap();
    let solid = solid_document.objects().next().unwrap().id();
    solid_document
        .select_objects_direct([solid], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut solid_document, "ExtractIsocurve 2,0,2 Direction=Both")
        .unwrap();
    let solid_outputs = solid_document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(solid_outputs.len(), 2);
    assert!(solid_outputs.iter().all(|object| {
        matches!(object.geometry(), Geometry::NurbsCurve(_))
            && solid_document.is_selected(object.id())
    }));
}

#[test]
fn extract_all_isocurves_matches_rhino_wire_density_rules() {
    let registry = CommandRegistry::with_builtins();
    for (wire_density, expected_count) in [(-1, 4), (0, 6), (1, 6), (2, 10), (3, 14)] {
        let mut document = Document::default();
        let surface = rational_multi_span_surface();
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .try_with_wire_density(wire_density)
            .unwrap();
        let source = document
            .add_geometry_with_attributes(Geometry::NurbsSurface(surface.clone()), attributes)
            .unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "ExtractIsocurve ExtractAll Direction=Both",)
                .unwrap(),
            format!("Extracted {expected_count} exact U/V isocurve(s) from 1 surface(s)")
        );
        assert_eq!(document.objects().len(), expected_count + 1);
        assert!(!document.is_selected(source));
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::NurbsSurface(surface.clone())
        );
        assert!(
            document
                .objects()
                .skip(1)
                .all(|object| document.is_selected(object.id()))
        );

        if wire_density == 2 {
            let fixed_v = [10.0, 10.75, 11.5, 12.75, 14.0];
            let fixed_u = [0.0, 0.35, 0.7, 1.35, 2.0];
            for (object, v) in document.objects().skip(1).take(5).zip(fixed_v) {
                let Geometry::NurbsCurve(curve) = object.geometry() else {
                    panic!("U wire must remain an exact NURBS curve")
                };
                assert_eq!(curve.domain(), surface.domain_u());
                for u in [0.0, 0.8, 2.0] {
                    assert!(
                        curve
                            .evaluate(u)
                            .unwrap()
                            .is_near(surface.evaluate(u, v).unwrap(), Tolerance::DEFAULT,)
                    );
                }
            }
            for (object, u) in document.objects().skip(6).zip(fixed_u) {
                let Geometry::NurbsCurve(curve) = object.geometry() else {
                    panic!("V wire must remain an exact NURBS curve")
                };
                // Density-generated curves use local knot origins;
                // the source surface's native V domain remains [10,14].
                assert_eq!(curve.domain(), 0.0..=4.0);
                assert_eq!(
                    curve.knots(),
                    surface
                        .knots_v()
                        .iter()
                        .map(|k| k - 10.)
                        .collect::<Vec<_>>()
                );
                for v in [10.0, 12.0, 14.0] {
                    assert!(
                        curve
                            .evaluate(v - 10.)
                            .unwrap()
                            .is_near(surface.evaluate(u, v).unwrap(), Tolerance::DEFAULT,)
                    );
                }
            }
        }
    }
}

#[test]
fn extract_all_isocurves_retains_both_closed_seams_and_skips_singular_wires() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Cylinder 0,0,0 2 5 Solid=No")
        .unwrap();
    let cylinder = document.objects().next().unwrap().id();
    document
        .select_objects_direct([cylinder], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractIsocurve ExtractAll Direction=Both",)
            .unwrap(),
        "Extracted 8 exact U/V isocurve(s) from 1 surface(s)"
    );
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 8);
    for output in &outputs[..3] {
        let Geometry::NurbsCurve(curve) = output.geometry() else {
            panic!("cylinder U wires must be NURBS circles")
        };
        assert!(curve.is_closed().unwrap());
    }
    let seam_curves = [&outputs[3], &outputs[7]];
    let mut seam_endpoints = Vec::new();
    for output in seam_curves {
        let Geometry::NurbsCurve(curve) = output.geometry() else {
            panic!("cylinder seam wires must be NURBS lines")
        };
        assert!(Tolerance::DEFAULT.approx_eq(curve.length(document.tolerance()).unwrap(), 5.0,));
        seam_endpoints.push((
            curve.evaluate(*curve.domain().start()).unwrap(),
            curve.evaluate(*curve.domain().end()).unwrap(),
        ));
    }
    assert_eq!(seam_endpoints[0], seam_endpoints[1]);

    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "Sphere 0,0,0 3").unwrap();
    let sphere = document.objects().last().unwrap().id();
    document
        .select_objects_direct([sphere], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "ExtractIsocurve ExtractAll Direction=Both")
        .unwrap();
    assert!(document.objects().skip(2).all(|object| {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            return false;
        };
        nurbs_curve_has_extent(curve)
    }));
}

#[test]
fn extract_all_isocurves_can_respect_or_ignore_brep_trims() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let closed_polyline = |coordinates: &[(Real, Real)]| {
        let mut points = coordinates
            .iter()
            .map(|(x, y)| Point3::try_new(*x, *y, 0.0).unwrap())
            .collect::<Vec<_>>();
        points.push(points[0]);
        Polyline3::try_new(points, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let outer = closed_polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
    let hole = closed_polyline(&[(4.0, 4.0), (4.0, 6.0), (6.0, 6.0), (6.0, 4.0)]);
    let source = document
        .add_geometry(Geometry::Brep(
            Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractIsocurve ExtractAll Direction=Both IgnoreTrims=No",
            )
            .unwrap(),
        "Extracted 8 exact U/V isocurve(s) from 1 surface(s)"
    );
    let trimmed_lengths = document
        .objects()
        .skip(1)
        .map(|object| match object.geometry() {
            Geometry::NurbsCurve(curve) => curve.length(document.tolerance()).unwrap(),
            _ => panic!("trim-aware wires must remain NURBS curves"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        trimmed_lengths
            .iter()
            .filter(|length| Tolerance::DEFAULT.approx_eq(**length, 4.0))
            .count(),
        4
    );
    assert_eq!(
        trimmed_lengths
            .iter()
            .filter(|length| Tolerance::DEFAULT.approx_eq(**length, 10.0))
            .count(),
        4
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractIsocurve ExtractAll Direction=Both IgnoreTrims=Yes",
            )
            .unwrap(),
        "Extracted 6 exact U/V isocurve(s) from 1 surface(s)"
    );
    assert!(document.objects().skip(1).all(|object| {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            return false;
        };
        Tolerance::DEFAULT.approx_eq(curve.length(document.tolerance()).unwrap(), 10.0)
    }));
}
