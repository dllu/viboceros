use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{Frame3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3};

#[test]
fn create_uv_curve_keeps_on_surface_inputs_native_parameters_and_history_across_units() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    let mut failures = Vec::new();
    for exponent in [-20, 0, 40, 80] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let pose = Frame3::try_from_unit_normal(
            Point3::try_new(5. * scale, -7. * scale, 11. * scale).unwrap(),
            Vector3::try_new(2., 3., 6.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
        )
        .unwrap();
        let point = |x, y| pose.point_at([x * scale, y * scale, 0.]).unwrap();
        let base = NurbsSurface::try_bilinear([
            point(0., 0.),
            point(10., 0.),
            point(8., 10.),
            point(0., 10.),
        ])
        .unwrap();
        let curve = NurbsCurve::try_new(
            3,
            vec![
                point(0., 2.),
                point(3.2, 20. / 3.),
                point(82. / 15., 8.),
                point(8.8, 6.),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        for (ue, ve) in [
            (0, 0),
            (-200, -200),
            (200, 200),
            (-600, -600),
            (600, 600),
            (-600, 600),
            (600, -600),
        ] {
            for te in [-600, 600] {
                checked += 1;
                let mut doc = Document::default();
                doc.set_tolerance(tolerance);
                let surface = base
                    .try_reparameterized(0. ..=2_f64.powi(ue), 0. ..=2_f64.powi(ve))
                    .unwrap();
                let source = curve.try_reparameterized(0. ..=2_f64.powi(te)).unwrap();
                let layer = doc.current_layer_id();
                let target = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
                let id = doc
                    .add_geometry_with_attributes(
                        Geometry::NurbsCurve(source.clone()),
                        ObjectAttributes::on_layer(layer).with_name("source"),
                    )
                    .unwrap();
                doc.select_object(id, SelectionMode::Replace).unwrap();
                doc.clear_history().unwrap();
                let before = doc.objects().cloned().collect::<Vec<_>>();
                let label = format!("scale=2^{exponent} UV=2^({ue},{ve}) source=2^{te}");
                match registry.execute(&mut doc, &format!("CreateUVCrv Surface={target}")) {
                    Ok(message) => {
                        if !message.contains("0 input(s) farther") {
                            failures.push(format!("{label}: {message}"));
                        }
                        let copy = doc
                            .objects()
                            .find(|o| o.id() != id && o.attributes().name() == Some("source"))
                            .unwrap();
                        let Geometry::NurbsCurve(flat) = copy.geometry() else {
                            panic!("flattened curve");
                        };
                        assert_eq!(flat.domain(), source.domain());
                        let border = doc
                            .objects()
                            .find_map(|o| {
                                if let Geometry::PolyCurve(c) = o.geometry() {
                                    Some(c)
                                } else {
                                    None
                                }
                            })
                            .unwrap();
                        assert_eq!(border.segments().len(), 4);
                        assert!(border.is_closed().unwrap());
                        let height = (100_f64 + 4. * (7_f64 / 8.).powi(2)).sqrt() * scale;
                        for (i, [x, y]) in [
                            [5. * scale, 0.],
                            [10. * scale, height / 2.],
                            [5. * scale, height],
                            [0., height / 2.],
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let t = border.parameters()[i]
                                .mul_add(0.5, border.parameters()[i + 1] * 0.5);
                            let expected = Point3::try_new(x, y, 0.).unwrap();
                            assert!(
                                border.evaluate(t).unwrap().distance_to(expected).unwrap() / scale
                                    < 2e-7
                            );
                        }

                        assert!(border.parameters().windows(2).all(|p| p[0] < p[1]));
                        for (i, segment) in border.segments().iter().enumerate() {
                            let width = 2_f64.powi(if i % 2 == 0 { ue } else { ve });
                            assert_eq!(segment.domain(), 0. ..=width);
                        }

                        for i in 0..=32 {
                            let t = i as f64 / 32.;
                            let p = flat.evaluate(flat.parameter_at(t).unwrap()).unwrap();
                            let v = 0.2 + 1.4 * t - t * t;
                            let h = (100_f64 + 4. * (7_f64 / 8.).powi(2)).sqrt();
                            let expected =
                                Point3::try_new(10. * scale * t, h * scale * v, 0.).unwrap();
                            if p.distance_to(expected).unwrap() / scale > 2e-7 {
                                failures.push(format!("{label}: wrong flat point at {t}"));
                                break;
                            }
                        }
                        for object in &before {
                            assert_eq!(doc.object(object.id()).unwrap(), object);
                        }
                        let after = doc.objects().cloned().collect::<Vec<_>>();
                        doc.undo().unwrap();
                        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                        doc.redo().unwrap();
                        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                    }
                    Err(error) => failures.push(format!("{label}: {error:?}")),
                }
            }
        }
    }
    eprintln!(
        "CREATE_UV_UNITS cases={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 56);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
