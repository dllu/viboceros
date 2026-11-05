//! Canonical edge gauges and post-partition cap parameterization.
use super::*;
impl Brep {
    /// Remove irrelevant common homogeneous gauges from polynomial edges.
    /// Euclidean controls, knots, parameterization and all topology stay exact.
    pub fn try_normalized_polynomial_edge_weights(mut self) -> Result<Self, GeometryError> {
        for edge in &mut self.edges {
            if !edge.curve.is_rational() && edge.curve.control_points()[0].weight() != 1. {
                self.extrusion = None;
                edge.curve = NurbsCurve::try_new(
                    edge.curve.degree(),
                    edge.curve
                        .control_points()
                        .iter()
                        .map(|c| c.point())
                        .collect(),
                    edge.curve.knots().to_vec(),
                )?;
            }
        }
        Ok(self)
    }
    /// Native cap trims use projective end-weight normalization after all
    /// boundary subdivisions. Keep exact partition certificates on the raw
    /// restrictions first; then validate the normalized geometric loci.
    pub fn try_command_linear_cap_trim_weights(
        mut self,
        caps: [usize; 2],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        for index in caps {
            let face = self
                .faces
                .get_mut(index)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "missing extrusion cap",
                })?;
            for boundary in &mut face.loops {
                for trim in &mut boundary.trims {
                    let controls = trim.curve.control_points();
                    if controls[0].weight() == 1. && controls.last().unwrap().weight() == 1. {
                        continue;
                    }
                    let lifted = NurbsCurve::try_new_rational(
                        trim.curve.degree(),
                        controls
                            .iter()
                            .map(|c| {
                                WeightedPoint3::try_new(
                                    Point3::try_new(c.point().x(), c.point().y(), 0.)?,
                                    c.weight(),
                                )
                            })
                            .collect::<Result<Vec<_>, GeometryError>>()?,
                        trim.curve.knots().to_vec(),
                    )?;
                    let normalized =
                        lifted.try_trimmed_with_normalized_end_weights(lifted.domain())?;
                    trim.curve = NurbsCurve2::try_new_rational(
                        normalized.degree(),
                        normalized
                            .control_points()
                            .iter()
                            .map(|c| {
                                WeightedPoint2::try_new(
                                    Point2::try_new(c.point().x(), c.point().y())?,
                                    c.weight(),
                                )
                            })
                            .collect::<Result<Vec<_>, GeometryError>>()?,
                        normalized.knots().to_vec(),
                    )?;
                }
            }
        }
        self.extrusion = None;
        self.validate(tolerance)?;
        Ok(self)
    }
}
