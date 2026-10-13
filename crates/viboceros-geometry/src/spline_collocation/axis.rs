//! One-dimensional collocation shared by curve and tensor-surface fitting.

use super::*;
use faer::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct Station {
    pub parameter: Real,
    pub side: ParameterSide,
    pub fixed: bool,
}

pub(crate) struct Axis {
    pub degree: usize,
    pub knots: Vec<Real>,
    pub stations: Vec<Station>,
    bands: Option<Vec<[Real; 5]>>,
}

impl Axis {
    pub fn for_solver(
        degree: usize,
        knots: Vec<Real>,
        solver: Option<crate::MorphInterpolationSolver>,
    ) -> Result<Self, GeometryError> {
        Self::prepare(degree, knots, solver.is_none())
    }

    pub fn cubic_composition_with_solver(
        source_degree: usize,
        source_knots: &[Real],
        domain: RangeInclusive<Real>,
        maximum: usize,
        solver: Option<crate::MorphInterpolationSolver>,
    ) -> Option<Self> {
        if !(1..=3).contains(&source_degree) {
            return None;
        }
        let degree = source_degree * 3;
        let mut knots = vec![*domain.start(); degree + 1];
        for group in source_knots.chunk_by(|a, b| a == b) {
            if group[0] > *domain.start() && group[0] < *domain.end() {
                knots.extend(std::iter::repeat_n(
                    group[0],
                    degree - source_degree + group.len(),
                ));
            }
        }
        knots.extend(std::iter::repeat_n(*domain.end(), degree + 1));
        if knots.len() - degree - 1 > maximum {
            return None;
        }
        Self::for_solver(degree, knots, solver).ok()
    }

    pub(crate) fn new(degree: usize, knots: Vec<Real>) -> Result<Self, GeometryError> {
        Self::prepare(degree, knots, true)
    }

    pub(crate) fn sites(degree: usize, knots: Vec<Real>) -> Result<Self, GeometryError> {
        Self::prepare(degree, knots, false)
    }

    fn prepare(degree: usize, knots: Vec<Real>, build_bands: bool) -> Result<Self, GeometryError> {
        let count = knots.len() - degree - 1;
        let mut stations = Vec::with_capacity(count);
        let mut bands = (build_bands && degree == DEGREE).then(|| Vec::with_capacity(count));
        for i in 0..count {
            let station = if let Some(bands) = bands.as_mut() {
                let (row, parameter, side, fixed) = collocation_row(&knots, i)?;
                bands.push(row);
                Station {
                    parameter,
                    side,
                    fixed,
                }
            } else if degree == DEGREE {
                let (parameter, side, fixed) = collocation_site(&knots, i)?;
                Station {
                    parameter,
                    side,
                    fixed,
                }
            } else {
                let fixed = knots[i + 1] == knots[i + degree];
                let parameter = if fixed {
                    knots[i + 1]
                } else {
                    let interior = &knots[i + 1..=i + degree];
                    let scale = interior.iter().map(|t| t.abs()).fold(0.0, Real::max);
                    if scale == 0.0 {
                        0.0
                    } else {
                        (interior
                            .iter()
                            .map(|t| t / scale / degree as Real)
                            .sum::<Real>())
                        .clamp(-1.0, 1.0)
                            * scale
                    }
                };
                let side = if fixed && knots[i] < parameter {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                Station {
                    parameter,
                    side,
                    fixed,
                }
            };
            stations.push(station);
        }
        Ok(Self {
            degree,
            knots,
            stations,
            bands,
        })
    }

    pub fn solve_with(
        &self,
        rhs: Mat<Real>,
        channels: usize,
        solver: Option<crate::MorphInterpolationSolver>,
    ) -> Result<Mat<Real>, GeometryError> {
        let Some(solver) = solver else {
            return self.solve(rhs);
        };
        let count = self.stations.len();
        if rhs.nrows() != count
            || rhs.ncols() == 0
            || ![3, 4].contains(&channels)
            || !rhs.ncols().is_multiple_of(channels)
        {
            return Err(GeometryError::InvalidControlNet {
                context: "morph interpolation target layout",
            });
        }
        let columns = rhs.ncols() / channels * 4;
        let targets = (0..count * columns)
            .map(|index| {
                let row = index / columns;
                let column = index % columns;
                if column % 4 >= channels {
                    1.
                } else {
                    rhs[(row, column / 4 * channels + column % 4)]
                }
            })
            .collect::<Vec<_>>();
        let parameters = self
            .stations
            .iter()
            .map(|s| s.parameter)
            .collect::<Vec<_>>();
        let fixed_rows = self
            .stations
            .iter()
            .map(|s| u8::from(s.fixed))
            .collect::<Vec<_>>();
        let solution = solver(&crate::FixedInterpolationSystem {
            system: crate::NurbsInterpolationSystem {
                degree: self.degree,
                knots: &self.knots,
                parameters: &parameters,
                periodic: false,
                columns,
                targets: &targets,
            },
            fixed_rows: &fixed_rows,
        })?;
        if solution.len() != targets.len() {
            return Err(GeometryError::InvalidControlNet {
                context: "morph interpolation backend returned an inconsistent layout",
            });
        }
        require_finite(
            solution.iter().copied(),
            "morph interpolation backend solution",
        )?;
        Ok(Mat::from_fn(count, rhs.ncols(), |row, column| {
            solution[row * columns + column / channels * 4 + column % channels]
        }))
    }

    pub fn solve(&self, rhs: Mat<Real>) -> Result<Mat<Real>, GeometryError> {
        if let Some(bands) = &self.bands {
            return super::solve(bands, rhs);
        }
        let count = self.stations.len();
        let mut matrix = Mat::zeros(count, count);
        for (i, station) in self.stations.iter().enumerate() {
            if station.fixed {
                matrix[(i, i)] = 1.0;
            } else {
                for (j, value) in
                    bspline_basis_values(&self.knots, self.degree, count, station.parameter)?
                        .into_iter()
                        .enumerate()
                {
                    matrix[(i, j)] = value;
                }
            }
        }
        let solution = matrix.full_piv_lu().solve(&rhs);
        require_finite(
            (0..solution.ncols())
                .flat_map(|j| (0..solution.nrows()).map(move |i| (i, j)))
                .map(|(i, j)| solution[(i, j)]),
            "morph collocation solution",
        )?;
        Ok(solution)
    }
}
