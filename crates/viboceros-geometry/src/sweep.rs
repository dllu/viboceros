//! One-rail section placement and blending, independent of surface fitting.

use crate::{
    Curve3, CurveRef, Frame3, FrameTransportOptions, GeometryError, NurbsCurve, NurbsSurface,
    ParameterSide, Point3, Real, Tolerance, UnitVector3, Vector3, WeightedPoint3,
};
use std::ops::RangeInclusive;

mod basis;
mod fit;
#[cfg(test)]
mod tests;

const MAX_SECTIONS: usize = 256;
const MAX_SECTION_CONTROLS: usize = 512;
const MAX_AXIS_CONTROLS: usize = 1024;
const MAX_SURFACE_CONTROLS: usize = 262_144;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SweepFrameStyle {
    #[default]
    Freeform,
    Roadlike(UnitVector3),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SweepBlend {
    #[default]
    Local,
    Global,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SweepSection {
    pub parameter: Real,
    pub curve: NurbsCurve,
}

/// An ordered sweep definition. Section parameters are explicit native rail
/// parameters; profile directions and seams are retained, not guessed.
pub struct Sweep1 {
    rail: Curve3,
    sections: Vec<SweepSection>,
    local: Vec<Vec<WeightedPoint3>>,
    style: SweepFrameStyle,
    blend: SweepBlend,
    tolerance: Tolerance,
    domain: RangeInclusive<Real>,
    angular_tolerance: Real,
}

impl Sweep1 {
    pub fn try_new(
        rail: CurveRef<'_>,
        sections: &[SweepSection],
        style: SweepFrameStyle,
        blend: SweepBlend,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if sections.is_empty() || sections.len() > MAX_SECTIONS {
            return Err(invalid("expected 1 to 256 sections"));
        }
        if sections
            .iter()
            .any(|s| !s.parameter.is_finite() || !rail.domain().contains(&s.parameter))
            || sections
                .windows(2)
                .any(|p| p[0].parameter >= p[1].parameter)
        {
            return Err(invalid(
                "section rail parameters must be finite and strictly increasing",
            ));
        }
        let start = sections[0].parameter;
        let end = if sections.len() == 1 {
            *rail.domain().end()
        } else {
            sections.last().unwrap().parameter
        };
        if start >= end {
            return Err(invalid("empty swept rail interval"));
        }
        // A rail kink needs a miter construction, not an interpolated jump of
        // transported profiles. Reject it until that topology is constructed.
        let trimmed = rail.to_owned().try_trimmed(start..=end)?;
        if trimmed
            .as_ref()
            .to_nurbs()?
            .has_full_multiplicity_kink(tolerance)?
        {
            return Err(invalid("rail corners require sweep miter construction"));
        }
        if trimmed.as_ref().is_closed()? {
            return Err(invalid("closed rail sweep closure is not implemented"));
        }
        let curves = sections.iter().map(|s| s.curve.clone()).collect::<Vec<_>>();
        if curves
            .iter()
            .flat_map(|c| c.control_points())
            .any(|c| c.weight() <= 0.0)
        {
            return Err(invalid("sweep sections require positive rational weights"));
        }
        let curves = crate::section_basis::prepare(
            &curves,
            MAX_SECTION_CONTROLS,
            invalid("section control budget exceeded"),
        )?;
        let mut radius: Real = tolerance.absolute();
        for section in sections {
            let origin = rail.evaluate(section.parameter)?;
            for control in section.curve.control_points() {
                radius = radius.max(origin.distance_to(control.point())?);
            }
        }
        let mut result = Self {
            rail: rail.to_owned(),
            sections: sections
                .iter()
                .zip(curves)
                .map(|(s, curve)| SweepSection {
                    parameter: s.parameter,
                    curve,
                })
                .collect(),
            local: Vec::new(),
            style,
            blend,
            tolerance,
            domain: start..=end,
            angular_tolerance: (0.05 * (tolerance.absolute() / radius)).min(1e-10),
        };
        let parameters = result
            .sections
            .iter()
            .map(|s| s.parameter)
            .collect::<Vec<_>>();
        let frames = result.frames(&parameters)?;
        result.local = result
            .sections
            .iter()
            .zip(frames)
            .map(|(section, frame)| {
                section
                    .curve
                    .control_points()
                    .iter()
                    .map(|control| {
                        let delta = frame.origin().vector_to(control.point())?;
                        let coordinates = frame.axes().map(|axis| delta.dot(axis.as_vector()));
                        let [x, y, z] = coordinates;
                        WeightedPoint3::try_new(Point3::try_new(x?, y?, z?)?, control.weight())
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }

    pub fn domain(&self) -> RangeInclusive<Real> {
        self.domain.clone()
    }

    /// Exact rational cross-sections of the chosen frame/blending model.
    pub fn sections_at(&self, parameters: &[Real]) -> Result<Vec<NurbsCurve>, GeometryError> {
        if parameters.iter().any(|t| !self.domain.contains(t)) {
            return Err(invalid("sample outside swept interval"));
        }
        let frames = self.frames(parameters)?;
        let sampler = if self.sections.len() > 1 {
            let mut sampler =
                crate::curve::ArcLengthSampler::try_new(self.rail.as_ref(), self.tolerance)?;
            if parameters.len() > 16 {
                sampler.prepare_repeated_sampling(16)?;
            }
            Some(sampler)
        } else {
            None
        };
        let distances = self
            .sections
            .iter()
            .map(|s| {
                sampler.as_ref().map_or(Ok(0.0), |sampler| {
                    sampler.distance_at_parameter(s.parameter)
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        parameters
            .iter()
            .zip(frames)
            .map(|(&t, frame)| {
                let index = self
                    .sections
                    .partition_point(|s| s.parameter <= t)
                    .saturating_sub(1);
                let next = (index + 1).min(self.sections.len() - 1);
                let mut f = if index == next {
                    0.0
                } else {
                    (sampler.as_ref().unwrap().distance_at_parameter(t)? - distances[index])
                        / (distances[next] - distances[index])
                };
                if self.blend == SweepBlend::Local {
                    f = f * f * (3.0 - 2.0 * f);
                }
                let controls = self.local[index]
                    .iter()
                    .zip(&self.local[next])
                    .map(|(a, b)| {
                        let weight = a.weight().mul_add(1.0 - f, b.weight() * f);
                        let a = a.point().to_array().map(|x| x * a.weight());
                        let b = b.point().to_array().map(|x| x * b.weight());
                        let coordinates: [Real; 3] =
                            std::array::from_fn(|i| a[i].mul_add(1.0 - f, b[i] * f) / weight);
                        let axes = frame.axes().map(|a| a.as_vector().to_array());
                        let delta = Vector3::try_from(std::array::from_fn(|i| {
                            axes[0][i].mul_add(
                                coordinates[0],
                                axes[1][i].mul_add(coordinates[1], axes[2][i] * coordinates[2]),
                            )
                        }))?;
                        WeightedPoint3::try_new(frame.origin().translated(delta)?, weight)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                NurbsCurve::try_new_rational(
                    self.sections[0].curve.degree(),
                    controls,
                    self.sections[0].curve.knots().to_vec(),
                )
            })
            .collect()
    }

    /// Refits the rail to a cubic arc-length parameterization, then
    /// interpolates transported profiles at its Greville stations.
    pub fn to_surface(&self) -> Result<NurbsSurface, GeometryError> {
        basis::rail_basis(self, true)
    }

    /// Fits the continuous frame/blending model, not Rhino's fixed-basis sweep.
    /// U follows native rail parameters. Audits are sampled and bounded.
    pub fn fit_model_surface(&self) -> Result<NurbsSurface, GeometryError> {
        fit::fit(self)
    }

    /// Interpolates transported sections in the rail's existing rational
    /// basis. This preserves the rail basis, not a continuous rigid sweep
    /// between interpolation sites. Interior sections not already at Greville
    /// stations require a refit before adding interpolation sites.
    pub fn to_rail_basis_surface(&self) -> Result<NurbsSurface, GeometryError> {
        basis::rail_basis(self, false)
    }

    fn frames(&self, parameters: &[Real]) -> Result<Vec<Frame3>, GeometryError> {
        if parameters.is_empty() || parameters.windows(2).any(|p| p[0] >= p[1]) {
            return Err(invalid("frame parameters must be nonempty and increasing"));
        }
        let start = *self.domain.start();
        let mut ts = parameters.to_vec();
        let prepend = ts[0] > start;
        if prepend {
            ts.insert(0, start);
        }
        let mut frames = match self.style {
            SweepFrameStyle::Freeform => self.rail.as_ref().rotation_minimizing_frames(
                &ts,
                None,
                FrameTransportOptions {
                    angular_tolerance: self.angular_tolerance,
                    side: ParameterSide::Right,
                    ..Default::default()
                },
            )?,
            SweepFrameStyle::Roadlike(axis) => ts
                .iter()
                .map(|&t| {
                    let sample = self.rail.as_ref().evaluate_with_tangent(t)?;
                    let x = sample
                        .tangent()
                        .as_vector()
                        .cross(axis.as_vector())?
                        .normalized_nonzero()?;
                    let y = sample.tangent().as_vector().cross(x.as_vector())?;
                    Frame3::try_from_directions(
                        sample.point(),
                        x.as_vector(),
                        y,
                        Tolerance::try_new(
                            Real::MIN_POSITIVE,
                            64.0 * Real::EPSILON,
                            64.0 * Real::EPSILON,
                        )?,
                    )
                })
                .collect::<Result<Vec<_>, GeometryError>>()?,
        };
        if prepend {
            frames.remove(0);
        }
        Ok(frames)
    }
}

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidSweep { context }
}
