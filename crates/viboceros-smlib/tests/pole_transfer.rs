#![cfg(feature = "native")]

use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3};
use viboceros_smlib::Shape;

fn pole_count(brep: &Brep) -> usize {
    brep.faces()
        .iter()
        .flat_map(|face| face.loops())
        .flat_map(|boundary| boundary.trims())
        .filter(|trim| trim.edge().is_none())
        .count()
}

fn round_trip_3dm(source: &Brep, tolerance: Tolerance) -> Brep {
    use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pole.3dm");
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(ThreeDmGeometry::Brep(source.clone()), 0)],
    );
    viboceros_io::write_3dm_file(&path, &model).unwrap();
    let model = viboceros_io::read_3dm_file(&path, tolerance).unwrap();
    let ThreeDmGeometry::Brep(result) = &model.objects[0].geometry else {
        panic!("B-rep")
    };
    result.clone()
}

#[test]
fn native_pole_transfer_preserves_singular_boundaries_under_independent_uv_scales() {
    let tolerance = Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap();
    let origin = Point3::try_new(1., -2., 3.).unwrap();
    let frame =
        Frame3::try_from_normal(origin, Vector3::try_new(0., 0., 1.).unwrap(), tolerance).unwrap();
    let mut errors = Vec::new();
    for [u, v] in [[1., 1.], [1e-4, 1.], [1., 1e-4], [1e-4, 1e-4], [1e4, 1e-4]] {
        for whole in [false, true] {
            for swapped in [false, true] {
                let surface = NurbsSurface::try_sphere(frame, 2.)
                    .unwrap()
                    .try_reparameterized(0. ..=u, 0. ..=v)
                    .unwrap();
                let surface = if whole {
                    surface
                } else {
                    surface
                        .try_trimmed(surface.domain_u(), 0. ..=v / 2.)
                        .unwrap()
                };
                let surface = if swapped {
                    surface.try_swapped_uv().unwrap()
                } else {
                    surface
                };
                let input = Brep::try_surface_face(surface, tolerance).unwrap();
                // Transposition reverses a closed surface's orientation.
                let input = if swapped { input.reversed() } else { input };
                let before = input.clone();
                let expected_poles = if whole { 2 } else { 1 };
                assert_eq!(pole_count(&input), expected_poles);
                let result = Shape::from_brep(&input, tolerance)
                    .map_err(|error| format!("import: {error}"))
                    .and_then(|native| {
                        native
                            .to_brep(tolerance)
                            .map_err(|error| format!("export: {error}"))
                    });
                match result {
                    Ok(output) => {
                        let poles = pole_count(&output);
                        eprintln!(
                            "POLE_TRANSFER u={u:e} v={v:e} whole={whole} swapped={swapped} poles={poles}/{expected_poles}"
                        );
                        if poles != expected_poles {
                            errors.push(format!("u={u:e} v={v:e} whole={whole} swapped={swapped}: poles {poles}/{expected_poles}"));
                            continue;
                        }
                        assert_eq!(output.is_solid(), whole);
                        assert!(output.is_manifold());
                        if whole {
                            let volume = output.signed_volume(Tolerance::DEFAULT).unwrap();
                            assert!((volume - 32. * std::f64::consts::PI / 3.).abs() < 1e-7);
                        }
                        let area = output.area(Tolerance::DEFAULT).unwrap();
                        let expected_area = if whole { 16. } else { 8. } * std::f64::consts::PI;
                        assert!((area - expected_area).abs() < 1e-7, "area={area}");
                        for face in output.faces() {
                            for (u, v) in [(0.13, 0.17), (0.41, 0.43), (0.79, 0.81)] {
                                let u = face.surface().parameter_at_u(u).unwrap();
                                let v = face.surface().parameter_at_v(v).unwrap();
                                let p = face.surface().evaluate(u, v).unwrap();
                                assert!((p.distance_to(origin).unwrap() - 2.).abs() < 1e-12);
                                let normal = face.surface().normal_at(u, v).unwrap();
                                let normal = if face.is_reversed() {
                                    normal.opposite()
                                } else {
                                    normal
                                };
                                assert!(
                                    normal
                                        .as_vector()
                                        .dot(origin.vector_to(p).unwrap())
                                        .unwrap()
                                        > 2. - 1e-12
                                );
                            }
                        }
                        if u == 1e-4 && v == 1e-4 {
                            let imported = round_trip_3dm(&output, tolerance);
                            assert_eq!(pole_count(&imported), expected_poles);
                            assert_eq!(imported.is_solid(), whole);
                            assert!(
                                (imported.area(Tolerance::DEFAULT).unwrap() - expected_area).abs()
                                    < 1e-7
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "POLE_TRANSFER u={u:e} v={v:e} whole={whole} swapped={swapped} error={error}"
                        );
                        errors.push(format!(
                            "u={u:e} v={v:e} whole={whole} swapped={swapped}: {error}"
                        ));
                    }
                }
                assert_eq!(input, before);
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
