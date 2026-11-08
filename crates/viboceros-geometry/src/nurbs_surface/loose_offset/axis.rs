//! Sparse original-basis Greville layouts, including cyclic control tails.
use super::*;
use crate::spline_collocation::{Axis, Station};

pub(super) struct Layout {
    pub stations: Vec<Station>,
    pub rows: Vec<(usize, Vec<Real>)>,
    degree: usize,
    shift: Option<usize>,
}

impl Layout {
    pub fn new(degree: usize, knots: &[Real], periodic: bool) -> Result<Self, GeometryError> {
        let stored = knots.len() - degree - 1;
        let (stations, shift) = if periodic {
            let (parameters, shift) =
                crate::nurbs::periodic_greville_parameters(degree, stored, stored - degree, knots)?;
            (
                parameters
                    .into_iter()
                    .map(|parameter| Station {
                        parameter,
                        side: ParameterSide::Right,
                        fixed: false,
                    })
                    .collect(),
                Some(shift),
            )
        } else {
            if knots[..=degree].iter().any(|&k| k != knots[degree])
                || knots[stored..].iter().any(|&k| k != knots[stored])
            {
                return Err(unsupported("open input must be clamped"));
            }
            (Axis::sites(degree, knots.to_vec())?.stations, None)
        };
        if stations.windows(2).any(|s| {
            s[0].parameter > s[1].parameter
                || (s[0].parameter == s[1].parameter
                    && !(s[0].fixed && s[1].fixed && s[0].side != s[1].side))
        }) {
            return Err(unsupported("Greville stations are not independent"));
        }
        let rows = stations
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if s.fixed {
                    Ok((i, vec![1.]))
                } else {
                    crate::nurbs::bspline_basis_nonzero(knots, degree, stored, s.parameter)
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            stations,
            rows,
            degree,
            shift,
        })
    }
    pub fn len(&self) -> usize {
        self.stations.len()
    }
    pub fn solve(&self, rhs: Mat<Real>) -> Result<Mat<Real>, GeometryError> {
        if let Some(shift) = self.shift {
            crate::spline_collocation::cyclic::solve(self.degree, &self.rows, shift, rhs)
        } else {
            crate::spline_collocation::general_banded::solve(self.degree, &self.rows, rhs)
        }
    }
}
