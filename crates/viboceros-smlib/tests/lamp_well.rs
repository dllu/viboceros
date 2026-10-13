#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, Ellipse3, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::{BooleanOperation, Solid};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::try_new(x, y, z).unwrap()
}

fn ruled(a: &viboceros_geometry::NurbsCurve, b: &viboceros_geometry::NurbsCurve) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        a.degree(),
        1,
        a.control_points().len(),
        2,
        a.control_points()
            .iter()
            .chain(b.control_points())
            .copied()
            .collect(),
        a.knots().to_vec(),
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
// Degree 5 x 3 front fender master and 7 mm Greville normal control-net
// thickness from user-supplied LMP2-Evo-V3-Source-and-Validation/lmp2_v3.rs.
// The retained reproduction contains only this panel and one rational cutter.
fn crown() -> Brep {
    let side = 1.;
    let zs = [0.29, 0.64, 0.80, 0.78, 0.62, 0.52];
    let corner = [0.24, 0.12, 0.035, 0., 0., 0.];
    let shift = [0.23, 0.13, 0.04, 0., 0., 0.];
    let mut points = Vec::new();
    for (y, z, a) in [
        (0.46, -0.09, 0.),
        (0.985, 0.08, 0.),
        (0.985, 0.04, 0.55),
        (0.985, 0., 1.),
    ] {
        for i in 0..6 {
            points.push(p(
                -2.32 + 1.67 * i as f64 / 5. + shift[i] + a * corner[i],
                side * y,
                zs[i] + z,
            ));
        }
    }
    let outer = NurbsSurface::try_new(
        5,
        3,
        6,
        4,
        points,
        vec![0., 0., 0., 0., 0., 0., 1., 1., 1., 1., 1., 1.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let mut controls = Vec::new();
    for j in 0..4 {
        for i in 0..6 {
            let (_, du, dv) = outer
                .evaluate_with_derivatives(i as f64 / 5., j as f64 / 3.)
                .unwrap();
            let normal = du
                .cross(dv)
                .unwrap()
                .normalized(Tolerance::DEFAULT)
                .unwrap()
                .as_vector();
            let cp = outer.control_point(i, j).unwrap();
            controls.push(
                WeightedPoint3::try_new(
                    cp.point()
                        .translated(normal.scaled(-0.007).unwrap())
                        .unwrap(),
                    cp.weight(),
                )
                .unwrap(),
            );
        }
    }
    let inner = NurbsSurface::try_new_rational(
        5,
        3,
        6,
        4,
        controls,
        outer.knots_u().to_vec(),
        outer.knots_v().to_vec(),
    )
    .unwrap();
    let mut faces = vec![
        Brep::try_surface_face(outer.clone(), Tolerance::DEFAULT).unwrap(),
        Brep::try_surface_face(inner.clone(), Tolerance::DEFAULT).unwrap(),
    ];
    for u in [0., 1.] {
        faces.push(
            Brep::try_surface_face(
                ruled(&outer.isocurve_v(u).unwrap(), &inner.isocurve_v(u).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    for v in [0., 1.] {
        faces.push(
            Brep::try_surface_face(
                ruled(&outer.isocurve_u(v).unwrap(), &inner.isocurve_u(v).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    let mut joined =
        viboceros_geometry::join_breps(&faces.iter().collect::<Vec<_>>(), 1e-7, Tolerance::DEFAULT)
            .unwrap();
    assert_eq!(joined.len(), 1);
    let mut crown = joined.remove(0).brep;
    if crown.signed_volume(Tolerance::DEFAULT).unwrap() < 0. {
        crown.reverse_orientation();
    }
    crown
}
fn ellipse_cutter(crown: &Brep, radii: [f64; 2], angle: f64) -> Brep {
    let tolerance = Tolerance::DEFAULT;
    let (center, du, dv) = crown.faces()[0]
        .surface()
        .evaluate_with_derivatives(0.13, 0.30)
        .unwrap();
    let normal = du
        .cross(dv)
        .unwrap()
        .normalized(tolerance)
        .unwrap()
        .as_vector();
    let x = du.normalized(tolerance).unwrap().as_vector();
    let y = normal.cross(x).unwrap();
    let xx = v(
        x.x() * angle.cos() + y.x() * angle.sin(),
        x.y() * angle.cos() + y.y() * angle.sin(),
        x.z() * angle.cos() + y.z() * angle.sin(),
    )
    .normalized(tolerance)
    .unwrap();
    let yy = v(
        -x.x() * angle.sin() + y.x() * angle.cos(),
        -x.y() * angle.sin() + y.y() * angle.cos(),
        -x.z() * angle.sin() + y.z() * angle.cos(),
    )
    .normalized(tolerance)
    .unwrap();
    let center = center.translated(normal.scaled(0.2).unwrap()).unwrap();
    let ellipse = Ellipse3::try_new(center, radii[0], radii[1], xx, yy, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    Brep::try_extruded_curve(
        &ellipse,
        v(0., 0., 0.),
        normal.scaled(-0.4).unwrap(),
        tolerance,
    )
    .unwrap()
}

fn lamp_boolean_roundtrip(radii: [f64; 2], angles: &[f64]) {
    use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};
    let tolerance = Tolerance::DEFAULT;
    let crown = crown();
    let native = Solid::from_brep(&crown, tolerance).unwrap();
    let before = native.properties(1e-8).unwrap();
    for &angle in angles {
        let cutter = ellipse_cutter(&crown, radii, angle);
        let cutter = Solid::from_brep(&cutter, tolerance).unwrap();
        for postprocess in [true, false] {
            let result = native
                .boolean_with_postprocess(&cutter, BooleanOperation::Difference, postprocess)
                .unwrap();
            assert!(result.properties(1e-8).unwrap().manifold);
            // Query native endpoints before any copy, NURBS conversion, or
            // UV generation. A portable tolerance change cannot pass this.
            result
                .validate_native_endpoints(Tolerance::NUMERICAL_VALIDATION)
                .unwrap();
            let brep = result.to_brep(tolerance).unwrap();
            assert!(brep.is_solid());
            let volume = result.volume(1e-8).unwrap();
            assert!(volume > 0. && volume < before.volume);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("lamp-well.3dm");
            let model = ThreeDmModel::new(
                vec![ThreeDmLayer {
                    name: "Default".into(),
                    color: [0, 0, 0],
                    visible: true,
                    locked: false,
                }],
                vec![],
                vec![ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0)],
            );
            viboceros_io::write_3dm_file(&path, &model).unwrap();
            let restored = viboceros_io::read_3dm_file(&path, tolerance).unwrap();
            let ThreeDmGeometry::Brep(restored) = &restored.objects[0].geometry else {
                panic!("portable B-rep expected")
            };
            assert!(restored.is_solid());
            assert_eq!(restored.vertices().len(), brep.vertices().len());
            assert_eq!(restored.edges().len(), brep.edges().len());
            assert_eq!(restored.faces().len(), brep.faces().len());
            for (source, restored) in brep.edges().iter().zip(restored.edges()) {
                assert_eq!(source.vertices(), restored.vertices());
                for f in [0., 0.25, 0.5, 0.75, 1.] {
                    let a = source
                        .curve()
                        .evaluate(source.curve().parameter_at(f).unwrap())
                        .unwrap();
                    let b = restored
                        .curve()
                        .evaluate(restored.curve().parameter_at(f).unwrap())
                        .unwrap();
                    assert!(a.distance_to(b).unwrap() < 1e-12);
                }
            }
        }
    }
    assert_eq!(native.properties(1e-8).unwrap(), before);
}

#[test]
fn front_fender_lamp_boolean_survives_portable_and_3dm_roundtrip() {
    lamp_boolean_roundtrip([0.16, 0.085], &[0., 0.5]);
}

// Reconstructed from the surviving source master, not the lost original
// body-boolean-ellipse.rs experiment. On unpatched usd-brep, the larger cutter
// produced native endpoint gaps up to 4.7 mm against ~1 nm zone radii.
#[test]
fn boundary_crossing_lamp_cut_preserves_native_endpoints_and_3dm_topology() {
    lamp_boolean_roundtrip([0.30, 0.16], &[0.3]);
}
