use super::*;
use crate::{Point3, WeightedPoint3};

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn boundaries(weights: [Real; 4]) -> Vec<NurbsCurve> {
    let corners = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 1.0],
        [4.0, 3.0, 2.0],
        [0.0, 3.0, -1.0],
    ];
    let mids = [
        [2.0, -1.0, 2.0],
        [5.0, 1.5, 3.0],
        [2.0, 4.0, -1.0],
        [-1.0, 1.5, 2.0],
    ];
    (0..4)
        .map(|i| {
            NurbsCurve::try_new_rational(
                2,
                [corners[i], mids[i], corners[(i + 1) % 4]]
                    .into_iter()
                    .zip([1.0, weights[i], 1.0])
                    .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn invalid_input_counts_closed_boundaries_and_resource_limits_are_rejected() {
    let source = boundaries([1.0; 4]);
    for curves in [vec![], vec![source[0].clone()], vec![source[0].clone(); 5]] {
        assert!(matches!(
            arrange_edge_surface_curves(&curves, Tolerance::DEFAULT),
            Err(GeometryError::InvalidEdgeSurfaceBoundaries)
        ));
    }
    let closed = NurbsCurve::try_new(
        1,
        vec![
            point([0.0, 0.0, 0.0]),
            point([1.0, 0.0, 0.0]),
            point([1.0, 1.0, 0.0]),
            point([0.0, 1.0, 0.0]),
            point([0.0, 0.0, 0.0]),
        ],
        vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        arrange_edge_surface_curves(&[closed, source[0].clone()], Tolerance::DEFAULT),
        Err(GeometryError::InvalidEdgeSurfaceBoundaries)
    ));
    let broken = NurbsCurve::try_new(
        1,
        vec![
            point([0.0, 0.0, 0.0]),
            point([1.0, 0.0, 0.0]),
            point([2.0, 0.0, 0.0]),
            point([3.0, 0.0, 0.0]),
        ],
        vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        arrange_edge_surface_curves(&[broken, source[0].clone()], Tolerance::DEFAULT),
        Err(GeometryError::InvalidEdgeSurfaceBoundaries)
    ));
    let huge = NurbsCurve::try_clamped_uniform(
        1,
        (0..=MAX_EDGE_CONTROLS)
            .map(|i| point([i as Real, 0.0, 0.0]))
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        arrange_edge_surface_curves(&[huge, source[0].clone()], Tolerance::DEFAULT),
        Err(GeometryError::EdgeSurfaceResourceLimit { .. })
    ));
}
