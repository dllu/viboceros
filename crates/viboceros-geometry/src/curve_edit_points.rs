//! Greville editing with original knots/weights and shared interpolation backends.
use crate::{
    FixedInterpolationSystem, GeometryError, NurbsCurve, NurbsInterpolationSystem, ParameterSide,
    Point3, Real, Tolerance, WeightedPoint3, spline_collocation::Axis,
};
use faer::Mat;

#[cfg(test)]
mod tests;
const MAX_CONTROLS: usize = 1000;
const MAX_DEGREE: usize = 17;

#[derive(Clone, Copy, Debug)]
pub struct CurveEditPoint {
    pub parameter: Real,
    pub side: ParameterSide,
    pub point: Point3,
}
/// Original-basis interpolation. A periodic job includes its final domain
/// endpoint without a repeated target row and supplies the native control shift.
pub struct CurveEditPointInterpolation<'a> {
    pub system: FixedInterpolationSystem<'a>,
    pub periodic_shift: Option<usize>,
}

struct Layout {
    parameters: Vec<Real>,
    sides: Vec<ParameterSide>,
    fixed: Vec<u8>,
    shift: Option<usize>,
}
impl Layout {
    fn new(curve: &NurbsCurve) -> Result<Self, GeometryError> {
        let n = curve.control_points().len();
        if n > MAX_CONTROLS || curve.degree() > MAX_DEGREE {
            return Err(GeometryError::UnsupportedCurveEditPoints {
                context: "degree/count limit",
            });
        }
        let sign = curve.control_points()[0].weight().is_sign_positive();
        if curve
            .control_points()
            .iter()
            .any(|c| c.weight().is_sign_positive() != sign)
        {
            return Err(GeometryError::UnsupportedCurveEditPoints {
                context: "mixed-sign weights",
            });
        }
        if curve.is_periodic() {
            let unique = n - curve.degree();
            if (0..curve.degree())
                .any(|i| curve.control_points()[i] != curve.control_points()[unique + i])
            {
                return Err(GeometryError::UnsupportedCurveEditPoints {
                    context: "periodic tails must repeat exactly",
                });
            }
            let (parameters, shift) = crate::nurbs::periodic_greville_parameters(
                curve.degree(),
                n,
                unique,
                curve.knots(),
            )?;
            if parameters.windows(2).any(|p| p[0] >= p[1]) {
                return Err(GeometryError::UnsupportedCurveEditPoints {
                    context: "periodic Greville stations must advance",
                });
            }
            return Ok(Self {
                sides: vec![ParameterSide::Right; unique],
                fixed: vec![0; unique],
                parameters,
                shift: Some(shift),
            });
        }
        let domain = curve.domain();
        if curve.knots()[..=curve.degree()]
            .iter()
            .any(|k| k != domain.start())
            || curve.knots()[n..].iter().any(|k| k != domain.end())
        {
            return Err(GeometryError::UnsupportedCurveEditPoints {
                context: "non-periodic input must be clamped",
            });
        }
        let axis = Axis::sites(curve.degree(), curve.knots().to_vec())?;
        if axis.stations.windows(2).any(|pair| {
            pair[0].parameter > pair[1].parameter
                || (pair[0].parameter == pair[1].parameter
                    && !(pair[0].fixed && pair[1].fixed && pair[0].side != pair[1].side))
        }) {
            return Err(GeometryError::UnsupportedCurveEditPoints {
                context: "Greville stations are not independently representable",
            });
        }
        Ok(Self {
            parameters: axis.stations.iter().map(|s| s.parameter).collect(),
            sides: axis.stations.iter().map(|s| s.side).collect(),
            fixed: axis.stations.iter().map(|s| u8::from(s.fixed)).collect(),
            shift: None,
        })
    }
}
impl NurbsCurve {
    /// Sided Greville edit locations. Periodic curves omit the repeated tail.
    /// Clamped open/closed and exact-tail periodic inputs are supported through
    /// degree 17 and 1000 stored controls; mixed weights/unclamped open inputs decline.
    pub fn edit_points(&self) -> Result<Vec<CurveEditPoint>, GeometryError> {
        let layout = Layout::new(self)?;
        layout
            .parameters
            .iter()
            .zip(&layout.sides)
            .map(|(&parameter, &side)| {
                Ok(CurveEditPoint {
                    parameter,
                    side,
                    point: self.evaluate_on_side(parameter, side)?,
                })
            })
            .collect()
    }
    pub fn try_with_edit_points(
        &self,
        points: &[Point3],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.try_with_edit_points_with_solver(points, tolerance, &mut portable)
    }
    /// Keep degree, knots and weights, delegating only the original-basis solve.
    /// Exact rational station checks admit the reconstructed output atomically.
    pub fn try_with_edit_points_with_solver<E: From<GeometryError>>(
        &self,
        points: &[Point3],
        tolerance: Tolerance,
        solve: &mut impl FnMut(&CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        let layout = Layout::new(self)?;
        let n = layout.parameters.len();
        if points.len() != n {
            return Err(GeometryError::UnsupportedCurveEditPoints {
                context: "edit point count differs from source layout",
            }
            .into());
        }
        let origin = points[0].to_array();
        let scale = self
            .control_points()
            .iter()
            .map(|c| c.weight().abs())
            .fold(0., f64::max);
        let mut targets = Vec::with_capacity(n * 4);
        for (i, &t) in layout.parameters.iter().enumerate() {
            let weight = if layout.fixed[i] != 0 {
                self.control_points()[i].weight() / scale
            } else {
                self.evaluate_scaled_homogeneous(t, scale)?[3]
            };
            if weight == 0. || !weight.is_finite() {
                return Err(GeometryError::CurveEditPointInterpolationFailed.into());
            }
            for (p, o) in points[i].to_array().into_iter().zip(origin) {
                targets.push((p - o) * weight);
            }
            targets.push(weight);
        }
        crate::require_finite(targets.iter().copied(), "edit-point homogeneous targets")?;
        let mut parameters = layout.parameters.clone();
        if layout.shift.is_some() {
            parameters.push(*self.domain().end());
        }
        let job = CurveEditPointInterpolation {
            system: FixedInterpolationSystem {
                system: NurbsInterpolationSystem {
                    degree: self.degree(),
                    knots: self.knots(),
                    parameters: &parameters,
                    periodic: layout.shift.is_some(),
                    columns: 4,
                    targets: &targets,
                },
                fixed_rows: &layout.fixed,
            },
            periodic_shift: layout.shift,
        };
        let solution = solve(&job)?;
        if solution.len() != targets.len() {
            return Err(GeometryError::CurveEditPointInterpolationFailed.into());
        }
        crate::require_finite(solution.iter().copied(), "edit-point solution")?;
        let controls = (0..self.control_points().len())
            .map(|i| {
                let j = i % n;
                let weight = self.control_points()[i].weight();
                let normalized = weight / scale;
                if normalized == 0. {
                    return Err(GeometryError::CurveEditPointInterpolationFailed);
                }
                let point = if layout.shift.is_none() && layout.fixed[j] != 0 {
                    points[j]
                } else {
                    Point3::try_from(std::array::from_fn(|k| {
                        solution[j * 4 + k] / normalized + origin[k]
                    }))?
                };
                WeightedPoint3::try_new(point, weight)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let result = Self::try_new_rational(self.degree(), controls, self.knots().to_vec())?;
        let stations = layout
            .parameters
            .iter()
            .zip(&layout.sides)
            .zip(points)
            .map(|((&t, &s), &p)| (t, s, p))
            .collect::<Vec<_>>();
        if !crate::surface_pullback::edit_point_bounds(&result, &stations, tolerance.absolute())? {
            return Err(GeometryError::CurveEditPointInterpolationFailed.into());
        }
        Ok(result)
    }
}

fn portable(job: &CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, GeometryError> {
    let system = &job.system.system;
    if let Some(shift) = job.periodic_shift {
        return crate::nurbs::portable_periodic_interpolation(system, shift);
    }
    let axis = Axis::new(system.degree, system.knots.to_vec())?;
    let rhs = Mat::from_fn(axis.stations.len(), 4, |i, j| system.targets[i * 4 + j]);
    let result = axis.solve(rhs)?;
    Ok((0..result.nrows())
        .flat_map(|i| {
            let result = &result;
            (0..4).map(move |j| result[(i, j)])
        })
        .collect())
}
