use super::*;

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}

#[test]
fn scale_nu_signed_zero_cplane_world_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Point(point([2., 3., 4.])))
        .unwrap();
    document.select_all();
    let context = CommandContext {
        construction_plane: Frame3::try_from_directions(
            point([5., -4., 2.]),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Vector3::try_new(-1., 0., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    };
    registry
        .execute_in_context(&mut document, "ScaleNU 1,2,3 2 3 .5", context)
        .unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Point(point([4., 4., 3.5]))
    );
    document.undo().unwrap();
    registry
        .execute_in_context(
            &mut document,
            "ScaleNU 1,2,3 2 -1 .5 WorldCoordinates",
            context,
        )
        .unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Point(point([3., 1., 3.5]))
    );
    document.undo().unwrap();
    registry
        .execute_in_context(
            &mut document,
            "ScaleNU 0,0,0 0 1 1 WorldCoordinates",
            context,
        )
        .unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Point(point([0., 3., 4.]))
    );
    assert_eq!(registry.axis_scale_defaults("ScaleNU"), Some([0., 1., 1.]));
}

#[test]
fn scale_nu_references_and_invalid_inputs_preserve_geometry_and_defaults() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Point(point([2., 3., 4.])))
        .unwrap();
    document.select_all();
    registry
        .execute(&mut document, "ScaleNU 0,0,0 2,0,0 6,0,0 1 1")
        .unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Point(point([6., 3., 4.]))
    );
    assert_eq!(registry.axis_scale_defaults("ScaleNU"), Some([3., 1., 1.]));
    for input in [
        "ScaleNU 0,0,0 2 NaN 1",
        "ScaleNU 0,0,0 2 1 inf",
        "ScaleNU 0,0,0 2 1",
        "ScaleNU 0,0,0 2,0,0 6 1 1",
        "ScaleNU 0,0,0 0,2,0 6,0,0 1 1",
        "ScaleNU 0,0,0 2 1 1 Rigid=Maybe",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Point(point([6., 3., 4.]))
        );
        assert_eq!(registry.axis_scale_defaults("ScaleNU"), Some([3., 1., 1.]));
    }
    assert!(!registry.remember_axis_scale("ScaleNU", 3, 2.));
    assert!(!registry.remember_axis_scale("ScaleNU", 0, Real::NAN));
    assert_eq!(
        CommandRegistry::with_builtins().axis_scale_defaults("ScaleNU"),
        Some([1.; 3])
    );
}

#[test]
fn scale_nu_large_projected_references_and_zero_targets() {
    let plane = CommandContext::default().construction_plane;
    let origin = point([-Real::MAX, 0., 0.]);
    let reference = point([0., 0., 0.]);
    assert_eq!(
        reference_factor(plane, origin, 0, reference, origin, Tolerance::DEFAULT).unwrap(),
        0.
    );
    assert_eq!(
        reference_factor(
            plane,
            origin,
            0,
            reference,
            point([0., Real::MAX, Real::MAX]),
            Tolerance::DEFAULT
        )
        .unwrap(),
        1.
    );
    assert!(reference_factor(plane, origin, 3, reference, reference, Tolerance::DEFAULT).is_err());
    assert_eq!(
        scale_map(plane, origin, [1.; 3]).unwrap(),
        AffineTransform3::identity()
    );
}
