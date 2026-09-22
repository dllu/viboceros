//! Input-only recipes. Never simplify the resulting trims or bypass validation.
use super::*;
use viboceros_geometry::{BrepLoop, BrepTrim, NurbsCurve2, WeightedPoint2};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(super) struct EndpointEncoding {
    degree: usize,
    endpoint_indices: Vec<usize>,
    weights: Vec<f64>,
    knots: Vec<f64>,
}

impl EndpointEncoding {
    pub(super) fn apply(&self, source: Brep, tolerance: Tolerance) -> Result<Brep, ProbeError> {
        let count = self.endpoint_indices.len();
        if !(1..=8).contains(&self.degree)
            || !(2..=64).contains(&count)
            || self.weights.len() != count
            || self.knots.len() != count + self.degree + 1
            || self.endpoint_indices.first() != Some(&0)
            || self.endpoint_indices.last() != Some(&1)
            || self.endpoint_indices.iter().any(|&i| i > 1)
        {
            return Err(ProbeError::FixtureInvariant(
                "invalid trim endpoint encoding",
            ));
        }
        let faces = source
            .faces()
            .iter()
            .map(|face| {
                let loops = face
                    .loops()
                    .iter()
                    .map(|boundary| {
                        let trims = boundary
                            .trims()
                            .iter()
                            .map(|trim| {
                                let controls = trim.curve().control_points();
                                if trim.curve().degree() != 1 || controls.len() != 2 {
                                    return Err(ProbeError::FixtureInvariant(
                                        "trim encoding needs two-control degree-one inputs",
                                    ));
                                }
                                let controls = self
                                    .endpoint_indices
                                    .iter()
                                    .zip(&self.weights)
                                    .map(|(&i, &w)| WeightedPoint2::try_new(controls[i].point(), w))
                                    .collect::<Result<Vec<_>, _>>()?;
                                let curve = NurbsCurve2::try_new_rational(
                                    self.degree,
                                    controls,
                                    self.knots.clone(),
                                )?;
                                Ok(BrepTrim::try_new(
                                    trim.vertices(),
                                    trim.edge(),
                                    trim.is_reversed_3d(),
                                    curve,
                                    trim.trim_type(),
                                    trim.iso(),
                                    trim.tolerance(),
                                )?)
                            })
                            .collect::<Result<Vec<_>, ProbeError>>()?;
                        Ok(BrepLoop::try_new(boundary.loop_type(), trims)?)
                    })
                    .collect::<Result<Vec<_>, ProbeError>>()?;
                Ok(BrepFace::try_new(
                    face.surface().clone(),
                    face.is_reversed(),
                    loops,
                )?)
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        Ok(Brep::try_new(
            source.vertices().to_vec(),
            source.edges().to_vec(),
            faces,
            tolerance,
        )?)
    }
}
