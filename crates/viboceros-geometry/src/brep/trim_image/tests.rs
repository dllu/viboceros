use super::*;

#[test]
fn local_parameter_frame_retains_composed_points_and_native_trim_derivatives() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let mut source = Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for sign in [1., -1.] {
        let trim = &mut source.faces[0].loops[0].trims[0];
        trim.curve = NurbsCurve2::try_new_rational(
            1,
            trim.curve
                .control_points()
                .iter()
                .zip([sign, 2. * sign])
                .map(|(c, w)| WeightedPoint2::try_new(c.point(), w).unwrap())
                .collect(),
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let translated =
            crate::brep::parameter_frame::tests::translated_face(&source.faces[0], [1e12, -2e12]);
        let original = translated.clone();
        let frame = translated.local_parameter_frame().unwrap();
        let image = LiftedTrim::new(&frame.face.loops[0].trims[0], &frame.face.surface).unwrap();
        for t in [0., 0.1, 0.3, 0.7, 0.9, 1.] {
            let expected = p(2. * t / (1. + t), 0.);
            let (point, derivative) = image.jet(t).unwrap();
            assert!(point.distance_to(expected).unwrap() < 2e-15);
            assert!((derivative.x() - 2. / (1. + t).powi(2)).abs() < 2e-14);
            assert_eq!(derivative.y(), 0.);
            assert_eq!(derivative.z(), 0.);
            assert!(
                image
                    .point(t, ParameterSide::Right)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 2e-15
            );
        }
        // Native UV output necessarily rounds on the large-origin grid. This
        // demonstrates why local composition, not a looser tolerance, is needed.
        let uv = translated.loops[0].trims[0].curve.evaluate(0.3).unwrap();
        let rounded_image = translated.surface.evaluate(uv.x(), uv.y()).unwrap();
        assert!((rounded_image.x() - 0.6 / 1.3).abs() > 1e-6);
        assert_eq!(translated, original);
    }
}
