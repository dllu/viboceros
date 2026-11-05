use viboceros_geometry::{AffineTransform3, Brep, Circle3, Frame3, Point3, Tolerance, Vector3};

#[test]
fn runtime_extrusion_caps_preserve_trim_images_topology_and_native_archive() {
    let tolerance = Tolerance::DEFAULT;
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let frame = Frame3::try_from_directions(
        origin,
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        tolerance,
    )
    .unwrap();
    let circle = Circle3::try_new(
        origin,
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        tolerance,
    )
    .unwrap();
    for height in [-3., 3.] {
        let initial = Brep::try_planar_profile_extrusion(
            &circle.to_nurbs().unwrap(),
            frame,
            height,
            true,
            tolerance,
        )
        .unwrap();
        let transform = AffineTransform3::try_new(
            [[1., 0.25, 0.1], [0., 1., 0.3], [0., 0., 1.]],
            Vector3::try_new(10., -4., 3.).unwrap(),
        )
        .unwrap();
        let source = initial.transformed(transform, tolerance).unwrap();
        let before = source.clone();
        let archive = source.extrusion().unwrap().archive().to_vec();
        let result = source.try_extrusion_brep_form(tolerance).unwrap();
        assert_eq!(source, before);
        assert_eq!(source.extrusion().unwrap().archive(), archive);
        assert!(!result.is_extrusion());
        assert_eq!(result.vertices(), source.vertices());
        assert_eq!(result.edges(), source.edges());
        assert_eq!(result.faces().len(), source.faces().len());
        assert_ne!(result.faces()[1].surface(), source.faces()[1].surface());
        for (a, b) in source.faces().iter().zip(result.faces()) {
            assert_eq!(a.loops(), b.loops());
            assert_eq!(a.is_reversed(), b.is_reversed());
        }
        for (a, b) in source.faces().iter().zip(result.faces()).skip(1) {
            for trim in a.loops().iter().flat_map(|l| l.trims()) {
                for i in 1..16 {
                    let uv = trim
                        .curve()
                        .evaluate(trim.curve().parameter_at(i as f64 / 16.).unwrap())
                        .unwrap();
                    let p = a.surface().evaluate(uv.x(), uv.y()).unwrap();
                    let q = b.surface().evaluate(uv.x(), uv.y()).unwrap();
                    assert!(p.distance_to(q).unwrap() < 1e-12);
                }
            }
        }
        assert!(result.try_extrusion_brep_form(tolerance).is_err());
    }
}
