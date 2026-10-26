use super::*;

#[test]
fn boundary_extension_uses_the_nearest_hit_beyond_each_line_end() {
    let source = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(5.0, 0.0)],
        vec![0.0, 0.0, 5.0, 5.0],
    )
    .unwrap();
    let boundary = |x| {
        NurbsCurve::try_new(
            1,
            vec![point(x, -5.0), point(x, 5.0)],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap()
    };
    let extended = source
        .try_merged_to_curve_boundaries(
            CurveExtensionSide::Both,
            CurveExtensionStyle::Line,
            &[boundary(12.0), boundary(-3.0), boundary(8.0)],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(extended.degree(), 1);
    assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().start(), -3.0));
    assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().end(), 8.0));
    assert_point_near(
        extended.evaluate(*extended.domain().start()).unwrap(),
        point(-3.0, 0.0),
    );
    assert_point_near(
        extended.evaluate(*extended.domain().end()).unwrap(),
        point(8.0, 0.0),
    );

    assert_eq!(
        source.try_merged_to_curve_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Line,
            &[boundary(2.0)],
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::CurveExtensionBoundaryNotFound)
    );
}

#[test]
fn boundary_extension_intersects_surfaces_and_trimmed_breps() {
    let source = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(5.0, 0.0)],
        vec![0.0, 0.0, 5.0, 5.0],
    )
    .unwrap();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(10.0, -5.0, -5.0).unwrap(),
        Point3::try_new(10.0, 5.0, -5.0).unwrap(),
        Point3::try_new(10.0, 5.0, 5.0).unwrap(),
        Point3::try_new(10.0, -5.0, 5.0).unwrap(),
    ])
    .unwrap();
    let extended = source
        .try_merged_to_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Line,
            &[CurveExtensionBoundary::Surface(surface)],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_point_near(
        extended.evaluate(*extended.domain().end()).unwrap(),
        point(10.0, 0.0),
    );

    let frame = Frame3::try_from_normal(
        point(0.0, 0.0),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let box_boundary = Brep::try_box(
        frame,
        [[12.0, 14.0], [-2.0, 2.0], [-2.0, 2.0]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let extended = source
        .try_merged_to_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Line,
            &[CurveExtensionBoundary::Brep(box_boundary)],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_point_near(
        extended.evaluate(*extended.domain().end()).unwrap(),
        point(12.0, 0.0),
    );
}

#[test]
fn boundary_extension_respects_curvature_and_brep_trim_holes() {
    let frame = Frame3::try_from_normal(
        point(0.0, 0.0),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame, 2.0, -2.0, 2.0).unwrap();
    let radial_source = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(0.5, 0.0)],
        vec![0.0, 0.0, 0.5, 0.5],
    )
    .unwrap();
    let extended = radial_source
        .try_merged_to_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Line,
            &[CurveExtensionBoundary::Surface(cylinder)],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_point_near(
        extended.evaluate(*extended.domain().end()).unwrap(),
        point(2.0, 0.0),
    );

    let outer = Polyline3::try_new(
        vec![
            Point3::try_new(10.0, -3.0, -3.0).unwrap(),
            Point3::try_new(10.0, 3.0, -3.0).unwrap(),
            Point3::try_new(10.0, 3.0, 3.0).unwrap(),
            Point3::try_new(10.0, -3.0, 3.0).unwrap(),
            Point3::try_new(10.0, -3.0, -3.0).unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let hole = crate::Circle3::try_new(
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        1.0,
        crate::UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let holed_face = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
    let farther_surface = NurbsSurface::try_bilinear([
        Point3::try_new(12.0, -3.0, -3.0).unwrap(),
        Point3::try_new(12.0, 3.0, -3.0).unwrap(),
        Point3::try_new(12.0, 3.0, 3.0).unwrap(),
        Point3::try_new(12.0, -3.0, 3.0).unwrap(),
    ])
    .unwrap();
    let extended = radial_source
        .try_merged_to_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Line,
            &[
                CurveExtensionBoundary::Brep(holed_face),
                CurveExtensionBoundary::Surface(farther_surface),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_point_near(
        extended.evaluate(*extended.domain().end()).unwrap(),
        point(12.0, 0.0),
    );
}

#[test]
fn boundary_extension_enters_a_coplanar_trim_before_the_underlying_surface_edge() {
    let source = NurbsCurve::try_new(
        1,
        vec![point(0.0, 1.0), point(5.0, 1.0)],
        vec![0.0, 0.0, 5.0, 5.0],
    )
    .unwrap();
    let diamond = Polyline3::try_new(
        vec![
            point(12.0, -3.0),
            point(15.0, 0.0),
            point(12.0, 3.0),
            point(9.0, 0.0),
            point(12.0, -3.0),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let face = Brep::try_planar_face_with_holes(&diamond, &[], Tolerance::DEFAULT).unwrap();
    let extended = source
        .try_merged_to_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Line,
            &[CurveExtensionBoundary::Brep(face)],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_point_near(
        extended.evaluate(*extended.domain().end()).unwrap(),
        point(10.0, 1.0),
    );
}

#[test]
fn smooth_boundary_extension_matches_exact_nurbs_extrapolation() {
    let source = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(2.0, 3.0), point(4.0, 0.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let boundary = NurbsCurve::try_new(
        1,
        vec![point(7.0, -20.0), point(7.0, 20.0)],
        vec![0.0, 0.0, 40.0, 40.0],
    )
    .unwrap();
    let extended = source
        .try_merged_to_curve_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Smooth,
            &[boundary],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!((*extended.domain().end() - 1.75).abs() <= 2.0e-12);
    assert_point_near(extended.control_points()[2].point(), point(7.0, -7.875));
}

#[test]
fn circular_boundary_extension_hits_the_exact_osculating_arc() {
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
    let boundary = NurbsCurve::try_new(
        1,
        vec![point(-0.5, -2.0), point(-0.5, 2.0)],
        vec![0.0, 0.0, 4.0, 4.0],
    )
    .unwrap();
    let joined = source
        .try_joined_to_curve_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Arc,
            std::slice::from_ref(&boundary),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(joined.degree(), 2);
    assert_eq!(joined.knot_multiplicity(1.0).unwrap(), 2);
    assert_point_near(
        joined.evaluate(*joined.domain().end()).unwrap(),
        point(-0.5, 3.0_f64.sqrt() * 0.5),
    );
    assert!(
        (joined
            .try_trimmed(1.0..=*joined.domain().end())
            .unwrap()
            .length(Tolerance::DEFAULT)
            .unwrap()
            - std::f64::consts::FRAC_PI_6)
            .abs()
            < 1.0e-11
    );

    let merged = source
        .try_merged_to_curve_boundaries(
            CurveExtensionSide::End,
            CurveExtensionStyle::Natural,
            &[boundary],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!(
        merged
            .try_canonical_circular_arc(Tolerance::DEFAULT)
            .unwrap()
            .is_some()
    );
    assert_point_near(
        merged.evaluate(*merged.domain().end()).unwrap(),
        point(-0.5, 3.0_f64.sqrt() * 0.5),
    );
}
