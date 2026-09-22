use super::*;

#[test]
fn stationary_samples_do_not_exclude_a_regular_distance_witness() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let surface = NurbsSurface::try_bilinear([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)]).unwrap();
    let uv = |x| Point2::try_new(x, 0.).unwrap();
    for gauge in [1., -1.] {
        let curve = NurbsCurve2::try_new_rational(
            2,
            [0., 0., 1., 1., 1.]
                .into_iter()
                .zip([1., 2., 1., 2., 1.])
                .map(|(x, w)| WeightedPoint2::try_new(uv(x), gauge * w).unwrap())
                .collect(),
            vec![-3., -3., -3., 2., 2., 7., 7., 7.],
        )
        .unwrap();
        let trim = BrepTrim::try_new(
            [0, 1],
            Some(0),
            false,
            curve,
            BrepTrimType::Boundary,
            SurfaceIso::South,
            [0.; 2],
        )
        .unwrap();
        let image = LiftedTrim::new(&trim, &surface).unwrap();
        // More than sixteen equally near samples on the constant second span.
        // None can move because its tangent is zero. The regular first-span
        // seed is farther away, but its image contains every target X in [0,1].
        let mut samples = (0..=64)
            .map(|i| (2. + 5. * i as Real / 64., ParameterSide::Right, p(1., 0.)))
            .collect::<Vec<_>>();
        samples.push((
            -0.5,
            ParameterSide::Right,
            image.point(-0.5, ParameterSide::Right).unwrap(),
        ));
        for reverse in [false, true] {
            if reverse {
                samples.reverse();
            }
            let target = p(0.99, 0.);
            let (distance, parameter) = image.distance_witness(target, &samples, 1e-12).unwrap();
            // In the active span x=s²/(1+2s-2s²), s=(t+3)/5.
            let q: Real = 0.99;
            let expected = -3. + 5. * (q + (3. * q * q + q).sqrt()) / (1. + 2. * q);
            assert!(
                distance <= 1e-12,
                "distance={distance}, parameter={parameter}"
            );
            assert!((parameter - expected).abs() <= 1e-10);
            assert_eq!(
                distance,
                image
                    .point(parameter, ParameterSide::Right)
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
            );
            let (miss, _) = image
                .distance_witness(p(0.99, 1e-4), &samples, 1e-12)
                .unwrap();
            assert!(miss >= 1e-4);
        }
    }
}

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
            let (point, derivative) = image.jet(t, ParameterSide::Right).unwrap();
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
