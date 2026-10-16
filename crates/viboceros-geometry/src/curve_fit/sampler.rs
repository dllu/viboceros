//! Arc-length targets in either their natural or a common smooth parameter.
use super::*;
use crate::CurveSample;

#[derive(Clone, Copy)]
pub(super) struct FitKink {
    pub parameter: Real,
    pub incoming_tangent: crate::UnitVector3,
    pub outgoing_tangent: crate::UnitVector3,
}

pub(super) struct FitSampler<'a> {
    arc: ArcLengthSampler<'a>,
    map: Option<StationMap>,
}

impl<'a> FitSampler<'a> {
    pub fn try_new(
        arc: ArcLengthSampler<'a>,
        distances: &[Real],
        parameters: Option<&[Real]>,
    ) -> Result<Self, GeometryError> {
        let map = parameters
            .map(|parameters| StationMap::try_new(distances, parameters, arc.total_length()))
            .transpose()?;
        Ok(Self { arc, map })
    }

    pub fn end_parameter(&self) -> Real {
        self.map.as_ref().map_or(self.arc.total_length(), |_| 1.)
    }

    pub fn parameter_at_distance(&self, distance: Real) -> Result<Real, GeometryError> {
        self.map.as_ref().map_or(Ok(distance), |map| {
            map.inverse(distance / self.arc.total_length())
        })
    }

    fn distance_at_parameter(&self, parameter: Real) -> Result<Real, GeometryError> {
        match &self.map {
            None => Ok(parameter),
            Some(map) => match map.parameters.binary_search_by(|t| t.total_cmp(&parameter)) {
                Ok(i) => Ok(map.source_distances[i]),
                Err(_) => Ok(map.value(parameter)? * self.arc.total_length()),
            },
        }
    }

    pub fn point_at_parameter(&self, parameter: Real) -> Result<Point3, GeometryError> {
        self.arc
            .point_at_distance(self.distance_at_parameter(parameter)?)
    }

    pub fn sample_at_parameter(&self, parameter: Real) -> Result<CurveSample, GeometryError> {
        self.arc
            .sample_at_distance(self.distance_at_parameter(parameter)?)
    }

    pub fn speed_at_parameter(&self, parameter: Real) -> Result<Real, GeometryError> {
        let speed = match &self.map {
            None => 1.,
            Some(map) => {
                map.curve.evaluate_with_derivative(parameter)?.1.x() * self.arc.total_length()
            }
        };
        if speed.is_finite() && speed > 0. {
            Ok(speed)
        } else {
            Err(invalid(
                "parameter map has an unrepresentable or nonpositive speed",
            ))
        }
    }

    pub fn map_kinks(&self, source: &[ArcLengthKink]) -> Result<Vec<FitKink>, GeometryError> {
        source
            .iter()
            .map(|k| {
                Ok(FitKink {
                    parameter: self.parameter_at_distance(k.distance)?,
                    incoming_tangent: k.incoming_tangent,
                    outgoing_tangent: k.outgoing_tangent,
                })
            })
            .collect()
    }
}

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidCurveFitConstraints { context }
}

struct StationMap {
    curve: NurbsCurve,
    parameters: Vec<Real>,
    distances: Vec<Real>,
    source_distances: Vec<Real>,
}

impl StationMap {
    fn try_new(
        distances: &[Real],
        parameters: &[Real],
        length: Real,
    ) -> Result<Self, GeometryError> {
        if distances.len() != parameters.len()
            || !(2..=MAX_FIT_CONSTRAINTS).contains(&distances.len())
        {
            return Err(invalid(
                "a common parameter map requires 2 to 64 paired stations",
            ));
        }
        let mut stations = distances
            .iter()
            .copied()
            .zip(parameters.iter().copied())
            .collect::<Vec<_>>();
        stations.sort_by(|a, b| a.0.total_cmp(&b.0));
        if stations
            .iter()
            .any(|&(s, t)| !s.is_finite() || !t.is_finite())
            || stations[0] != (0., 0.)
            || stations[stations.len() - 1] != (length, 1.)
            || stations
                .windows(2)
                .any(|p| p[0].0 >= p[1].0 || p[0].1 >= p[1].1)
        {
            return Err(invalid(
                "common stations must increase from both source endpoints to [0,1]",
            ));
        }
        let parameters = stations.iter().map(|p| p.1).collect::<Vec<_>>();
        let source_distances = stations.iter().map(|p| p.0).collect::<Vec<_>>();
        let distances = stations.iter().map(|p| p.0 / length).collect::<Vec<_>>();
        if distances.windows(2).any(|p| p[0] >= p[1]) {
            return Err(invalid(
                "normalization collapsed distinct source length stations",
            ));
        }
        let degree = (stations.len() - 1).min(3);
        let mut knots = vec![0.; degree + 1];
        for j in 1..stations.len() - degree {
            knots.push(parameters[j..j + degree].iter().sum::<Real>() / degree as Real);
        }
        knots.extend(std::iter::repeat_n(1., degree + 1));
        if knots[degree..=stations.len()]
            .windows(2)
            .any(|p| p[0] >= p[1])
        {
            return Err(invalid(
                "averaging collapsed a smooth parameter-map knot span",
            ));
        }
        let system = crate::NurbsInterpolationSystem {
            degree,
            knots: &knots,
            parameters: &parameters,
            periodic: false,
            columns: 1,
            targets: &distances,
        };
        let mut controls = crate::surface_rebuild::portable_solve(&system)?;
        // Clamped endpoint equations independently fix these exact values.
        controls[0] = 0.;
        *controls.last_mut().unwrap() = 1.;
        // Positive derivative controls prove strict monotonicity everywhere.
        // A global interpolant that cannot prove this is declined, never clipped.
        if controls.windows(2).any(|p| p[0] >= p[1]) {
            return Err(invalid(
                "smooth station interpolant has nonpositive derivative controls",
            ));
        }
        let curve = NurbsCurve::try_new(
            degree,
            controls
                .into_iter()
                .map(|x| Point3::try_new(x, 0., 0.))
                .collect::<Result<Vec<_>, _>>()?,
            knots,
        )?;
        Ok(Self {
            curve,
            parameters,
            distances,
            source_distances,
        })
    }

    fn value(&self, parameter: Real) -> Result<Real, GeometryError> {
        if parameter == 0. || parameter == 1. {
            return Ok(parameter);
        }
        // Positive control increments and the exact 0/1 endpoints bound the
        // mathematical image. Clamp only floating evaluation roundoff.
        Ok(self.curve.evaluate(parameter)?.x().clamp(0., 1.))
    }

    fn inverse(&self, fraction: Real) -> Result<Real, GeometryError> {
        match self.distances.binary_search_by(|s| s.total_cmp(&fraction)) {
            Ok(i) => Ok(self.parameters[i]),
            Err(i) if i == 0 || i == self.distances.len() => {
                Err(invalid("source station is outside the parameter map"))
            }
            Err(i) => {
                let (mut a, mut b) = (self.parameters[i - 1], self.parameters[i]);
                for _ in 0..64 {
                    let m = stable_midpoint(a, b);
                    if m == a || m == b {
                        break;
                    }
                    if self.value(m)? < fraction {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                Ok(stable_midpoint(a, b))
            }
        }
    }
}
