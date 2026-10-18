use viboceros_geometry::{Frame3, Point3, Tolerance, Vector3};

fn validate(frame: Frame3) {
    let axes = frame.axes().map(|axis| axis.as_vector());
    for axis in axes {
        assert!((axis.length().unwrap() - 1.).abs() < 8. * f64::EPSILON);
    }
    for i in 0..3 {
        for j in 0..i {
            assert!(axes[i].dot(axes[j]).unwrap().abs() < 16. * f64::EPSILON);
        }
    }
    assert!(axes[0].cross(axes[1]).unwrap().dot(axes[2]).unwrap() > 1. - 16. * f64::EPSILON);
}

#[test]
fn validated_direction_frames_retain_x_without_model_distance_admission() {
    let origin = Point3::try_new(1e24, -2e24, 3e24).unwrap();
    let x = Vector3::try_new(1., 2., 3.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let y = Vector3::try_new(-2., 1., 4.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let normal = Vector3::try_new(2., -3., 6.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let mut previous = None;
    for absolute in [1e-24, 1e-7, 0.1, 1., 1e17, 1e100] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-9).unwrap();
        let frames = [
            Frame3::try_from_unit_directions(origin, x, y, tolerance).unwrap(),
            Frame3::try_from_unit_x_and_normal(origin, x, normal, tolerance).unwrap(),
        ];
        for frame in frames {
            assert_eq!(frame.origin(), origin);
            assert_eq!(frame.x_axis(), x);
            validate(frame);
        }
        if let Some(previous) = previous {
            assert_eq!(frames, previous);
        }
        previous = Some(frames);
    }
}

#[test]
fn angular_admission_and_orthogonality_survive_nearly_parallel_unit_directions() {
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let x = Vector3::try_new(1., 1., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let perpendicular = Vector3::try_new(1., -1., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    for absolute in [1e-24, 1e-7, 0.1, 1., 1e17, 1e100] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-9).unwrap();
        for (separation, accepted) in [(0., false), (0.5e-9, false), (2e-9, true), (1e-6, true)] {
            let a = x.as_vector().to_array();
            let b = perpendicular.as_vector().to_array();
            let direction = Vector3::try_from(std::array::from_fn(|i| a[i] + separation * b[i]))
                .unwrap()
                .normalized_nonzero()
                .unwrap();
            for result in [
                Frame3::try_from_unit_directions(origin, x, direction, tolerance),
                Frame3::try_from_unit_x_and_normal(origin, x, direction, tolerance),
            ] {
                assert_eq!(
                    result.is_ok(),
                    accepted,
                    "distance={absolute} separation={separation}"
                );
                if let Ok(frame) = result {
                    assert_eq!(frame.x_axis(), x);
                    validate(frame);
                }
            }
        }
    }
}

#[test]
fn raw_frame_constructors_keep_distance_checks_on_point_displacements() {
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    for absolute in [1e-24, 1e-7, 0.1, 1., 1e17, 1e100] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-9).unwrap();
        let x = Vector3::try_new(3. * absolute, 0., 0.).unwrap();
        let y = Vector3::try_new(0., 3. * absolute, 0.).unwrap();
        let short = Vector3::try_new(0.5 * absolute, 0., 0.).unwrap();
        assert!(Frame3::try_from_directions(origin, short, y, tolerance).is_err());
        assert!(Frame3::try_from_directions(origin, y, short, tolerance).is_err());
        assert!(
            Frame3::try_from_x_and_normal(
                origin,
                x,
                Vector3::try_new(3. * absolute, 0., 0.5 * absolute).unwrap(),
                tolerance
            )
            .is_err()
        );
        validate(Frame3::try_from_directions(origin, x, y, tolerance).unwrap());
    }
}
