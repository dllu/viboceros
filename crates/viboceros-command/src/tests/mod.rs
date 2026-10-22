//! Command regression topics and shared geometry fixtures.
mod curves;
mod document;
mod extraction;
mod intersection;
mod mesh_edit;
mod primitives;
mod refinement;
mod surfaces;
mod transforms;

use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::{BrepTrimType, WeightedPoint3};

fn planar_intersection_surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 10.0, 0.0).unwrap(),
        Point3::try_new(0.0, 10.0, 0.0).unwrap(),
    ])
    .unwrap()
}

fn nonaffine_planar_surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(8.0, 10.0, 0.0).unwrap(),
        Point3::try_new(0.0, 10.0, 0.0).unwrap(),
    ])
    .unwrap()
}

fn vertical_intersection_surface(x_start: Real, x_end: Real) -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(x_start, 5.0, -5.0).unwrap(),
        Point3::try_new(x_end, 5.0, -5.0).unwrap(),
        Point3::try_new(x_end, 5.0, 5.0).unwrap(),
        Point3::try_new(x_start, 5.0, 5.0).unwrap(),
    ])
    .unwrap()
}

fn transverse_surface_at_x(x: Real) -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(x, -5.0, -5.0).unwrap(),
        Point3::try_new(x, 15.0, -5.0).unwrap(),
        Point3::try_new(x, 15.0, 5.0).unwrap(),
        Point3::try_new(x, -5.0, 5.0).unwrap(),
    ])
    .unwrap()
}

fn transverse_surface_at_y(y: Real) -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(-5.0, y, -5.0).unwrap(),
        Point3::try_new(15.0, y, -5.0).unwrap(),
        Point3::try_new(15.0, y, 5.0).unwrap(),
        Point3::try_new(-5.0, y, 5.0).unwrap(),
    ])
    .unwrap()
}

fn west_east_diagonal_cutting_surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 2.0, -5.0).unwrap(),
        Point3::try_new(10.0, 8.0, -5.0).unwrap(),
        Point3::try_new(10.0, 8.0, 5.0).unwrap(),
        Point3::try_new(0.0, 2.0, 5.0).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=136.0_f64.sqrt(), 0.0..=10.0)
    .unwrap()
}

fn south_north_diagonal_cutting_surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(2.0, 0.0, -5.0).unwrap(),
        Point3::try_new(8.0, 10.0, -5.0).unwrap(),
        Point3::try_new(8.0, 10.0, 5.0).unwrap(),
        Point3::try_new(2.0, 0.0, 5.0).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=136.0_f64.sqrt(), 0.0..=10.0)
    .unwrap()
}

fn straight_cutting_surface(start: [Real; 2], end: [Real; 2]) -> NurbsSurface {
    let length = (end[0] - start[0]).hypot(end[1] - start[1]);
    NurbsSurface::try_bilinear([
        Point3::try_new(start[0], start[1], -5.0).unwrap(),
        Point3::try_new(end[0], end[1], -5.0).unwrap(),
        Point3::try_new(end[0], end[1], 5.0).unwrap(),
        Point3::try_new(start[0], start[1], 5.0).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=length, 0.0..=10.0)
    .unwrap()
}

fn curved_cutting_curve(points: [[Real; 2]; 3]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        points
            .into_iter()
            .zip([1.0, 0.75, 1.0])
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_new(point[0], point[1], 0.0).unwrap(), weight)
                    .unwrap()
            })
            .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap()
}

fn assert_surface_bounds_near(
    actual: [[Real; 2]; 2],
    expected: [[Real; 2]; 2],
    tolerance: Tolerance,
) {
    for (actual, expected) in actual
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert!(
            tolerance.approx_eq(actual, expected),
            "surface parameter bound {actual} differs from {expected}"
        );
    }
}

fn intersection_box() -> Brep {
    intersection_box_with_intervals([[0.0, 10.0], [0.0, 10.0], [0.0, 10.0]])
}

fn intersection_box_with_intervals(intervals: [[Real; 2]; 3]) -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(frame, intervals, Tolerance::DEFAULT).unwrap()
}

fn intersection_cylinder() -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_cylinder(frame, 5.0, 0.0, 10.0, Tolerance::DEFAULT).unwrap()
}

pub(super) fn rational_multi_span_surface() -> NurbsSurface {
    let mut controls = Vec::new();
    for v in 0..4 {
        for u in 0..4 {
            controls.push(
                WeightedPoint3::try_new(
                    Point3::try_new(
                        u as Real * 2.0 + v as Real * 0.25,
                        v as Real * 1.5 - u as Real * 0.4,
                        (u * v) as Real * 0.3,
                    )
                    .unwrap(),
                    0.5 + (u + 2 * v) as Real * 0.2,
                )
                .unwrap(),
            );
        }
    }
    NurbsSurface::try_new_rational(
        2,
        2,
        4,
        4,
        controls,
        vec![-2.0, -1.0, 0.0, 0.7, 2.0, 3.0, 4.0],
        vec![8.0, 9.0, 10.0, 11.5, 14.0, 15.0, 16.0],
    )
    .unwrap()
}

fn multiple_knot_curve() -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        [
            [0.0, 0.0, 0.0],
            [1.0, 3.0, 1.0],
            [3.0, -2.0, 2.0],
            [5.0, 4.0, -1.0],
            [7.0, 0.0, 1.0],
            [9.0, -3.0, 2.0],
            [11.0, 5.0, -2.0],
            [13.0, 1.0, 0.0],
            [15.0, -1.0, 3.0],
            [18.0, 2.0, 1.0],
        ]
        .into_iter()
        .map(|point| Point3::try_from(point).unwrap())
        .collect(),
        vec![
            0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0,
        ],
    )
    .unwrap()
}

fn periodic_cubic_curve() -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 2.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
    )
    .unwrap()
}

fn periodic_cubic_u_surface() -> NurbsSurface {
    let curve = periodic_cubic_curve();
    let mut controls = curve
        .control_points()
        .iter()
        .map(|control| control.point())
        .collect::<Vec<_>>();
    controls.extend(curve.control_points().iter().map(|control| {
        control
            .point()
            .translated(Vector3::try_new(0.0, 0.0, 3.0).unwrap())
            .unwrap()
    }));
    NurbsSurface::try_new(
        3,
        1,
        7,
        2,
        controls,
        curve.knots().to_vec(),
        vec![0.0, 0.0, 5.0, 5.0],
    )
    .unwrap()
}

fn rectangular_annulus_mesh(tolerance: Tolerance) -> TriangleMesh {
    TriangleMesh::try_new_faces(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 10.0, 0.0).unwrap(),
            Point3::try_new(0.0, 10.0, 0.0).unwrap(),
            Point3::try_new(3.0, 3.0, 0.0).unwrap(),
            Point3::try_new(7.0, 3.0, 0.0).unwrap(),
            Point3::try_new(7.0, 7.0, 0.0).unwrap(),
            Point3::try_new(3.0, 7.0, 0.0).unwrap(),
        ],
        vec![
            viboceros_geometry::MeshFace::Quad([0, 1, 5, 4]),
            viboceros_geometry::MeshFace::Quad([1, 2, 6, 5]),
            viboceros_geometry::MeshFace::Quad([2, 3, 7, 6]),
            viboceros_geometry::MeshFace::Quad([3, 0, 4, 7]),
        ],
        tolerance,
    )
    .unwrap()
}
