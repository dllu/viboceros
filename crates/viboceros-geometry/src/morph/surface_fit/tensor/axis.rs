use super::*;
use std::ops::RangeInclusive;

pub(super) struct Axis {
    pub degree: usize,
    pub knots: Vec<Real>,
    pub stations: Vec<Row>,
}

impl Axis {
    pub fn cubic_composition(
        source_degree: usize,
        source_knots: &[Real],
        domain: RangeInclusive<Real>,
        maximum: usize,
    ) -> Option<Self> {
        let degree = source_degree.checked_mul(3)?;
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
        let count = knots.len() - degree - 1;
        if count > maximum {
            return None;
        }
        let stations = (0..count)
            .map(|i| {
                if degree == DEGREE {
                    return collocation_row(&knots, i).ok();
                }
                let fixed = knots[i + 1] == knots[i + degree];
                let t = if fixed {
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
                let side = if fixed && knots[i] < t {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                Some(([0.0; 5], t, side, fixed))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            degree,
            knots,
            stations,
        })
    }
}
