use super::*;
use viboceros_geometry::Vector3;

fn point(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

#[test]
fn failed_curve_surface_and_brep_fits_leave_in_place_and_copy_document_edits_atomic() {
    use crate::{Document, ObjectAttributes, SelectionMode};
    struct CannotFit;
    impl PointMorph for CannotFit {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            point.translated(Vector3::try_new(0.0, 0.0, 1.0)?)
        }
        fn morph_nurbs_curve(
            &self,
            _: &NurbsCurve,
            tolerance: Tolerance,
        ) -> Result<NurbsCurve, GeometryError> {
            Err(GeometryError::CurveMorphDidNotConverge {
                tolerance: tolerance.absolute(),
                deviation: 1.0,
                maximum: 512,
            })
        }
        fn morph_nurbs_surface(
            &self,
            _: &NurbsSurface,
            tolerance: Tolerance,
        ) -> Result<NurbsSurface, GeometryError> {
            Err(GeometryError::SurfaceMorphDidNotConverge {
                tolerance: tolerance.absolute(),
                deviation: 1.0,
                maximum: 256,
            })
        }
    }
    let sources = [
        Geometry::Line(
            LineSegment::try_new(point(0.0, 0.0), point(1.0, 0.0), Tolerance::DEFAULT).unwrap(),
        ),
        Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(0.0, 1.0),
            ])
            .unwrap(),
        ),
        Geometry::Brep(
            Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    point(0.0, 0.0),
                    point(1.0, 0.0),
                    point(1.0, 1.0),
                    point(0.0, 1.0),
                ])
                .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
    ];
    for source in sources {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(point(0.0, 0.0)))
            .unwrap();
        let shape = document
            .add_geometry_with_attributes(
                source,
                ObjectAttributes::on_layer(document.current_layer_id()).with_name("Source"),
            )
            .unwrap();
        let group = document
            .add_group(Some("Shared".into()), [first, shape])
            .unwrap();
        document
            .select_objects_direct([first, shape], SelectionMode::Replace)
            .unwrap();
        let before_objects = document.objects().cloned().collect::<Vec<_>>();
        let before_group = document.group(group).unwrap().clone();
        let before_history = document.undo_label().map(str::to_owned);
        assert!(document.morph_objects([first, shape], &CannotFit).is_err());
        assert!(
            document
                .copy_objects_morphed([first, shape], &CannotFit)
                .is_err()
        );
        assert_eq!(
            document.objects().cloned().collect::<Vec<_>>(),
            before_objects
        );
        assert_eq!(document.group(group).unwrap(), &before_group);
        assert_eq!(document.undo_label(), before_history.as_deref());
        assert!(document.is_selected(first) && document.is_selected(shape));
        assert_eq!(document.groups().len(), 1);
    }
}

#[test]
fn rational_representation_keeps_polyline_parameters_unlike_explicit_conversion() {
    let source = Polyline3::try_with_parameters(
        vec![point(0.0, 0.0), point(2.0, 0.0), point(2.0, 3.0)],
        vec![-7.0, 3.0, 13.0],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let geometry = Geometry::Polyline(source.clone());
    let native = geometry.nurbs_curve_representation().unwrap().unwrap();
    assert_eq!(native.knots(), &[-7.0, -7.0, 3.0, 13.0, 13.0]);
    for i in 0..=64 {
        let t = -7.0 + 20.0 * i as f64 / 64.0;
        assert!(
            native
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                < 1e-13
        );
    }
    let converted = geometry.converted_to_nurbs_curve().unwrap().unwrap();
    assert_eq!(converted.knots(), &[0.0, 0.0, 2.0, 5.0, 5.0]);
}

fn composite() -> PolyCurve3 {
    PolyCurve3::try_with_segment_domains(
        vec![
            NurbsCurve::try_new(
                2,
                vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)],
                vec![4.0, 4.0, 4.0, 6.0, 6.0, 6.0],
            )
            .unwrap(),
            NurbsCurve::try_new(
                1,
                vec![point(3.0, 0.0), point(4.0, 3.0)],
                vec![-1.0, -1.0, 1.0, 1.0],
            )
            .unwrap(),
        ],
        vec![-10.0, -3.0, 12.0],
    )
    .unwrap()
}

#[test]
fn affine_transform_and_nurbs_conversion_retain_composite_geometry() {
    let source = composite();
    let geometry = Geometry::PolyCurve(source.clone());
    let transform = AffineTransform3::try_new(
        [[2.0, 0.5, 0.0], [0.0, -3.0, 0.0], [0.0, 0.0, 1.0]],
        Vector3::try_new(7.0, 8.0, 9.0).unwrap(),
    )
    .unwrap();
    let transformed = geometry.transformed(transform, Tolerance::DEFAULT).unwrap();
    let Geometry::PolyCurve(result) = &transformed else {
        panic!("transform flattened polycurve")
    };
    assert_eq!(result.parameters(), source.parameters());
    assert_eq!(result.segments().len(), 2);
    assert_eq!(transformed.bounds(), result.control_point_bounds());
    let nurbs = geometry.nurbs_curve_representation().unwrap().unwrap();
    for i in 0..=100 {
        let t = source.parameter_at(i as f64 / 100.0).unwrap();
        let expected = source.evaluate(t).unwrap();
        assert!(nurbs.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-12);
        assert!(
            result
                .evaluate(t)
                .unwrap()
                .distance_to(transform.transform_point(expected).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn nonlinear_morph_keeps_segment_domains_and_connected_junctions() {
    struct Lift;
    impl PointMorph for Lift {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(point.x(), point.y(), point.x() * point.x())
        }
    }
    let source = composite();
    let Geometry::PolyCurve(result) = Geometry::PolyCurve(source.clone())
        .morphed(&Lift, Tolerance::DEFAULT)
        .unwrap()
    else {
        panic!("morph flattened polycurve")
    };
    assert_eq!(result.parameters(), source.parameters());
    for i in 0..=100 {
        let t = source.parameter_at(i as f64 / 100.0).unwrap();
        let expected = Lift.morph_point(source.evaluate(t).unwrap()).unwrap();
        assert!(result.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-8);
    }
}

#[test]
fn duplicate_detection_ignores_global_domain_and_whole_curve_direction() {
    let source = composite();
    let geometry = Geometry::PolyCurve(source.clone());
    assert!(
        geometry
            .geometrically_equals(&Geometry::PolyCurve(source.reversed().unwrap()))
            .unwrap()
    );
    assert!(
        geometry
            .geometrically_equals(&Geometry::PolyCurve(
                source.try_reparameterized(0.0..=1.0).unwrap()
            ))
            .unwrap()
    );
    let shifted = geometry
        .transformed(
            AffineTransform3::from_translation(Vector3::try_new(0.0, 0.0, 1.0).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!(!geometry.geometrically_equals(&shifted).unwrap());
    let mut segments = source.segments().to_vec();
    segments.push(
        NurbsCurve::try_clamped_uniform(1, vec![point(4.0, 3.0), point(0.0, 0.0)])
            .unwrap()
            .into(),
    );
    let closed = PolyCurve3::try_new(segments).unwrap();
    assert!(
        !Geometry::PolyCurve(closed.clone())
            .geometrically_equals(&Geometry::PolyCurve(closed.reversed().unwrap()))
            .unwrap()
    );
    assert!(
        Geometry::PolyCurve(closed.clone())
            .geometrically_equals(&Geometry::PolyCurve(
                closed.try_reparameterized(0.0..=1.0).unwrap()
            ))
            .unwrap()
    );
}
