use super::*;
use crate::{Circle3, Ellipse3, Vector3};

#[test]
fn trimmed_circle_keeps_native_angular_parameters() {
    let circle = Circle3::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        3.0,
        Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_reparameterized(-7.0..=13.0)
    .unwrap();
    let source = Curve3::Circle(circle);
    let piece = source.try_trimmed(-4.0..=9.0).unwrap();
    assert!(matches!(piece, Curve3::Arc(_)));
    for i in 0..=128 {
        let t = piece.as_ref().parameter_at(i as Real / 128.0).unwrap();
        assert!(
            piece
                .as_ref()
                .evaluate(t)
                .unwrap()
                .distance_to(source.as_ref().evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn ellipse_split_at_existing_quarter_knots_has_regular_derivatives() {
    let axis = |x, y, z| {
        Vector3::try_new(x, y, z)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
    };
    let ellipse = Ellipse3::try_new(
        Point3::try_new(2.0, -3.0, 4.0).unwrap(),
        5.0,
        2.0,
        axis(1.0, 0.0, 0.0),
        axis(0.0, 1.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_reparameterized(-7.0..=13.0)
    .unwrap();
    let pieces = Curve3::Ellipse(ellipse)
        .try_split_at_parameters(&[3.0, -2.0])
        .unwrap();
    for piece in pieces {
        let view = piece.as_ref();
        for i in 0..=32 {
            let t = view.parameter_at(i as Real / 32.0).unwrap();
            view.evaluate_with_second_derivative(t).unwrap();
            view.evaluate_with_tangent(t)
                .unwrap_or_else(|e| panic!("at {t} on {piece:?}: {e}"));
        }
        view.divide_by_count_samples(17, true, Tolerance::DEFAULT)
            .unwrap_or_else(|e| panic!("division of {piece:?}: {e}"));
    }
}

#[test]
fn reparameterization_keeps_exact_knots_and_no_op_identity() {
    let curve = crate::NurbsCurve::try_new(
        1,
        (0..5)
            .map(|i| Point3::try_new(i as Real, 0.0, 0.0).unwrap())
            .collect(),
        vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
    )
    .unwrap();
    let mapped = curve.try_reparameterized(-7.0..=13.0).unwrap();
    assert_eq!(mapped.knots(), &[-7.0, -7.0, -2.0, 3.0, 8.0, 13.0, 13.0]);
    assert_eq!(mapped.try_reparameterized(mapped.domain()).unwrap(), mapped);
    let piece = mapped.try_trimmed(-2.0..=3.0).unwrap();
    assert_eq!(piece.control_points().len(), 2);
    assert!(piece.derivative_at(3.0).unwrap().length().unwrap() > 0.0);
}

#[test]
fn polyline_seams_near_existing_vertices_do_not_create_sliver_segments() {
    let curve = Polyline3::try_new(
        [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0), (0.0, 0.0)]
            .map(|(x, y)| Point3::try_new(x, y, 0.0).unwrap())
            .to_vec(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = Curve3::Polyline(curve.clone());
    for t in [
        0.0_f64.next_up(),
        1.0_f64.next_down(),
        1.0_f64.next_up(),
        4.0_f64.next_down(),
    ] {
        let result = source.try_change_closed_seam(t).unwrap();
        let Curve3::Polyline(result) = result else {
            panic!("seam must retain polyline")
        };
        assert_eq!(result.vertices().len(), curve.vertices().len());
        assert_eq!(result.domain(), t..=t + 4.0);
        assert_eq!(result.length().unwrap(), 14.0);
    }
}

#[test]
fn tiny_circle_seam_does_not_revalidate_with_document_distance_tolerance() {
    let tolerance = Tolerance::try_new(1e-14, 1e-12, 1e-10).unwrap();
    let circle = Circle3::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        1e-12,
        Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        tolerance,
    )
    .unwrap();
    let t = *circle.domain().end() * 0.3;
    let moved = circle.try_change_closed_seam(t).unwrap();
    assert_eq!(moved.radius(), circle.radius());
    assert!(
        moved
            .evaluate(t)
            .unwrap()
            .distance_to(circle.evaluate(t).unwrap())
            .unwrap()
            < 1e-26
    );
}

#[test]
fn analytic_closest_locations_use_native_parameters_and_clamp_arc_ends() {
    let axis = Vector3::try_new(0.0, 0.0, 1.0)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let circle = Circle3::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        3.0,
        axis,
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_reparameterized(-7.0..=13.0)
    .unwrap();
    let arc = crate::CircularArc3::try_from_circle_sweep(circle, 1.2)
        .unwrap()
        .try_reparameterized(-7.0..=13.0)
        .unwrap();
    let angle = 0.7_f64;
    let target = Point3::try_new(5.0 * angle.cos(), 5.0 * angle.sin(), 11.0).unwrap();
    let circular = CurveRef::Circle(&circle)
        .closest_parameter(target, Tolerance::DEFAULT)
        .unwrap();
    let partial = CurveRef::Arc(&arc)
        .closest_parameter(target, Tolerance::DEFAULT)
        .unwrap();
    assert!((circular - (-7.0 + 20.0 * angle / std::f64::consts::TAU)).abs() < 1e-13);
    assert!((partial - (-7.0 + 20.0 * angle / 1.2)).abs() < 1e-13);
    assert_eq!(
        CurveRef::Arc(&arc)
            .closest_parameter(Point3::try_new(3.0, -1.0, 0.0).unwrap(), Tolerance::DEFAULT)
            .unwrap(),
        -7.0
    );
}

#[test]
fn wrapped_ellipse_length_station_matches_high_precision_reference() {
    let axis = |x, y, z| {
        Vector3::try_new(x, y, z)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
    };
    let curve = Curve3::Ellipse(
        Ellipse3::try_new(
            Point3::try_new(2.0, -3.0, 4.0).unwrap(),
            5.0,
            2.0,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_reparameterized(-7.0..=13.0)
        .unwrap(),
    );
    let pieces = curve.try_split_at_parameters(&[1.0]).unwrap();
    let stations = pieces[0]
        .as_ref()
        .divide_by_count_samples(17, true, Tolerance::DEFAULT)
        .unwrap();
    // Independently checked by 40-digit rational de Boor + arc-length quadrature.
    assert!((stations[3].parameter() - 5.771_158_143_654_643_5).abs() < 2e-11);
}

#[test]
fn native_edits_reject_invalid_intervals_and_split_lists() {
    let line = Curve3::Line(
        crate::LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    for parameters in [
        vec![],
        vec![0.0],
        vec![10.0],
        vec![2.0, 2.0],
        vec![Real::NAN],
        vec![Real::INFINITY],
    ] {
        assert!(line.try_split_at_parameters(&parameters).is_err());
    }
    for interval in [0.0..=0.0, -1.0..=5.0, 5.0..=11.0, Real::NAN..=3.0] {
        assert!(line.try_trimmed(interval).is_err());
    }
    assert!(matches!(
        line.try_change_closed_seam(2.0),
        Err(GeometryError::CurveSeamMustBeClosed)
    ));
}
