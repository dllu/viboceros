use super::*;

fn elliptic_stadium(tolerance: Tolerance) -> crate::PolyCurve3 {
    use crate::{CurveSegment3, LineSegment, PolyCurve3};
    use std::f64::consts::{FRAC_PI_2, PI};
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let ellipse = |x| {
        crate::Ellipse3::try_from_three_points(p(x, 0.), p(x + 3., 0.), p(x, 2.), tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let left = ellipse(-4.)
        .try_trimmed(FRAC_PI_2..=3. * FRAC_PI_2)
        .unwrap();
    let right = ellipse(4.).try_change_closed_seam(3. * FRAC_PI_2).unwrap();
    let right = right
        .try_trimmed(*right.domain().start()..=right.domain().start() + PI)
        .unwrap();
    PolyCurve3::try_new(vec![
        CurveSegment3::Line(LineSegment::try_new(p(4., 2.), p(-4., 2.), tolerance).unwrap()),
        CurveSegment3::NurbsCurve(left),
        CurveSegment3::Line(LineSegment::try_new(p(-4., -2.), p(4., -2.), tolerance).unwrap()),
        CurveSegment3::NurbsCurve(right),
    ])
    .unwrap()
}

#[test]
fn planar_inset_curved_groups_keep_straight_subdivision_triangles_and_total_area() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let curve = elliptic_stadium(tolerance);
    let mut splits = Vec::new();
    for i in 0..4 {
        let d = curve.segment_domain(i).unwrap();
        if i > 0 {
            splits.push(*d.start());
        }
        splits.push(d.start().midpoint(*d.end()));
    }
    splits.sort_by(Real::total_cmp);
    let source = Brep::try_planar_face(&curve.to_nurbs().unwrap(), tolerance)
        .unwrap()
        .try_split_edges_at_parameters(&[(0, splits)], tolerance)
        .unwrap();
    let before = source.clone();
    let result = source
        .try_inset_planar_faces(&[0], 0.3, true, tolerance)
        .unwrap();
    assert_eq!(
        (
            source.edges().len(),
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (8, 7, 18, 12)
    );
    assert_eq!(result.faces()[0].loops()[0].trims().len(), 4);
    assert_eq!(
        result.faces()[1..]
            .iter()
            .filter(|f| f.loops()[0].trims().len() == 3)
            .count(),
        2
    );
    assert!((result.area(tolerance).unwrap() - source.area(tolerance).unwrap()).abs() < 1e-6);
    assert_eq!(&result.edges()[..source.edges().len()], source.edges());
    assert_eq!(source, before);
}

#[test]
fn planar_inset_tight_curvature_continuous_edge_partitions_keep_annular_borders() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let curve = crate::Ellipse3::try_from_three_points(
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(6., 0., 0.).unwrap(),
        Point3::try_new(0., 4., 0.).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let source = Brep::try_planar_face(&curve, tolerance)
        .unwrap()
        .try_split_edges_at_parameters(
            &[(
                0,
                vec![
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                    3. * std::f64::consts::FRAC_PI_2,
                ],
            )],
            tolerance,
        )
        .unwrap();
    let before = source.clone();
    for distance in [0.3, 3.] {
        for crease_corners in [false, true] {
            let result = source
                .try_inset_planar_faces(&[0], distance, crease_corners, tolerance)
                .unwrap();
            assert_eq!(result.faces().len(), 2);
            assert_eq!(result.edges().len(), if distance == 0.3 { 5 } else { 6 });
            assert_eq!(&result.edges()[..source.edges().len()], source.edges());
            assert_eq!(source, before);
        }
    }
}
