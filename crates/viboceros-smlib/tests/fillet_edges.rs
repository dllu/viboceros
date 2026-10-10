#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_smlib::Solid;
fn box_() -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(frame, [[0., 10.]; 3], Tolerance::DEFAULT).unwrap()
}
#[test]
fn invalid_or_oversized_fillet_requests_leave_the_source_unchanged() {
    let source = box_();
    let before = source.clone();
    for (edges, radius) in [
        (vec![], 1.),
        (vec![999], 1.),
        (vec![0], 0.),
        (vec![0], f64::INFINITY),
        ((0..source.edges().len()).collect(), 6.),
    ] {
        let output = Solid::fillet_brep_edges(&source, &edges, radius, Tolerance::DEFAULT)
            .and_then(|solid| solid.to_brep(Tolerance::DEFAULT));
        assert!(
            output.is_err(),
            "radius {radius}, output: {:?}",
            output
                .as_ref()
                .map(|b| (b.faces().len(), b.signed_volume(Tolerance::DEFAULT)))
        );
        assert_eq!(source, before);
    }
}
#[test]
fn single_original_edge_rounds_with_analytic_volume_and_source_preservation() {
    let source = box_();
    let before = source.clone();
    for index in 0..source.edges().len() {
        let native =
            Solid::fillet_brep_edges(&source, &[index, index], 1., Tolerance::DEFAULT).unwrap();
        let result = native.to_brep(Tolerance::DEFAULT).unwrap();
        assert!(result.is_solid());
        let expected = 1000. - (1. - std::f64::consts::PI / 4.) * 10.;
        let volume = result.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (volume - expected).abs() < 1e-7,
            "edge {index}: Rust {volume:.15}, native {:.15}, expected {expected:.15}",
            native.properties(1e-8).unwrap().volume
        );
        let endpoints = source.edges()[index]
            .vertices()
            .map(|v| source.vertices()[v].point());
        // The original sharp midpoint must be absent from the result's boundary.
        let midpoint = Point3::try_new(
            (endpoints[0].x() + endpoints[1].x()) / 2.,
            (endpoints[0].y() + endpoints[1].y()) / 2.,
            (endpoints[0].z() + endpoints[1].z()) / 2.,
        )
        .unwrap();
        assert!(result.edges().iter().all(|e| {
            e.curve()
                .evaluate(
                    e.curve()
                        .closest_parameter(midpoint, Tolerance::DEFAULT)
                        .unwrap(),
                )
                .unwrap()
                .distance_to(midpoint)
                .unwrap()
                > 0.1
        }));
        assert_eq!(source, before);
    }
}
#[test]
fn all_edges_transfer_as_a_closed_rounded_box() {
    let source = box_();
    let edges = (0..source.edges().len()).collect::<Vec<_>>();
    let result = Solid::fillet_brep_edges(&source, &edges, 1., Tolerance::DEFAULT)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert!(result.is_solid());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rounded-box.3dm");
    let model = viboceros_io::ThreeDmModel::new(
        vec![viboceros_io::ThreeDmLayer {
            name: "Default".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![viboceros_io::ThreeDmObject::new(
            viboceros_io::ThreeDmGeometry::Brep(result.clone()),
            0,
        )],
    );
    viboceros_io::write_3dm_file(&path, &model).unwrap();
    let restored = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    let viboceros_io::ThreeDmGeometry::Brep(brep) = &restored.objects[0].geometry else {
        panic!("editable fillet expected")
    };
    assert!(brep.is_solid());
    assert_eq!(brep.faces().len(), result.faces().len());
    // Rounded cube: central cube, six face slabs, twelve quarter-cylinders,
    // eight spherical octants, with side length 10 and radius 1.
    let expected = 8_f64.powi(3)
        + 6. * 8_f64.powi(2)
        + 3. * std::f64::consts::PI * 8.
        + 4. * std::f64::consts::PI / 3.;
    let volume = result.signed_volume(Tolerance::DEFAULT).unwrap();
    let volume_epsilon = Tolerance::DEFAULT.absolute() * 6. * 10_f64.powi(2);
    assert!(
        (volume - expected).abs() < volume_epsilon,
        "Rust {volume:.15}, expected {expected:.15}"
    );
    let boundary_error = |p: Point3| {
        let squared = p
            .to_array()
            .into_iter()
            .map(|v| (1. - v).max(0.).max(v - 9.).powi(2))
            .sum::<f64>();
        (squared.sqrt() - 1.).abs()
    };
    for edge in result.edges() {
        let d = edge.curve().domain();
        for i in 0..=32 {
            let p = edge
                .curve()
                .evaluate(d.start() + (d.end() - d.start()) * i as f64 / 32.)
                .unwrap();
            assert!(
                boundary_error(p) < 1e-8,
                "spatial edge departs rounded-cube boundary: {p:?}"
            );
        }
    }
    for face in result.faces() {
        let u = face.surface().domain_u();
        let v = face.surface().domain_v();
        for i in 1..8 {
            for j in 1..8 {
                let a = u.start() + (u.end() - u.start()) * i as f64 / 8.;
                let b = v.start() + (v.end() - v.start()) * j as f64 / 8.;
                if face.contains_parameters(a, b, Tolerance::DEFAULT).unwrap() {
                    let p = face.surface().evaluate(a, b).unwrap();
                    assert!(
                        boundary_error(p) < 1e-8,
                        "trimmed surface departs rounded-cube boundary: {p:?}"
                    );
                }
            }
        }
        for trim in face.loops().iter().flat_map(|l| l.trims()) {
            let d = trim.curve().domain();
            for i in 0..=32 {
                let uv = trim
                    .curve()
                    .evaluate(d.start() + (d.end() - d.start()) * i as f64 / 32.)
                    .unwrap();
                let p = face.surface().evaluate(uv.x(), uv.y()).unwrap();
                assert!(
                    boundary_error(p) < 1e-8,
                    "lifted trim departs rounded-cube boundary: {p:?}"
                );
            }
        }
    }
}
