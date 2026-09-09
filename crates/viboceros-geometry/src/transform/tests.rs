use super::*;

#[test]
fn construction_preserves_row_major_coefficients_and_checks_every_entry() {
    let rows = [[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]];
    let translation = Vector3::try_new(10., 11., 12.).unwrap();
    let transform = AffineTransform3::try_new(rows, translation).unwrap();
    assert_eq!(transform.linear_rows(), rows);
    assert_eq!(transform.translation(), translation);
    for column in 0..3 {
        let mut basis = [0.; 3];
        basis[column] = 1.;
        assert_eq!(
            transform
                .transform_vector(Vector3::try_from(basis).unwrap())
                .unwrap()
                .to_array(),
            rows.map(|row| row[column])
        );
        for row in 0..3 {
            for invalid in [Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
                let mut invalid_rows = rows;
                invalid_rows[row][column] = invalid;
                assert!(AffineTransform3::try_new(invalid_rows, translation).is_err());
            }
        }
    }
}

#[test]
fn centered_scale_retains_finite_translation_after_linear_overflow() {
    let huge = 2_f64.powi(1023);
    for sign in [-1., 1.] {
        let center = point(sign * huge, 0., 0.);
        let scale = AffineTransform3::try_uniform_scale(center, 2.).unwrap();
        assert_eq!(scale.translation().to_array(), [-sign * huge, 0., 0.]);
        assert_eq!(scale.transform_point(center).unwrap(), center);
        assert_eq!(
            scale
                .transform_point(point(sign * huge * 0.5, 0., 0.))
                .unwrap(),
            point(0., 0., 0.)
        );
        assert!(AffineTransform3::try_uniform_scale(center, 4.).is_err());
    }
}
use crate::Tolerance;

#[test]
fn origin_mapping_retains_small_translation_after_cancellation() {
    let large = 2_f64.powi(100);
    let source = point(large, 1., 0.);
    let target = point(large, 0., 0.);
    let transform = AffineTransform3::try_mapping_origins(
        [[1., 1., 0.], [0., 1., 0.], [0., 0., 1.]],
        source,
        target,
    )
    .unwrap();
    assert_eq!(transform.translation().to_array(), [-1., -1., 0.]);
    assert_eq!(transform.transform_point(source).unwrap(), target);
}

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn identity_and_translation_distinguish_points_from_vectors() {
    let original = point(1.0, 2.0, 3.0);
    assert_eq!(
        AffineTransform3::identity()
            .transform_point(original)
            .unwrap(),
        original
    );

    let offset = Vector3::try_new(4.0, -5.0, 6.0).unwrap();
    let transform = AffineTransform3::from_translation(offset);
    assert_eq!(
        transform.transform_point(original).unwrap(),
        point(5.0, -3.0, 9.0)
    );
    assert_eq!(transform.transform_vector(offset).unwrap(), offset);
}

