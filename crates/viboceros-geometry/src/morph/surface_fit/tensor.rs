use super::super::interpolation::{DEGREE, collocation_row, knots, solve};
use super::*;
use faer::Mat;

pub(super) fn interpolate(
    point_at: &mut impl FnMut([Real; 2], [ParameterSide; 2]) -> Result<Point3, GeometryError>,
    breaks: &[Vec<Break>; 2],
) -> Result<NurbsSurface, GeometryError> {
    let [count_u, count_v] = breaks.each_ref().map(|b| control_count(b));
    let [knots_u, knots_v] = breaks.each_ref().map(|b| knots(b));
    let rows_u = (0..count_u)
        .map(|i| collocation_row(&knots_u, i))
        .collect::<Result<Vec<_>, _>>()?;
    let rows_v = (0..count_v)
        .map(|i| collocation_row(&knots_v, i))
        .collect::<Result<Vec<_>, _>>()?;
    let mut targets = Vec::with_capacity(count_u * count_v);
    for &(_, v, side_v, _) in &rows_v {
        for &(_, u, side_u, _) in &rows_u {
            targets.push(point_at([u, v], [side_u, side_v])?);
        }
    }
    let candidate = targets[0].to_array();
    let origin = if targets.iter().all(|p| {
        p.to_array()
            .into_iter()
            .zip(candidate)
            .all(|(a, b)| (a - b).is_finite())
    }) {
        candidate
    } else {
        [0.0; 3]
    };
    // A_u C A_v^T = P. Solve all XYZ/V right-hand sides together in U, then
    // transpose the layout and solve all XYZ/U right-hand sides together in V.
    let rhs_u = Mat::from_fn(count_u, count_v * 3, |u, column| {
        let (v, axis) = (column / 3, column % 3);
        targets[v * count_u + u].to_array()[axis] - origin[axis]
    });
    let solved_u = solve(&rows_u.iter().map(|r| r.0).collect::<Vec<_>>(), rhs_u)?;
    let rhs_v = Mat::from_fn(count_v, count_u * 3, |v, column| {
        let (u, axis) = (column / 3, column % 3);
        solved_u[(u, v * 3 + axis)]
    });
    let solved_v = solve(&rows_v.iter().map(|r| r.0).collect::<Vec<_>>(), rhs_v)?;
    let mut controls = Vec::with_capacity(count_u * count_v);
    for (v, row_v) in rows_v.iter().enumerate() {
        for (u, row_u) in rows_u.iter().enumerate() {
            let point = if row_u.3 && row_v.3 {
                targets[v * count_u + u]
            } else {
                Point3::try_from(std::array::from_fn(|axis| {
                    solved_v[(v, u * 3 + axis)] + origin[axis]
                }))?
            };
            controls.push(WeightedPoint3::try_new(point, 1.0)?);
        }
    }
    NurbsSurface::try_new_rational(DEGREE, DEGREE, count_u, count_v, controls, knots_u, knots_v)
}
