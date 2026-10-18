use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectAttributes, ObjectId, SelectionMode};
use viboceros_geometry::{Frame3, NurbsSurface, Point3, Tolerance, Vector3};

const SCALES: [i32; 4] = [-20, 0, 40, 80];
const DOMAINS: [(i32, i32); 7] = [
    (0, 0),
    (-200, -200),
    (200, 200),
    (-600, -600),
    (600, 600),
    (-600, 600),
    (600, -600),
];
fn unit(v: [f64; 3]) -> Vector3 {
    Vector3::try_from(v)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
        .as_vector()
}
fn local_point(u: f64, v: f64, warp: f64) -> [f64; 3] {
    [2. * u + v, 3. * v, warp * u * v]
}
fn axes(u: f64, v: f64, warp: f64) -> [Vector3; 3] {
    let x = unit([2., 0., warp * v]);
    let z = unit([-3. * warp * v, warp * v - 2. * warp * u, 6.]);
    [
        x,
        z.cross(x)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
            .as_vector(),
        z,
    ]
}
fn setup(
    exponent: i32,
    ue: i32,
    ve: i32,
    warp: f64,
) -> (f64, Frame3, Document, [ObjectId; 3], ObjectId) {
    let scale = 2_f64.powi(exponent);
    let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
    let pose = Frame3::try_from_unit_normal(
        Point3::try_new(5. * scale, -7. * scale, 11. * scale).unwrap(),
        unit([2., 3., 6.]).normalized_nonzero().unwrap(),
    )
    .unwrap();
    let mut doc = Document::default();
    doc.set_tolerance(tolerance);
    let layer = doc.current_layer_id();
    let sources = std::array::from_fn(|i| {
        let mut p = [scale, -2. * scale, 3. * scale];
        p[i] += 0.1 * scale;
        doc.add_geometry_with_attributes(
            Geometry::Point(Point3::try_from(p).unwrap()),
            ObjectAttributes::on_layer(layer).with_name(format!("source-{i}")),
        )
        .unwrap()
    });
    let surface =
        NurbsSurface::try_bilinear([(0., 0.), (1., 0.), (1., 1.), (0., 1.)].map(|(u, v)| {
            pose.point_at(local_point(u, v, warp).map(|x| x * scale))
                .unwrap()
        }))
        .unwrap()
        .try_reparameterized(0. ..=2_f64.powi(ue), 0. ..=2_f64.powi(ve))
        .unwrap();
    let target = doc
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface),
            ObjectAttributes::on_layer(layer).with_name("target"),
        )
        .unwrap();
    doc.select_objects_direct(sources, SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (scale, pose, doc, sources, target)
}
fn token(p: Point3) -> String {
    let [x, y, z] = p.to_array();
    format!("{x},{y},{z}")
}
fn copies(doc: &Document, sources: &[ObjectId; 3], target: ObjectId) -> Vec<Geometry> {
    doc.objects()
        .filter(|o| !sources.contains(&o.id()) && o.id() != target)
        .map(|o| o.geometry().clone())
        .collect()
}
fn check_history(
    doc: &mut Document,
    before: Vec<viboceros_document::Object>,
    sources: &[ObjectId; 3],
    target: ObjectId,
) {
    for original in &before {
        assert_eq!(doc.object(original.id()).unwrap(), original);
    }
    assert_eq!(
        doc.selected_object_ids().collect::<Vec<_>>(),
        sources.to_vec()
    );
    assert!(!doc.is_selected(target));
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn surface_arrays_keep_analytic_placements_when_uv_and_up_units_change() {
    let mut checked = 0;
    let mut failures = Vec::new();
    let registry = CommandRegistry::with_builtins();
    for exponent in SCALES {
        for (ue, ve) in DOMAINS {
            for warp in [0., 1.] {
                for mode in ["UV", "Isocurve"] {
                    let mut reference = None;
                    for ne in [-200, 0, 200] {
                        checked += 1;
                        let (scale, pose, mut doc, sources, target) = setup(exponent, ue, ve, warp);
                        let before = doc.objects().cloned().collect::<Vec<_>>();
                        let command = format!(
                            "ArraySrf 3 2 BasePoint={} Up=0,0,{} SurfaceName=target Mode={mode}",
                            token(Point3::try_new(scale, -2. * scale, 3. * scale).unwrap()),
                            2_f64.powi(ne)
                        );
                        let label = format!(
                            "scale=2^{exponent} UV=2^({ue},{ve}) warp={warp} Up=2^{ne} Mode={mode}"
                        );
                        match registry.execute(&mut doc, &command) {
                            Ok(_) => {
                                assert_eq!(doc.objects().len(), 22);
                                let actual = copies(&doc, &sources, target);
                                let mut expected = Vec::new();
                                for v in [0., 1.] {
                                    for u in [0., 0.5, 1.] {
                                        for axis in axes(u, v, warp) {
                                            let p = local_point(u, v, warp);
                                            let d = axis.to_array();
                                            expected.push(
                                                pose.point_at(std::array::from_fn(|i| {
                                                    scale * (p[i] + 0.1 * d[i])
                                                }))
                                                .unwrap(),
                                            );
                                        }
                                    }
                                }
                                for geometry in &actual {
                                    let Geometry::Point(p) = geometry else {
                                        panic!("point copy");
                                    };
                                    let index = expected
                                        .iter()
                                        .position(|q| p.distance_to(*q).unwrap() / scale < 2e-11)
                                        .unwrap_or_else(|| panic!("{label}: unexpected {p:?}"));
                                    expected.swap_remove(index);
                                }
                                assert!(expected.is_empty());
                                if let Some(reference) = &reference {
                                    assert_eq!(&actual, reference, "{label}");
                                } else {
                                    reference = Some(actual);
                                }
                                check_history(&mut doc, before, &sources, target);
                            }
                            Err(error) => failures.push(format!("{label}: {error:?}")),
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "SURFACE_PLACEMENT arrays={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 336);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn rigid_and_nonrigid_surface_orientation_keep_analytic_placements_across_uv_units() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    let mut failures = Vec::new();
    for exponent in SCALES {
        for (ue, ve) in DOMAINS {
            for warp in [0., 1.] {
                for rigid in [true, false] {
                    for flip in [false, true] {
                        let mut reference: Option<Vec<Geometry>> = None;
                        for ne in [-200, 0, 200] {
                            checked += 1;
                            let (scale, pose, mut doc, sources, target) =
                                setup(exponent, ue, ve, warp);
                            let before = doc.objects().cloned().collect::<Vec<_>>();
                            let base = Point3::try_new(scale, -2. * scale, 3. * scale).unwrap();
                            let reference_point =
                                Point3::try_new(2. * scale, -2. * scale, 3. * scale).unwrap();
                            let anchor = pose
                                .point_at(local_point(0.25, 0.5, warp).map(|x| x * scale))
                                .unwrap();
                            let command = format!(
                                "OrientOnSrf {} {} {} SurfaceName=target Copy=Yes Rigid={} Rotation=60 Flip={} SourceNormal=0,0,{}",
                                token(base),
                                token(reference_point),
                                token(anchor),
                                if rigid { "Yes" } else { "No" },
                                if flip { "Yes" } else { "No" },
                                2_f64.powi(ne)
                            );
                            let label = format!(
                                "scale=2^{exponent} UV=2^({ue},{ve}) warp={warp} rigid={rigid} flip={flip} normal=2^{ne}"
                            );
                            match registry.execute(&mut doc, &command) {
                                Ok(_) => {
                                    assert_eq!(doc.objects().len(), 7);
                                    let actual = copies(&doc, &sources, target);
                                    let [x, y, z] = axes(0.25, 0.5, warp);
                                    let du = Vector3::try_new(2., 0., warp * 0.5).unwrap();
                                    let dv = Vector3::try_new(1., 3., warp * 0.25).unwrap();
                                    for i in 0..3 {
                                        let object = doc
                                            .objects()
                                            .find(|o| {
                                                o.id() != sources[i]
                                                    && o.attributes().name()
                                                        == Some(format!("source-{i}").as_str())
                                            })
                                            .unwrap();
                                        let Geometry::Point(actual) = object.geometry() else {
                                            panic!("point copy");
                                        };
                                        let mut coordinates = [0.; 3];
                                        coordinates[i] = 0.1;
                                        let [a, b, c] = coordinates;
                                        let (sin, cos) = std::f64::consts::FRAC_PI_3.sin_cos();
                                        let sign = if flip { -1. } else { 1. };
                                        let tx = cos * a - sin * b;
                                        let ty = sign * (sin * a + cos * b);
                                        let tz = sign * c;
                                        let expected = if rigid {
                                            let p = local_point(0.25, 0.5, warp);
                                            let x = x.to_array();
                                            let y = y.to_array();
                                            let z = z.to_array();
                                            pose.point_at(std::array::from_fn(|i| {
                                                scale * (p[i] + tx * x[i] + ty * y[i] + tz * z[i])
                                            }))
                                            .unwrap()
                                        } else {
                                            let vs = dv.dot(y).unwrap();
                                            let u = 0.25
                                                + (tx - dv.dot(x).unwrap() * ty / vs)
                                                    / du.length().unwrap();
                                            let v = 0.5 + ty / vs;
                                            let p = local_point(u, v, warp);
                                            let n = unit([
                                                -3. * warp * v,
                                                warp * v - 2. * warp * u,
                                                6.,
                                            ])
                                            .to_array();
                                            pose.point_at(std::array::from_fn(|i| {
                                                scale * (p[i] + tz * n[i])
                                            }))
                                            .unwrap()
                                        };
                                        assert!(
                                            actual.distance_to(expected).unwrap() / scale < 2e-6,
                                            "{label}: expected={expected:?}, actual={actual:?}"
                                        );
                                    }
                                    if let Some(reference) = &reference {
                                        assert_eq!(&actual, reference, "{label}");
                                    } else {
                                        reference = Some(actual);
                                    }
                                    check_history(&mut doc, before, &sources, target);
                                }
                                Err(error) => failures.push(format!("{label}: {error:?}")),
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "SURFACE_PLACEMENT orientations={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 672);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn invalid_surface_placements_preserve_history_and_source_reference_policy() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    for exponent in SCALES {
        let (scale, pose, mut doc, sources, _) = setup(exponent, 200, -200, 0.);
        let base = Point3::try_new(scale, -2. * scale, 3. * scale).unwrap();
        let reference = Point3::try_new(2. * scale, -2. * scale, 3. * scale).unwrap();
        let target = pose
            .point_at(local_point(0.25, 0.5, 0.).map(|x| x * scale))
            .unwrap();
        let orient = format!(
            "OrientOnSrf {} {} {} SurfaceName=target",
            token(base),
            token(reference),
            token(target)
        );
        doc.add_geometry(Geometry::Point(base)).unwrap();
        doc.undo().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let selection = doc.selected_object_ids().collect::<Vec<_>>();
        for command in [
            format!(
                "ArraySrf 3 2 BasePoint={} Up=0,0,0 SurfaceName=target",
                token(base)
            ),
            format!("{orient} SourceNormal=0,0,0"),
            format!("{orient} SourceNormal=1,0,0"),
            format!(
                "OrientOnSrf {} {} {} SurfaceName=target",
                token(base),
                token(base),
                token(target)
            ),
            format!(
                "{orient} Rigid=No ConstrainNormal=Yes DestinationNormal={}",
                token(Point3::try_from(pose.x_axis().as_vector().to_array()).unwrap())
            ),
        ] {
            assert!(
                registry.execute(&mut doc, &command).is_err(),
                "accepted {command}"
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selection);
            assert!(!doc.can_undo() && doc.can_redo());
            checked += 1;
        }
        // Reference direction admission remains independent of document size.
        // A nonzero preferred normal projection below the document's angular
        // threshold still defines the same source XY plane.
        let command = format!("{orient} SourceNormal=1,0,{} Copy=Yes", 2_f64.powi(-80));
        registry.execute(&mut doc, &command).unwrap();
        assert_eq!(doc.objects().len(), 7);
        assert!(sources.iter().all(|id| doc.object(*id).is_some()));
        checked += 1;
    }
    assert_eq!(checked, 24);
    eprintln!("SURFACE_PLACEMENT admission={checked}");
}
