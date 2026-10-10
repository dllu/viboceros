#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Point3, Tolerance};
use viboceros_smlib::{BooleanOperation, Solid};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn native_box_transfers_shared_edges_faces_and_oriented_loops() {
    let solid = Solid::box_solid(p(0., 0., 0.), [2., 3., 4.]).unwrap();
    let before = solid.properties(1e-8).unwrap();
    let brep = solid.to_brep(Tolerance::DEFAULT).unwrap();
    assert_eq!(brep.vertices().len(), 8);
    assert_eq!(brep.edges().len(), 12);
    assert_eq!(brep.faces().len(), 6);
    assert!(brep.is_solid());
    assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-9);
    assert_eq!(solid.properties(1e-8).unwrap(), before);
}

#[test]
fn curved_solid_transfer_covers_cylinders_spheres_and_intersecting_cuts() {
    let box_solid = Solid::box_solid(p(0., 0., 0.), [10.; 3]).unwrap();
    let cylinder = Solid::cylinder(p(5., 5., -1.), 2., 12.).unwrap();
    let sphere = Solid::sphere(p(5., 5., 10.), 2.).unwrap();
    let through = box_solid
        .boolean(&cylinder, BooleanOperation::Difference)
        .unwrap();
    let pocket = box_solid
        .boolean(&sphere, BooleanOperation::Difference)
        .unwrap();
    let mut errors = Vec::new();
    for (name, solid, expected) in [
        ("cylinder", &cylinder, 48. * std::f64::consts::PI),
        ("sphere", &sphere, 32. * std::f64::consts::PI / 3.),
        ("through_hole", &through, 1000. - 40. * std::f64::consts::PI),
        ("pocket", &pocket, 1000. - 16. * std::f64::consts::PI / 3.),
    ] {
        match solid.to_brep(Tolerance::DEFAULT) {
            Ok(brep) => {
                assert!(brep.is_solid());
                let volume = brep.signed_volume(Tolerance::DEFAULT).unwrap();
                assert!(
                    (volume - expected).abs() < expected * 1e-9,
                    "{name}: {volume} != {expected}"
                );
                println!(
                    "{name}: {} faces, {} edges",
                    brep.faces().len(),
                    brep.edges().len()
                );
            }
            Err(error) => errors.push(format!("{name}: {error}")),
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

fn round_trip_3dm(source: &Brep) -> Brep {
    use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.3dm");
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
    let model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    let ThreeDmGeometry::Brep(result) = model.objects[0].geometry.clone() else {
        panic!("expected exact B-rep")
    };
    result
}

fn compare_brep(source: &Brep, result: &Brep, name: &str) {
    assert_eq!(
        source.vertices().len(),
        result.vertices().len(),
        "{name} vertices"
    );
    assert_eq!(source.edges().len(), result.edges().len(), "{name} edges");
    assert_eq!(source.faces().len(), result.faces().len(), "{name} faces");
    for (a, b) in source.vertices().iter().zip(result.vertices()) {
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
        assert_eq!(a.tolerance(), b.tolerance());
    }
    for (a, b) in source.edges().iter().zip(result.edges()) {
        assert_eq!(a.vertices(), b.vertices());
        assert_eq!(a.tolerance(), b.tolerance());
        assert_eq!(a.curve().degree(), b.curve().degree());
        assert_eq!(a.curve().knots(), b.curve().knots());
        assert_eq!(
            a.curve().control_points().len(),
            b.curve().control_points().len()
        );
        for (x, y) in a
            .curve()
            .control_points()
            .iter()
            .zip(b.curve().control_points())
        {
            assert!(x.point().distance_to(y.point()).unwrap() < 1e-12);
            assert!((x.weight() - y.weight()).abs() < 1e-14);
        }
    }
    for (a, b) in source.faces().iter().zip(result.faces()) {
        assert_eq!(a.is_reversed(), b.is_reversed());
        assert_eq!(a.loops().len(), b.loops().len());
        let (x, y) = (a.surface(), b.surface());
        assert_eq!((x.degree_u(), x.degree_v()), (y.degree_u(), y.degree_v()));
        assert_eq!(
            (x.control_point_count_u(), x.control_point_count_v()),
            (y.control_point_count_u(), y.control_point_count_v())
        );
        assert_eq!(x.knots_u(), y.knots_u());
        assert_eq!(x.knots_v(), y.knots_v());
        for (x, y) in x.control_points().iter().zip(y.control_points()) {
            assert!(x.point().distance_to(y.point()).unwrap() < 1e-12);
            assert!((x.weight() - y.weight()).abs() < 1e-14);
        }
        for (a, b) in a.loops().iter().zip(b.loops()) {
            assert_eq!(a.loop_type(), b.loop_type());
            assert_eq!(a.trims().len(), b.trims().len());
            for (a, b) in a.trims().iter().zip(b.trims()) {
                assert_eq!(a.vertices(), b.vertices());
                assert_eq!(a.edge(), b.edge());
                assert_eq!(a.is_reversed_3d(), b.is_reversed_3d());
                assert_eq!(a.trim_type(), b.trim_type());
                assert_eq!(a.iso(), b.iso());
                assert_eq!(a.tolerance(), b.tolerance());
                assert_eq!(a.curve().degree(), b.curve().degree());
                assert_eq!(a.curve().knots(), b.curve().knots());
                assert_eq!(
                    a.curve().control_points().len(),
                    b.curve().control_points().len()
                );
                for (x, y) in a
                    .curve()
                    .control_points()
                    .iter()
                    .zip(b.curve().control_points())
                {
                    let [x1, x2] = x.point().to_array();
                    let [y1, y2] = y.point().to_array();
                    assert!((x1 - y1).abs() < 1e-12 && (x2 - y2).abs() < 1e-12);
                    assert!((x.weight() - y.weight()).abs() < 1e-14);
                }
            }
        }
    }
}

#[test]
fn exact_curved_exports_survive_3dm_and_step_without_display_meshes() {
    let box_solid = Solid::box_solid(p(0., 0., 0.), [10.; 3]).unwrap();
    let cylinder = Solid::cylinder(p(5., 5., -1.), 2., 12.).unwrap();
    let sphere = Solid::sphere(p(5., 5., 10.), 2.).unwrap();
    let through = box_solid
        .boolean(&cylinder, BooleanOperation::Difference)
        .unwrap();
    let pocket = box_solid
        .boolean(&sphere, BooleanOperation::Difference)
        .unwrap();
    for (name, solid) in [
        ("box", &box_solid),
        ("cylinder", &cylinder),
        ("sphere", &sphere),
        ("through_hole", &through),
        ("pocket", &pocket),
    ] {
        let source = solid.to_brep(Tolerance::DEFAULT).unwrap();
        let volume = source.signed_volume(Tolerance::DEFAULT).unwrap();
        let three_dm = round_trip_3dm(&source);
        compare_brep(&source, &three_dm, name);
        let mut bytes = Vec::new();
        let written = viboceros_io::write_step_nurbs_breps(&mut bytes, [&source]);
        if source
            .faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .any(|t| t.edge().is_none())
        {
            assert!(
                matches!(
                    written,
                    Err(viboceros_io::StepError::UnsupportedNativeBrep {
                        reason: "singular UV trim has no STEP edge",
                        ..
                    })
                ),
                "{name}: {written:?}"
            );
            continue;
        }
        written.unwrap();
        let decoded = viboceros_io::read_step_native_instances(
            std::io::Cursor::new(bytes),
            Tolerance::DEFAULT,
        )
        .unwrap_or_else(|error| panic!("{name} STEP decode: {error}"));
        assert_eq!(decoded.instances.len(), 1, "{name} STEP shell count");
        let restored = &decoded.instances[0].brep;
        assert!(restored.is_solid(), "{name} STEP closure");
        let result = restored.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (result - volume).abs() < volume * 1e-9,
            "{name} STEP volume {result} != {volume}"
        );
    }
}
