//! Blend profiles across rail corners with no authored section.
use super::*;
use crate::{ArcLengthSampler, spline_collocation::Axis};

mod tilt;

struct Profile {
    section: SweepSection,
    distance: Real,
    incoming: Vec<WeightedPoint3>,
    outgoing: Vec<WeightedPoint3>,
}

struct Leg {
    frame: Frame3,
    finish: Point3,
    domain: RangeInclusive<Real>,
    warping: tilt::LegTilt,
}

pub(super) struct Model {
    legs: Vec<Leg>,
    profiles: Vec<Profile>,
    blend: SweepBlend,
    tolerance: Tolerance,
}

impl Model {
    pub(super) fn new(
        source: &NurbsCurve,
        domain: RangeInclusive<Real>,
        corners: &[Real],
        authored: &[Option<usize>],
        mut sections: Vec<SweepSection>,
        options: SweepProfileMiterOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if let Some(pole) = options.ends.start {
            sections.insert(0, point_section(*domain.start(), pole, &sections[0].curve)?);
        }
        if let Some(pole) = options.ends.end {
            sections.push(point_section(
                *domain.end(),
                pole,
                &sections.last().unwrap().curve,
            )?);
        }
        let rail = CurveRef::NurbsCurve(source);
        let starts = std::iter::once(*domain.start())
            .chain(corners.iter().copied())
            .collect::<Vec<_>>();
        let frames = match options.style {
            SweepFrameStyle::Freeform => {
                rail.rotation_minimizing_frames(&starts, None, Default::default())?
            }
            SweepFrameStyle::Roadlike(axis) => axis_frames(rail, &starts, |_| Ok(axis))?,
        };
        let end = rail.evaluate(*domain.end())?;
        let mut sampler = ArcLengthSampler::try_new(rail, tolerance)?;
        sampler.prepare_budgeted_repeated_sampling(32)?;
        let offset = sampler.distance_at_parameter(*domain.start())?;
        let distance = |t| Ok::<_, GeometryError>(sampler.distance_at_parameter(t)? - offset);
        let mut tilts = vec![None; frames.len() + 1];
        for (i, pair) in frames.windows(2).enumerate() {
            if authored[i].is_none() {
                tilts[i + 1] = Some(tilt::Turn::new(pair[0].z_axis(), pair[1].z_axis())?);
            }
        }
        let mut legs = Vec::with_capacity(frames.len());
        let finish_distance = distance(*domain.end())?;
        for (i, &frame) in frames.iter().enumerate() {
            let finish = frames.get(i + 1).map_or(end, |next| next.origin());
            let s = distance(starts[i])?;
            let e = if let Some(&t) = starts.get(i + 1) {
                distance(t)?
            } else {
                finish_distance
            };
            let native_end = starts.get(i + 1).copied().unwrap_or(*domain.end());
            let curve = source.try_trimmed(starts[i]..=native_end)?;
            let tangent = frame.z_axis().as_vector();
            let extent = frame.origin().vector_to(finish)?.dot(tangent)?;
            let error = (extent - (e - s)).abs();
            if e <= s
                || error > tolerance.absolute()
                || curve.control_points().iter().any(|c| {
                    frame
                        .projected_coordinates_of(c.point())
                        .map_or(true, |p| p[0].hypot(p[1]) > tolerance.absolute())
                })
            {
                return Err(invalid(
                    "an inferred sweep miter requires straight rail legs",
                ));
            }
            legs.push(Leg {
                frame,
                finish,
                domain: s..=e,
                warping: tilt::LegTilt::new(tilts[i], tilts[i + 1])?,
            });
        }
        let profiles = sections
            .into_iter()
            .map(|section| {
                let origin = rail.evaluate(section.parameter)?;
                let incoming = starts
                    .partition_point(|&t| t < section.parameter)
                    .saturating_sub(1);
                let outgoing = starts
                    .partition_point(|&t| t <= section.parameter)
                    .saturating_sub(1);
                let local = |i: usize| {
                    let frame = frames[i].with_origin(origin);
                    section
                        .curve
                        .control_points()
                        .iter()
                        .map(|c| {
                            let point = Point3::try_from(frame.coordinates_of(c.point())?)?;
                            WeightedPoint3::try_new(point, c.weight())
                        })
                        .collect::<Result<Vec<_>, GeometryError>>()
                };
                Ok(Profile {
                    distance: distance(section.parameter)?,
                    incoming: local(incoming)?,
                    outgoing: local(outgoing)?,
                    section,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if profiles
            .windows(2)
            .any(|pair| pair[0].distance >= pair[1].distance)
        {
            return Err(invalid("miter profiles must advance in arc length"));
        }
        Ok(Self {
            legs,
            profiles,
            blend: options.blend,
            tolerance,
        })
    }

    pub(super) fn constant_profile(&self) -> Result<Option<&SweepSection>, GeometryError> {
        let first = &self.profiles[0];
        let slack = self.tolerance.absolute();
        for points in self
            .profiles
            .iter()
            .flat_map(|p| [&p.incoming, &p.outgoing])
        {
            for (a, b) in first.outgoing.iter().zip(points) {
                if a.point().z().abs() > slack
                    || a.point().distance_to(b.point())? > slack
                    || a.weight() != b.weight()
                {
                    return Ok(None);
                }
            }
        }
        Ok(Some(&first.section))
    }

    pub(super) fn fit<E: From<GeometryError>>(
        &self,
        operations: &mut SweepSurfaceOperations<'_, E>,
    ) -> Result<Vec<NurbsSurface>, E> {
        let width = self.profiles[0].outgoing.len();
        let mut panels = Vec::with_capacity(self.legs.len());
        let mut shared: Option<NurbsCurve> = None;
        let mut count = 1;
        for leg in &self.legs {
            let lo = *leg.domain.start();
            let hi = *leg.domain.end();
            let interior = &self.profiles[self.profiles.partition_point(|p| p.distance <= lo)
                ..self.profiles.partition_point(|p| p.distance < hi)];
            let constraints = std::iter::once(lo)
                .chain(interior.iter().map(|p| p.distance))
                .chain([hi])
                .collect::<Vec<_>>();
            let chart = NurbsCurve::try_new(
                3,
                [0., 1. / 3., 2. / 3., 1.]
                    .map(|u| Point3::try_new(u * (hi - lo), 0., 0.))
                    .into_iter()
                    .collect::<Result<_, _>>()?,
                [lo; 4].into_iter().chain([hi; 4]).collect(),
            )?;
            let chart = basis::with_section_stations(chart, &constraints, operations)?;
            count += chart.control_points().len() - 1;
            check_control_budget(count, width)?;
            let axis = Axis::sites(3, chart.knots().to_vec())?;
            let mut samples = axis
                .stations
                .iter()
                .map(|s| self.sample(leg, s.parameter, interior))
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(previous) = shared.take() {
                if !matching_sections(&previous, &samples[0], self.tolerance)? {
                    return Err(invalid(
                        "the rail frames yield different cross sections at the miter",
                    )
                    .into());
                }
                samples[0] = previous;
            }
            shared = samples.last().cloned();
            let weights = vec![1.; samples.len()];
            let panel = basis::interpolate_sections(axis, &samples, &weights, operations)?;
            weights::require_positive_denominator(&panel)?;
            for profile in interior {
                let actual = panel.isocurve_v(profile.distance)?;
                if !matching_sections(&actual, &profile.section.curve, self.tolerance)? {
                    return Err(
                        invalid("section interpolation lost a supplied miter profile").into(),
                    );
                }
            }
            panels.push(panel);
        }
        Ok(panels)
    }

    fn sample(
        &self,
        leg: &Leg,
        s: Real,
        interior: &[Profile],
    ) -> Result<NurbsCurve, GeometryError> {
        let next = self.profiles.partition_point(|p| p.distance < s);
        let pinned = self
            .profiles
            .get(next.saturating_sub(1)..(next + 1).min(self.profiles.len()))
            .into_iter()
            .flatten()
            .find(|p| basis::parameters_near(s, p.distance, leg.domain.clone()));
        if let Some(p) = pinned {
            return Ok(p.section.curve.clone());
        }
        let Some([a, b]) = next
            .checked_sub(1)
            .and_then(|i| self.profiles.get(i..i + 2))
        else {
            return Err(invalid("miter sample outside the profiled rail interval"));
        };
        let fraction = ((s - a.distance) / (b.distance - a.distance)).clamp(0., 1.);
        let f = if self.blend == SweepBlend::Local {
            fraction * fraction * (3. - 2. * fraction)
        } else {
            fraction
        };
        let lo = *leg.domain.start();
        let hi = *leg.domain.end();
        let tangent = leg.frame.z_axis().as_vector();
        let origin = if s == hi {
            leg.finish
        } else {
            leg.frame.origin().translated(tangent.scaled(s - lo)?)?
        };
        let authored_interval = interior
            .first()
            .zip(interior.last())
            .map(|(a, b)| [a.distance, b.distance]);
        let normal = leg
            .warping
            .normal(tangent, s, [lo, hi], authored_interval)?;
        let divisor = tangent.dot(normal)?;
        if divisor <= 64. * Real::EPSILON {
            return Err(invalid("inferred miter plane is parallel to a rail leg"));
        }
        let frame = leg.frame.with_origin(origin);
        let controls = a
            .outgoing
            .iter()
            .zip(&b.incoming)
            .map(|(a, b)| {
                let ([x, y, z], weight) = blend_control(*a, *b, f, BlendCoordinates::Homogeneous);
                let transverse = frame.point_at([x, y, 0.])?;
                let shift = -origin.vector_to(transverse)?.dot(normal)? / divisor;
                let point = transverse
                    .translated(tangent.scaled(shift)?)?
                    .translated(normal.scaled(z)?)?;
                WeightedPoint3::try_new(point, weight)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        NurbsCurve::try_new_rational(
            a.section.curve.degree(),
            controls,
            a.section.curve.knots().to_vec(),
        )
    }
}

fn point_section(
    parameter: Real,
    pole: Point3,
    profile: &NurbsCurve,
) -> Result<SweepSection, GeometryError> {
    let controls = profile
        .control_points()
        .iter()
        .map(|c| WeightedPoint3::try_new(pole, c.weight()))
        .collect::<Result<Vec<_>, _>>()?;
    let curve = NurbsCurve::try_new_rational(profile.degree(), controls, profile.knots().to_vec())?;
    Ok(SweepSection { parameter, curve })
}

fn matching_sections(
    a: &NurbsCurve,
    b: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if a.degree() != b.degree() || a.knots() != b.knots() {
        return Ok(false);
    }
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        let weight_error = (a.weight() - b.weight()).abs();
        if a.point().distance_to(b.point())? > tolerance.absolute()
            || weight_error > tolerance.relative() * a.weight().abs().max(b.weight().abs())
        {
            return Ok(false);
        }
    }
    Ok(true)
}
