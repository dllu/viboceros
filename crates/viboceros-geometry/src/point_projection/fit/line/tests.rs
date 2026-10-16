use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn exact_collinear_line_fits_keep_extrema_across_coordinate_scales() {
    for scale in [2_f64.powi(-333), 1., 2_f64.powi(333)] {
        let points = [
            p(-3. * scale, -6. * scale, -9. * scale),
            p(scale, 2. * scale, 3. * scale),
            p(2. * scale, 4. * scale, 6. * scale),
        ];
        let line = LineSegment::try_fit_to_points(&points).unwrap().unwrap();
        assert_eq!(line.start(), points[0]);
        assert_eq!(line.end(), points[2]);
        assert_eq!(line.domain(), 0. ..=line.length().unwrap());
        let reversed =
            LineSegment::try_fit_to_points(&points.into_iter().rev().collect::<Vec<_>>())
                .unwrap()
                .unwrap();
        assert_eq!(line, reversed);
    }
}

#[test]
fn full_range_exact_lines_and_subnormal_lengths_do_not_enter_the_svd() {
    let h = 2_f64.powi(1000);
    let t = f64::from_bits(1);
    for points in [
        [p(-h, 0., -h), p(0., 0., 0.), p(h, 0., h)],
        [p(0., 0., 0.), p(t, 0., 0.), p(t, 0., 0.)],
    ] {
        let line = LineSegment::try_fit_to_points(&points).unwrap().unwrap();
        assert_eq!(line.start(), points[0]);
        assert_eq!(line.end(), points[2]);
        assert!(*line.domain().end() > 0.);
    }
}

#[test]
fn rank_two_and_three_clouds_choose_the_least_squared_orthogonal_error_axis() {
    for spatial in [false, true] {
        let points = [
            (-4., -1., -0.1),
            (-4., 1., 0.1),
            (4., -1., 0.1),
            (4., 1., -0.1),
        ]
        .map(|(x, y, z)| p(x, y, if spatial { z } else { 0. }));
        let line = LineSegment::try_fit_to_points(&points).unwrap().unwrap();
        assert!(line.start().distance_to(p(-4., 0., 0.)).unwrap() < 1e-13);
        assert!(line.end().distance_to(p(4., 0., 0.)).unwrap() < 1e-13);
        let projector = PointProjection3::onto_best_fit_line(&points)
            .unwrap()
            .unwrap();
        let error = points
            .into_iter()
            .map(|p| {
                p.distance_to(projector.project(p).unwrap())
                    .unwrap()
                    .powi(2)
            })
            .sum::<f64>();
        assert!((error - if spatial { 4.04 } else { 4. }).abs() < 1e-12);
    }
}

#[test]
fn repeated_point_weights_and_permutations_preserve_the_selected_objective() {
    let mut points = vec![
        p(0., 0., 1.),
        p(2., 0.2, 1.),
        p(4., 0., 1.),
        p(6., -0.1, 1.),
    ];
    let baseline = LineSegment::try_fit_to_points(&points).unwrap().unwrap();
    points.extend([points[1]; 6]);
    let weighted = LineSegment::try_fit_to_points(&points).unwrap().unwrap();
    assert!(baseline.start().distance_to(weighted.start()).unwrap() > 0.1);
    for _ in 0..points.len() {
        points.rotate_left(1);
        let line = LineSegment::try_fit_to_points(&points).unwrap().unwrap();
        assert!(line.start().distance_to(weighted.start()).unwrap() < 1e-12);
        assert!(line.end().distance_to(weighted.end()).unwrap() < 1e-12);
    }
}

#[test]
fn line_fit_degeneracy_limits_and_normalized_underflow_are_explicit() {
    assert!(LineSegment::try_fit_to_points(&[]).is_err());
    assert!(LineSegment::try_fit_to_points(&[p(1., 2., 3.)]).is_err());
    assert!(
        LineSegment::try_fit_to_points(&[p(1., 2., 3.); 3])
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        LineSegment::try_fit_to_points(&vec![p(0., 0., 0.); MAX_LINE_FIT_POINTS + 1]),
        Err(GeometryError::LineFitResourceLimit { .. })
    ));
    let (h, t) = (2_f64.powi(1023), f64::from_bits(1));
    assert!(matches!(
        LineSegment::try_fit_to_points(&[p(0., 0., 0.), p(h, 0., 0.), p(0., t, 0.)]),
        Err(GeometryError::LineFitNumericalRange)
    ));
    assert!(
        LineSegment::try_fit_to_points(&[p(0., 0., 0.), p(1e-9, 0., 0.)])
            .unwrap()
            .is_some()
    );
}
