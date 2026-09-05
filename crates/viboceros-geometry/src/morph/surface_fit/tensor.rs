use super::super::interpolation::{DEGREE, collocation_row, knots, solve};
use super::*;
use faer::Mat;
use faer::prelude::*;

mod axis;
use axis::Axis;

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
    let grid = Grid {
        degree_u: DEGREE,
        degree_v: DEGREE,
        rows_u,
        rows_v,
        targets,
        knots_u,
        knots_v,
    };
    grid.solve(None)
}

/// A cubic polynomial in XYZ composed with N/W has denominator W³ and
/// numerator degree at most three times each source degree. This is only a
/// candidate space, not an assumption that the supplied point map is cubic.
pub(super) fn rational_candidate(
    point_at: &mut impl FnMut([Real; 2], [ParameterSide; 2]) -> Result<Point3, GeometryError>,
    source: &NurbsSurface,
    maximum: usize,
) -> Result<Option<NurbsSurface>, GeometryError> {
    let Some(denominator) = super::denominator::source_weights(source) else {
        return Ok(None);
    };
    let Some(axis_u) = Axis::cubic_composition(
        source.degree_u(),
        source.knots_u(),
        source.domain_u(),
        maximum,
    ) else {
        return Ok(None);
    };
    let Some(axis_v) = Axis::cubic_composition(
        source.degree_v(),
        source.knots_v(),
        source.domain_v(),
        maximum,
    ) else {
        return Ok(None);
    };
    let (count_u, count_v) = (axis_u.stations.len(), axis_v.stations.len());
    let mut targets = Vec::with_capacity(count_u * count_v);
    let mut weights = Vec::with_capacity(count_u * count_v);
    for &(_, v, sv, _) in &axis_v.stations {
        for &(_, u, su, _) in &axis_u.stations {
            targets.push(point_at([u, v], [su, sv])?);
            let weight = denominator.evaluate_on_sides(u, v, su, sv)?.x().powi(3);
            if weight == 0.0 || !weight.is_finite() {
                return Ok(None);
            }
            weights.push(weight);
        }
    }
    let grid = Grid {
        degree_u: axis_u.degree,
        degree_v: axis_v.degree,
        rows_u: axis_u.stations,
        rows_v: axis_v.stations,
        targets,
        knots_u: axis_u.knots,
        knots_v: axis_v.knots,
    };
    // Numerical failure discards this optional candidate, not source samples.
    // Mapping failures above propagate, and no second point-map pass is used.
    Ok(grid.solve(Some(&weights)).ok())
}

type Row = ([Real; 5], Real, ParameterSide, bool);

struct Grid {
    degree_u: usize,
    degree_v: usize,
    rows_u: Vec<Row>,
    rows_v: Vec<Row>,
    targets: Vec<Point3>,
    knots_u: Vec<Real>,
    knots_v: Vec<Real>,
}

impl Grid {
    fn solve(&self, weights: Option<&[Real]>) -> Result<NurbsSurface, GeometryError> {
        let (rows_u, rows_v, targets) = (&self.rows_u, &self.rows_v, &self.targets);
        let (count_u, count_v) = (rows_u.len(), rows_v.len());
        let width = if weights.is_some() { 4 } else { 3 };
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
        let rhs_u = Mat::from_fn(count_u, count_v * width, |u, column| {
            let (v, axis) = (column / width, column % width);
            let weight = weights.map_or(1.0, |w| w[v * count_u + u]);
            if axis == 3 {
                weight
            } else {
                (targets[v * count_u + u].to_array()[axis] - origin[axis]) * weight
            }
        });
        let solve_axis = |rows: &[Row], knots: &[Real], degree: usize, rhs: Mat<Real>| {
            if degree == DEGREE {
                solve(&rows.iter().map(|r| r.0).collect::<Vec<_>>(), rhs)
            } else {
                let count = rows.len();
                let mut matrix = Mat::zeros(count, count);
                for (i, &(_, t, _, fixed)) in rows.iter().enumerate() {
                    if fixed {
                        matrix[(i, i)] = 1.0;
                    } else {
                        for (j, value) in
                            crate::nurbs::bspline_basis_values(knots, degree, count, t)?
                                .into_iter()
                                .enumerate()
                        {
                            matrix[(i, j)] = value;
                        }
                    }
                }
                Ok(matrix.full_piv_lu().solve(&rhs))
            }
        };
        let solved_u = solve_axis(rows_u, &self.knots_u, self.degree_u, rhs_u)?;
        let rhs_v = Mat::from_fn(count_v, count_u * width, |v, column| {
            let (u, axis) = (column / width, column % width);
            solved_u[(u, v * width + axis)]
        });
        let solved_v = solve_axis(rows_v, &self.knots_v, self.degree_v, rhs_v)?;
        let mut controls = Vec::with_capacity(count_u * count_v);
        for (v, row_v) in rows_v.iter().enumerate() {
            for (u, row_u) in rows_u.iter().enumerate() {
                let weight = if weights.is_some() {
                    solved_v[(v, u * width + 3)]
                } else {
                    1.0
                };
                if weight <= 0.0 || !weight.is_finite() {
                    return Err(GeometryError::ZeroWeightAtParameter);
                }
                let point = if row_u.3 && row_v.3 {
                    targets[v * count_u + u]
                } else {
                    Point3::try_from(std::array::from_fn(|axis| {
                        solved_v[(v, u * width + axis)] / weight + origin[axis]
                    }))?
                };
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
        }
        NurbsSurface::try_new_rational(
            self.degree_u,
            self.degree_v,
            count_u,
            count_v,
            controls,
            self.knots_u.clone(),
            self.knots_v.clone(),
        )
    }
}
