use super::*;
use viboceros_geometry::{BrepLoop, BrepTrim, NurbsCurve2, Point2, WeightedPoint2};

fn translated_uv_brep(surface: NurbsSurface, offset: [Real; 2]) -> Brep {
    // Independent 3D edges keep their original, well-resolved domains. Only
    // the face's UV representation is translated in this regression.
    let brep = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    let surface = surface
        .try_reparameterized(offset[0]..=offset[0] + 1., offset[1]..=offset[1] + 1.)
        .unwrap();
    let loops = brep.faces()[0]
        .loops()
        .iter()
        .map(|boundary| {
            BrepLoop::try_new(
                boundary.loop_type(),
                boundary
                    .trims()
                    .iter()
                    .map(|trim| {
                        let curve = trim.curve();
                        let controls = curve
                            .control_points()
                            .iter()
                            .map(|c| {
                                WeightedPoint2::try_new(
                                    Point2::try_new(
                                        c.point().x() + offset[0],
                                        c.point().y() + offset[1],
                                    )
                                    .unwrap(),
                                    c.weight(),
                                )
                                .unwrap()
                            })
                            .collect();
                        BrepTrim::try_new(
                            trim.vertices(),
                            trim.edge(),
                            trim.is_reversed_3d(),
                            NurbsCurve2::try_new_rational(
                                curve.degree(),
                                controls,
                                curve.knots().to_vec(),
                            )
                            .unwrap(),
                            trim.trim_type(),
                            trim.iso(),
                            trim.tolerance(),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    Brep::try_new(
        brep.vertices().to_vec(),
        brep.edges().to_vec(),
        vec![BrepFace::try_new(surface, false, loops).unwrap()],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn extract_all_brep_stations_are_not_rounded_to_the_native_uv_grid() {
    let registry = CommandRegistry::with_builtins();
    for offset in [[0., 0.], [1e12, -2e12], [-1e12, 2e12]] {
        for (direction, axes) in [("U", vec![0]), ("V", vec![1]), ("Both", vec![0, 1])] {
            let mut document = Document::default();
            let surface = NurbsSurface::try_bilinear([
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
                Point3::try_new(1., 1., 1.).unwrap(),
                Point3::try_new(0., 1., 0.).unwrap(),
            ])
            .unwrap();
            let geometry = Geometry::Brep(translated_uv_brep(surface, offset));
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
                    &format!("ExtractIsocurve ExtractAll Direction={direction}"),
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
