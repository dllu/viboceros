use super::*;
use crate::{CurveOffsetCornerStyle, Vector3};
fn source(points: &[[Real; 3]]) -> Polyline3 {
    Polyline3::try_new(
        points
            .iter()
            .map(|&p| Point3::try_from(p).unwrap())
            .collect(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn normal() -> UnitVector3 {
    Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}
#[test]
fn outside_fans_and_inside_triangles_sew_both_sides_without_changing_source() {
    let source = source(&[[0., 0., 0.], [5., 0., 0.], [5., 4., 0.]]);
    let before = source.clone();
    for (corner, faces, edges, vertices) in [
        (RibbonCornerStyle::Sharp, 4, 12, 9),
        (RibbonCornerStyle::Round, 7, 18, 12),
        (RibbonCornerStyle::Smooth, 7, 18, 12),
        (RibbonCornerStyle::Chamfer, 7, 18, 12),
    ] {
        let parts =
            try_polyline_ribbon(&source, -0.5, true, normal(), Tolerance::DEFAULT, corner).unwrap();
        assert_eq!(parts.len(), 1);
        let b = &parts[0];
        assert_eq!(
            (b.faces().len(), b.edges().len(), b.vertices().len()),
            (faces, edges, vertices)
        );
    }
    assert_eq!(source, before);
}
#[test]
fn none_keeps_convex_gaps_and_shares_the_original_source_vertex() {
    let source = source(&[[0., 0., 0.], [5., 0., 0.], [5., 4., 0.]]);
    let parts = try_polyline_ribbon(
        &source,
        -0.5,
        false,
        normal(),
        Tolerance::DEFAULT,
        CurveOffsetCornerStyle::None,
    )
    .unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(
        (
            parts[0].faces().len(),
            parts[0].edges().len(),
            parts[0].vertices().len()
        ),
        (2, 8, 7)
    );
    assert_eq!(parts[0].edge_connected_face_components().len(), 2);
}
#[test]
fn invalid_distance_nonplanarity_and_overlapping_inside_corners_fail_atomically() {
    let s = source(&[[0., 0., 0.], [5., 0., 0.], [5., 4., 0.]]);
    for d in [0., f64::NAN, f64::INFINITY, 8.] {
        assert!(
            try_polyline_ribbon(
                &s,
                d,
                false,
                normal(),
                Tolerance::DEFAULT,
                CurveOffsetCornerStyle::Round
            )
            .is_err()
        );
    }
    let nonplanar = source(&[[0., 0., 0.], [5., 0., 0.], [5., 4., 0.], [3., 4., 2.]]);
    assert!(
        try_polyline_ribbon(
            &nonplanar,
            0.5,
            false,
            normal(),
            Tolerance::DEFAULT,
            CurveOffsetCornerStyle::Sharp
        )
        .is_err()
    );
}

#[test]
fn patch_budget_is_checked_before_invoking_the_backend() {
    let points = (0..=1025).map(|i| [i as Real, 0., 0.]).collect::<Vec<_>>();
    let source = source(&points);
    let result = try_polyline_ribbon_with_ruled(
        &source,
        0.5,
        false,
        normal(),
        Tolerance::DEFAULT,
        CurveOffsetCornerStyle::Sharp,
        &mut |_, _| -> Result<NurbsSurface, GeometryError> {
            panic!("budget must precede construction")
        },
    );
    assert!(matches!(
        result,
        Err(GeometryError::RibbonResourceLimit {
            maximum: MAX_PATCHES
        })
    ));
}

#[test]
fn nonadjacent_source_vertices_keep_distinct_identities_at_a_self_contact() {
    let s = source(&[
        [0., 0., 0.],
        [5., 0., 0.],
        [5., 5., 0.],
        [0., 0., 0.],
        [-5., 0., 0.],
        [-5., -5., 0.],
    ]);
    let b = try_polyline_ribbon(
        &s,
        -0.2,
        false,
        normal(),
        Tolerance::DEFAULT,
        RibbonCornerStyle::None,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        b.vertices()
            .iter()
            .filter(|v| v.point() == s.vertices()[0])
            .count(),
        2
    );
}
