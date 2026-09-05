use super::*;
use crate::nurbs::{bspline_basis_values_extended, stable_knot_mean};

pub(super) struct Direction {
    pub knots: Vec<Real>,
    pub parameters: Vec<Real>,
    pub control_count: usize,
    pub amplification: Vec<Real>,
    matrix: Mat<Real>,
}

impl Direction {
    pub fn new(
        points: &[Point3],
        count: [usize; 2],
        axis: usize,
        degree: usize,
        closed: bool,
    ) -> Result<Self, GeometryError> {
        let n = count[axis];
        let width = count[1 - axis];
        let point = |i: usize, j: usize| {
            points[if axis == 0 {
                j * count[0] + i
            } else {
                i * count[0] + j
            }]
        };
        let mut parameters = vec![0.0];
        for i in 0..n - usize::from(!closed) {
            let distances = (0..width)
                .map(|j| point(i, j).distance_to(point((i + 1) % n, j)))
                .collect::<Result<Vec<_>, _>>()?;
            let delta = stable_knot_mean(&distances)?;
            let previous = *parameters.last().unwrap();
            let next = previous + delta;
            require_finite([next], "point grid parameters")?;
            if next < previous || (next == previous && (!closed || degree == 1)) {
                return Err(GeometryError::InvalidPointGrid {
                    context: "coincident successive interpolation stations",
                });
            }
            parameters.push(next);
        }
        let period = *parameters.last().unwrap();
        if period <= 0.0 {
            return Err(GeometryError::InvalidPointGrid {
                context: "zero-length parameter direction",
            });
        }
        let control_count = n + if closed { degree } else { 0 };
        let knots = if closed && degree > 1 {
            let parameter = |i: isize| {
                parameters[i.rem_euclid(n as isize) as usize]
                    + i.div_euclid(n as isize) as Real * period
            };
            let mut knots = (0..control_count + degree + 1)
                .map(|i| {
                    let start = i as isize - degree as isize - 1;
                    stable_knot_mean(
                        &(start..start + degree as isize)
                            .map(parameter)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let end = knots.len() - 1;
            knots[0] = knots[1];
            knots[end] = knots[end - 1];
            parameters = (0..n)
                .map(|i| stable_knot_mean(&knots[i + 1..=i + degree]))
                .collect::<Result<Vec<_>, _>>()?;
            knots
        } else if closed {
            let mut knots = vec![0.0];
            knots.extend_from_slice(&parameters);
            knots.push(period);
            parameters.pop();
            knots
        } else {
            let mut knots = vec![0.0; degree + 1];
            for i in 1..n - degree {
                knots.push(stable_knot_mean(&parameters[i..i + degree])?);
            }
            knots.extend(std::iter::repeat_n(period, degree + 1));
            knots
        };
        let mut matrix = Mat::zeros(n, n);
        let mut amplification = vec![0.0; n];
        for (i, &parameter) in parameters.iter().enumerate() {
            for (j, value) in
                bspline_basis_values_extended(&knots, degree, control_count, parameter)?
                    .into_iter()
                    .enumerate()
            {
                matrix[(i, j % n)] += value;
                amplification[i] += value.abs();
            }
        }
        Ok(Self {
            knots,
            parameters,
            control_count,
            amplification,
            matrix,
        })
    }

    pub fn solve(&self, targets: &Mat<Real>) -> Result<Mat<Real>, GeometryError> {
        let lu = self.matrix.full_piv_lu();
        if (0..self.matrix.nrows()).any(|i| !lu.U()[(i, i)].is_finite() || lu.U()[(i, i)] == 0.0) {
            return Err(GeometryError::SingularSystem);
        }
        let mut solution = lu.solve(targets);
        for j in 0..targets.ncols() {
            let constant = (0..targets.nrows()).all(|i| targets[(i, j)] == targets[(0, j)]);
            for i in 0..targets.nrows() {
                if constant {
                    solution[(i, j)] = targets[(0, j)];
                }
                require_finite([solution[(i, j)]], "point grid interpolation controls")?;
            }
        }
        Ok(solution)
    }
}
