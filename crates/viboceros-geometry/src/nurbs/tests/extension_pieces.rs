use super::*;

#[test]
fn merged_circular_extension_rebuilds_one_same_radius_arc() {
    let source = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), std::f64::consts::FRAC_1_SQRT_2).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let joined = source
        .try_joined_circularly_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
        .unwrap();
    let merged = source
        .try_merged_circularly_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
        .unwrap();
    let natural = source
        .try_merged_naturally_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
        .unwrap();
    let smooth = source
        .try_extended_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
        .unwrap();

    assert_eq!(joined.knot_multiplicity(1.0).unwrap(), 2);
    assert_eq!(natural, merged);
    assert_ne!(smooth, merged);
    assert_eq!(merged.degree(), 2);
    assert_eq!(merged.control_points().len(), 5);
    assert_eq!(merged.domain(), joined.domain());
    assert!(!merged.knots().contains(&1.0));
    assert!(
        Tolerance::DEFAULT.approx_eq(merged.control_points()[1].weight(), 0.868_960_162_057_560_4,)
    );
    assert_point_near(
        merged.control_points()[4].point(),
        Point3::try_new(-0.479_425_538_604_203, 0.877_582_561_890_372_5, 0.0).unwrap(),
    );
}

#[test]
fn merged_linear_extension_simplifies_lines_and_extends_polyline_end_spans() {
    let straight = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let merged = straight
        .try_merged_linearly_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(merged.degree(), 1);
    assert_eq!(merged.domain(), 0.0..=1.125);
    assert_eq!(
        merged
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(12.0, 0.0, 0.0).unwrap(),
        ]
    );

    let polyline = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let merged = polyline
        .try_merged_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(merged.degree(), 1);
    assert_eq!(merged.control_points().len(), 3);
    assert_eq!(merged.domain(), 0.0..=3.0);
    assert_eq!(
        merged.control_points()[2].point(),
        Point3::try_new(1.0, 2.0, 0.0).unwrap()
    );
}

#[test]
fn joined_linear_extension_uses_unit_polyline_spans() {
    let polyline = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 2.0, 5.0, 5.0],
    )
    .unwrap();
    let joined = polyline
        .try_joined_linearly_by_length(CurveExtensionSide::Both, 1.5, Tolerance::DEFAULT)
        .unwrap();

    assert_eq!(joined.degree(), 1);
    assert_eq!(joined.domain(), 0.0..=4.0);
    assert_eq!(joined.knots(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
    assert_eq!(
        joined
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![
            Point3::try_new(-1.5, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(2.0, 4.5, 0.0).unwrap(),
        ]
    );
}

#[test]
fn joined_natural_extension_retains_source_with_a_full_seam() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, 1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let joined = curve
        .try_joined_naturally_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
        .unwrap();

    assert_eq!(joined.degree(), 2);
    assert_eq!(joined.knot_multiplicity(1.0).unwrap(), 2);
    assert!(
        Tolerance::DEFAULT.approx_eq(
            joined
                .try_trimmed(1.0..=*joined.domain().end())
                .unwrap()
                .length(Tolerance::DEFAULT)
                .unwrap(),
            2.0,
        )
    );
    for sample in 0..=32 {
        let parameter = sample as Real / 32.0;
        assert_point_near(
            joined.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }
}

#[test]
fn separate_linear_extensions_are_unit_domain_tangent_lines() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, 1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let extensions = curve
        .try_separate_linear_extensions_by_length(CurveExtensionSide::Both, 2.0, Tolerance::DEFAULT)
        .unwrap();

    assert_eq!(extensions.len(), 2);
    for extension in &extensions {
        assert_eq!(extension.degree(), 1);
        assert_eq!(extension.domain(), 0.0..=1.0);
        assert_eq!(extension.knots(), &[0.0, 0.0, 1.0, 1.0]);
        assert!(Tolerance::DEFAULT.approx_eq(extension.length(Tolerance::DEFAULT).unwrap(), 2.0,));
    }
    assert_point_near(
        extensions[0].control_points()[0].point(),
        Point3::try_new(-0.894_427_190_999_915_9, -1.788_854_381_999_831_7, 0.0).unwrap(),
    );
    assert_eq!(
        extensions[0].control_points()[1].point(),
        curve.evaluate(0.0).unwrap()
    );
    assert_eq!(
        extensions[1].control_points()[0].point(),
        curve.evaluate(1.0).unwrap()
    );
    assert_point_near(
        extensions[1].control_points()[1].point(),
        Point3::try_new(4.788_854_381_999_831_5, 0.105_572_809_000_084_14, 0.0).unwrap(),
    );
}

#[test]
fn separate_natural_extension_is_the_exact_extrapolated_piece() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, 1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let mut extensions = curve
        .try_separate_natural_extensions_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
        .unwrap();
    let extension = extensions.pop().unwrap();

    assert_eq!(*extension.domain().start(), *curve.domain().end());
    assert!(Tolerance::DEFAULT.approx_eq(extension.length(Tolerance::DEFAULT).unwrap(), 2.0,));
    assert_point_near(
        extension.evaluate(1.0).unwrap(),
        curve.evaluate(1.0).unwrap(),
    );
    let extended = curve
        .try_extended_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
        .unwrap();
    let expected = extended
        .try_trimmed(1.0..=*extended.domain().end())
        .unwrap();
    assert_eq!(extension, expected);
}