#[test]
fn applies_a_general_finite_linear_part() {
    let transform = AffineTransform3::try_new(
        [[0.0, -2.0, 0.0], [3.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
        Vector3::try_new(10.0, 20.0, 30.0).unwrap(),
    )
    .unwrap();
    assert_eq!(
        transform.transform_point(point(1.0, 2.0, 3.0)).unwrap(),
        point(6.0, 23.0, 42.0)
    );
    assert_eq!(
        transform
            .transform_vector(Vector3::try_new(1.0, 2.0, 3.0).unwrap())
            .unwrap(),
        Vector3::try_new(-4.0, 3.0, 12.0).unwrap()
    );
}

#[test]
fn composition_matches_sequential_integer_maps_in_application_order() {
    let zero = Vector3::try_new(0., 0., 0.).unwrap();
    let maps = [
        AffineTransform3::identity(),
        AffineTransform3::from_translation(Vector3::try_new(2., -3., 5.).unwrap()),
        AffineTransform3::try_new([[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]], zero).unwrap(),
        AffineTransform3::try_new(
            [[2., 1., 0.], [0., -3., 1.], [0., 0., 0.]],
            Vector3::try_new(1., 2., 3.).unwrap(),
        )
        .unwrap(),
    ];
    for first in maps {
        for second in maps {
            let composed = first.then(second).unwrap();
            for p in [point(0., 0., 0.), point(1., 2., 3.), point(-3., 2., -7.)] {
                assert_eq!(
                    composed.transform_point(p).unwrap(),
                    second
                        .transform_point(first.transform_point(p).unwrap())
                        .unwrap()
                );
                let v = Vector3::try_from(p.to_array()).unwrap();
                assert_eq!(
                    composed.transform_vector(v).unwrap(),
                    second
                        .transform_vector(first.transform_vector(v).unwrap())
                        .unwrap()
                );
            }
            for third in maps {
                assert_eq!(
                    composed.then(third).unwrap(),
                    first.then(second.then(third).unwrap()).unwrap()
                );
            }
        }
    }
    assert_ne!(
        maps[1].then(maps[2]).unwrap(),
        maps[2].then(maps[1]).unwrap()
    );
}

#[test]
fn composition_recovers_overflowing_products_and_checks_final_coefficients() {
    let huge = 2_f64.powi(1023);
    let first = AffineTransform3::try_new(
        [[huge, 0., 0.], [huge, 0., 0.], [0., 0., 0.]],
        Vector3::try_new(huge, huge, 0.).unwrap(),
    )
    .unwrap();
    let second = AffineTransform3::try_new(
        [[2., -2., 0.], [0., 0., 0.], [0., 0., 0.]],
        Vector3::try_new(1., 2., 3.).unwrap(),
    )
    .unwrap();
    assert_eq!(
        first.then(second).unwrap(),
        AffineTransform3::try_new([[0.; 3]; 3], Vector3::try_new(1., 2., 3.).unwrap()).unwrap()
    );
    let doubled = AffineTransform3::try_new(
        [[2., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(-huge, 0., 0.).unwrap(),
    )
    .unwrap();
    let translated = AffineTransform3::from_translation(Vector3::try_new(huge, 0., 0.).unwrap());
    assert_eq!(translated.then(doubled).unwrap().translation().x(), huge);
    assert!(first.then(doubled).is_err());
    assert!(translated.then(translated).is_err());
}

#[test]
fn point_transform_includes_translation_before_rounding_or_overflow() {
    let huge = 2_f64.powi(1023);
    let transform = AffineTransform3::try_new(
        [[2., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(-huge, 0., 0.).unwrap(),
    )
    .unwrap();
    assert_eq!(
        transform.transform_point(point(huge, 2., 3.)).unwrap(),
        point(huge, 2., 3.)
    );
    assert!(
        transform
            .transform_vector(Vector3::try_new(huge, 2., 3.).unwrap())
            .is_err()
    );
    let cancellation = AffineTransform3::try_new(
        [[1., 1., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(-huge, 0., 0.).unwrap(),
    )
    .unwrap();
    for small in [1., -1., Real::MIN_POSITIVE, Real::from_bits(1)] {
        assert_eq!(
            cancellation
                .transform_point(point(huge, small, 0.))
                .unwrap(),
            point(small, small, 0.)
        );
    }
}

#[test]
fn scaled_dot_product_preserves_large_cancellation() {
    let transform = AffineTransform3::try_new(
        [[1.0, 1.0, -1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    let transformed = transform
        .transform_point(point(Real::MAX, Real::MAX, Real::MAX))
        .unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(transformed.x(), Real::MAX));
    assert_eq!(transformed.y(), Real::MAX);
    assert_eq!(transformed.z(), Real::MAX);
}

#[test]
fn rejects_non_finite_coefficients_and_outputs() {
    assert!(
        AffineTransform3::try_new(
            [[Real::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .is_err()
    );
    let translation =
        AffineTransform3::from_translation(Vector3::try_new(Real::MAX, 0.0, 0.0).unwrap());
    assert!(
        translation
            .transform_point(point(Real::MAX, 0.0, 0.0))
            .is_err()
    );
}

#[test]
fn centered_uniform_scale_keeps_its_fixed_point() {
    let center = point(1.0, 2.0, 3.0);
    let transform = AffineTransform3::try_uniform_scale(center, 2.0).unwrap();
    assert_eq!(transform.transform_point(center).unwrap(), center);
    assert_eq!(
        transform.transform_point(point(2.0, 4.0, 6.0)).unwrap(),
        point(3.0, 6.0, 9.0)
    );
    assert!(AffineTransform3::try_uniform_scale(center, Real::NAN).is_err());
}

#[test]
fn nonuniform_and_directional_scales_retain_their_fixed_point() {
    let center = point(1.0, 2.0, 3.0);
    let nonuniform = AffineTransform3::try_nonuniform_scale(center, [-2.0, 0.5, 0.0]).unwrap();
    assert_eq!(nonuniform.transform_point(center).unwrap(), center);
    assert_eq!(
        nonuniform.transform_point(point(2.0, 4.0, 6.0)).unwrap(),
        point(-1.0, 3.0, 3.0)
    );

    let direction = UnitVector3::try_new(1.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
    let directional = AffineTransform3::try_directional_scale(center, direction, 3.0).unwrap();
    // Direction normalization and the stored translation are rounded.
    // Applying their affine coefficients with one compensated sum need
    // not reproduce the center bit-for-bit (two-stage rounding hid this).
    assert!(
        directional
            .transform_point(center)
            .unwrap()
            .distance_to(center)
            .unwrap()
            <= 4. * Real::EPSILON
    );
    assert!(
        directional
            .transform_point(point(2.0, 3.0, 4.0))
            .unwrap()
            .is_near(point(4.0, 5.0, 4.0), Tolerance::DEFAULT)
    );
    assert!(
        directional
            .transform_point(point(2.0, 1.0, 4.0))
            .unwrap()
            .is_near(point(2.0, 1.0, 4.0), Tolerance::DEFAULT)
    );

    let flattened = AffineTransform3::try_directional_scale(center, direction, 0.0).unwrap();
    assert!(
        flattened
            .transform_point(point(2.0, 3.0, 4.0))
            .unwrap()
            .is_near(point(1.0, 2.0, 4.0), Tolerance::DEFAULT)
    );
    assert!(AffineTransform3::try_nonuniform_scale(center, [1.0, Real::NAN, 1.0]).is_err());
    assert!(AffineTransform3::try_directional_scale(center, direction, Real::INFINITY).is_err());
}

#[test]
fn shear_uses_perpendicular_directions_and_retains_its_fixed_point() {
    let fixed = point(1.0, 2.0, 3.0);
    let reference = UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
    let shear_direction = UnitVector3::try_new(-1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
    let transform =
        AffineTransform3::try_shear(fixed, reference, shear_direction, 0.5, Tolerance::DEFAULT)
            .unwrap();

    assert_eq!(transform.transform_point(fixed).unwrap(), fixed);
    assert_eq!(
        transform.transform_point(point(4.0, -2.0, 5.0)).unwrap(),
        point(6.0, -2.0, 5.0)
    );
    assert_eq!(
        transform.transform_point(point(-3.0, 2.0, 7.0)).unwrap(),
        point(-3.0, 2.0, 7.0)
    );

    assert!(
        AffineTransform3::try_shear(fixed, reference, reference, 0.5, Tolerance::DEFAULT,).is_err()
    );
    assert!(
        AffineTransform3::try_shear(
            fixed,
            reference,
            shear_direction,
            Real::NAN,
            Tolerance::DEFAULT,
        )
        .is_err()
    );
}

#[test]
fn planar_projection_retains_tangent_coordinates_and_plane_points() {
    let origin = point(1.0, 2.0, 3.0);
    let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
    let projection = AffineTransform3::try_planar_projection(Plane::new(origin, normal)).unwrap();

    assert_eq!(
        projection.transform_point(point(4.0, -5.0, 9.0)).unwrap(),
        point(4.0, -5.0, 3.0)
    );
    assert_eq!(
        projection.transform_point(point(-7.0, 8.0, 3.0)).unwrap(),
        point(-7.0, 8.0, 3.0)
    );
}

#[test]
fn axis_rotation_uses_rodrigues_formula_about_a_fixed_point() {
    let center = point(1.0, 1.0, 0.0);
    let axis = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
    let transform =
        AffineTransform3::try_rotation(center, axis, std::f64::consts::FRAC_PI_2).unwrap();
    assert!(
        transform
            .transform_point(center)
            .unwrap()
            .is_near(center, Tolerance::DEFAULT)
    );
    assert!(
        transform
            .transform_point(point(2.0, 1.0, 0.0))
            .unwrap()
            .is_near(point(1.0, 2.0, 0.0), Tolerance::DEFAULT)
    );
    assert!(AffineTransform3::try_rotation(center, axis, Real::INFINITY).is_err());
}

#[test]
fn shortest_rotation_maps_parallel_oblique_and_antiparallel_directions() {
    let directions = [
        (
            UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
        ),
        (
            UnitVector3::try_new(1.0, 2.0, 3.0, Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(-2.0, 4.0, 1.0, Tolerance::DEFAULT).unwrap(),
        ),
        (
            UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(0.0, -1.0, 0.0, Tolerance::DEFAULT).unwrap(),
        ),
        (
            UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(-1.0, 1.0e-8, 0.0, Tolerance::DEFAULT).unwrap(),
        ),
    ];
    for (from, to) in directions {
        let rotation =
            AffineTransform3::try_rotation_between(from, to, Tolerance::DEFAULT).unwrap();
        let actual = rotation.transform_vector(from.as_vector()).unwrap();
        for (actual, expected) in actual.to_array().into_iter().zip(to.as_vector().to_array()) {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
        let rows = rotation.linear_rows();
        for row in 0..3 {
            for column in 0..3 {
                let dot = (0..3)
                    .map(|index| rows[row][index] * rows[column][index])
                    .sum::<Real>();
                let expected = if row == column { 1.0 } else { 0.0 };
                assert!(Tolerance::DEFAULT.approx_eq(dot, expected));
            }
        }
    }
}

#[test]
fn direction_mapping_scales_axially_or_uniformly_around_the_source_origin() {
    let source_origin = point(1.0, 2.0, 3.0);
    let target_origin = point(10.0, -1.0, 4.0);
    let source_direction = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
    let target_direction = UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
    let axial = AffineTransform3::try_direction_mapping(
        source_origin,
        source_direction,
        target_origin,
        target_direction,
        3.0,
        1.0,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (source, expected) in [
        (source_origin, target_origin),
        (point(2.0, 2.0, 3.0), point(10.0, 2.0, 4.0)),
        (point(1.0, 3.0, 3.0), point(9.0, -1.0, 4.0)),
        (point(1.0, 2.0, 4.0), point(10.0, -1.0, 5.0)),
    ] {
        assert!(
            axial
                .transform_point(source)
                .unwrap()
                .is_near(expected, Tolerance::DEFAULT)
        );
    }

    let uniform = AffineTransform3::try_direction_mapping(
        source_origin,
        source_direction,
        target_origin,
        target_direction,
        3.0,
        3.0,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(
        uniform
            .transform_point(point(1.0, 3.0, 4.0))
            .unwrap()
            .is_near(point(7.0, -1.0, 7.0), Tolerance::DEFAULT)
    );
}

#[test]
fn frame_mapping_matches_origins_axes_and_uniform_scale() {
    let source = Frame3::try_from_points(
        point(1.0, 2.0, 3.0),
        point(3.0, 2.0, 3.0),
        point(1.0, 3.0, 4.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let target = Frame3::try_from_points(
        point(10.0, -1.0, 4.0),
        point(10.0, 5.0, 4.0),
        point(8.0, -1.0, 8.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let transform = AffineTransform3::try_frame_mapping(source, target, [3.0, 3.0, 3.0]).unwrap();
    assert!(
        transform
            .transform_point(source.origin())
            .unwrap()
            .is_near(target.origin(), Tolerance::DEFAULT)
    );
    for (source_axis, target_axis) in source.axes().into_iter().zip(target.axes()) {
        let actual = transform.transform_vector(source_axis.as_vector()).unwrap();
        let expected = target_axis.as_vector().scaled(3.0).unwrap();
        for (actual, expected) in actual.to_array().into_iter().zip(expected.to_array()) {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
    }
}

#[test]
fn reflection_fixes_its_plane_and_reverses_the_normal_coordinate() {
    let point_on_plane = point(2.0, -5.0, 7.0);
    let normal = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
    let transform = AffineTransform3::try_reflection(point_on_plane, normal).unwrap();
    assert_eq!(
        transform.transform_point(point_on_plane).unwrap(),
        point_on_plane
    );
    assert_eq!(
        transform.transform_point(point(5.0, 3.0, -1.0)).unwrap(),
        point(-1.0, 3.0, -1.0)
    );
    assert_eq!(
        transform
            .transform_vector(Vector3::try_new(1.0, 2.0, 3.0).unwrap())
            .unwrap(),
        Vector3::try_new(-1.0, 2.0, 3.0).unwrap()
    );
}

#[test]
fn reports_orientation_and_a_stable_linear_scale_bound() {
    let origin = point(0.0, 0.0, 0.0);
    let identity = AffineTransform3::identity();
    assert!(!identity.orientation_reversing().unwrap());
    assert_eq!(identity.maximum_linear_scale().unwrap(), 1.0);

    let nonuniform = AffineTransform3::try_nonuniform_scale(origin, [-2.0, 3.0, 4.0]).unwrap();
    assert!(nonuniform.orientation_reversing().unwrap());
    assert_eq!(nonuniform.maximum_linear_scale().unwrap(), 4.0);

    let projection = AffineTransform3::try_nonuniform_scale(origin, [1.0, 1.0, 0.0]).unwrap();
    assert!(projection.orientation_reversing().is_err());
    assert_eq!(projection.maximum_linear_scale().unwrap(), 1.0);
}
