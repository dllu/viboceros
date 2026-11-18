//! Circular rail samples and cubic control rows for Pipe.
use super::*;
use std::f64::consts::{PI, TAU};

pub(super) struct CircularRail {
    plane: Frame3,
    radius: Real,
    sweep: Real,
    span_count: usize,
    closed: bool,
    tolerance: Tolerance,
}

impl CircularRail {
    pub(super) fn from_curve(
        rail: CurveRef<'_>,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let (center, x, normal, radius, sweep) = match rail {
            CurveRef::Arc(arc) => (
                arc.center(),
                arc.x_axis(),
                arc.normal()?,
                arc.radius(),
                arc.sweep_radians(),
            ),
            CurveRef::Circle(circle) => (
                circle.center(),
                circle.x_axis(),
                circle.normal()?,
                circle.radius(),
                TAU,
            ),
            _ => return Ok(None),
        };
        let plane = Frame3::try_from_unit_x_and_normal(center, x, normal, tolerance)?;
        let span_count = rail.to_nurbs()?.control_points().len() / 2;
        Ok(Some(Self {
            plane,
            radius,
            sweep,
            span_count,
            closed: rail.is_closed()?,
            tolerance,
        }))
    }

    pub(super) fn cages(
        &self,
        first: &Profile,
        second: Option<&Profile>,
        options: PipeOptions,
    ) -> Result<Vec<ControlCage>, CommandError> {
        if !first.stations.is_empty() {
            return Err(CommandError::Usage(
                "Pipe SubD intermediate radii require a straight rail",
            ));
        }
        let varying = |p: &Profile| p.endpoints[0] != p.endpoints[1];
        let tapered = varying(first) || second.is_some_and(varying);
        if self.closed && (tapered || second.is_some()) {
            return Err(CommandError::Usage(
                "Pipe SubD on a closed circle takes one constant radius",
            ));
        }
        let cap = if self.closed {
            PipeCap::None
        } else {
            options.cap
        };
        let frames = [
            self.end_frame(0., tapered)?,
            self.end_frame(self.sweep, tapered)?,
        ];
        let stations = self.stations();
        assemble_cages(first, second, cap, |cage, profile, reverse| {
            let mut samples = self.samples(&stations, profile, options.blend)?;
            if self.closed {
                interpolate_periodic(&mut samples);
            } else {
                interpolate_control_stations(&mut samples);
            }
            let mut rings = Vec::with_capacity(samples.len());
            for [cx, cy, rx, ry, r] in samples {
                rings.push(cage.points.len() as u32);
                for [x, y] in OCTAGON {
                    let y = if reverse { -y } else { y };
                    let (radial, vertical) = if tapered { (-x, y) } else { (y, x) };
                    cage.points.push(self.plane.point_at([
                        cx + radial * rx,
                        cy + radial * ry,
                        vertical * r,
                    ])?);
                }
            }
            Ok((cage.join_wall(&rings, self.closed), frames))
        })
    }

    fn end_frame(&self, angle: Real, tapered: bool) -> Result<Frame3, GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let origin = self
            .plane
            .point_at([self.radius * cosine, self.radius * sine, 0.])?;
        let tangent = self.plane.vector_at([-sine, cosine, 0.])?;
        let x = if tapered {
            self.plane.vector_at([-cosine, -sine, 0.])?
        } else {
            self.plane.z_axis().as_vector()
        };
        Frame3::try_from_x_and_normal(origin, x, tangent, self.tolerance)
    }

    fn stations(&self) -> Vec<Real> {
        let angle = self.sweep / self.span_count as Real;
        // Even samples in the source rational span give a chord-length knot
        // grid. The second pass through each interval measures both half chords.
        let subdivisions = ((angle / (PI / 240.)).ceil() as usize).next_power_of_two();
        let mut knots = vec![0.];
        let mut distance = 0.;
        let (sine, cosine) = (angle * 0.5).sin_cos();
        for _ in 0..self.span_count {
            let mut prev = 0.;
            for i in 1..=2 * subdivisions {
                let t = i as Real / (2 * subdivisions) as Real;
                let next = 2. * (t * sine).atan2(1. + t * (cosine - 1.));
                distance += 2. * (0.5 * (next - prev)).sin();
                prev = next;
                if i % 2 == 0 {
                    knots.push(distance);
                }
            }
        }
        for knot in &mut knots {
            *knot /= distance;
        }
        if self.closed {
            // Insert one knot in the span on each side of the seam. Keep that
            // seam as the second vertex ring when forming the periodic grid.
            let last = knots.len() - 1;
            let before = knots[last - 1];
            let half_before = before.midpoint(1.);
            knots.pop();
            knots.push(half_before);
            knots.insert(1, 0.5 * knots[1]);
            knots.splice(..0, [before - 1., half_before - 1.]);
            knots.windows(3).map(mean).collect()
        } else {
            knots.splice(..0, [0.; 2]);
            knots.extend([1.; 2]);
            knots.windows(3).map(mean).collect()
        }
    }

    fn samples(
        &self,
        stations: &[Real],
        profile: &Profile,
        blend: SweepBlend,
    ) -> Result<Vec<[Real; 5]>, CommandError> {
        let [first, last] = profile.endpoints;
        if first.max(last) >= self.radius {
            return Err(CommandError::Usage(
                "Pipe radius must be smaller than the circular rail radius",
            ));
        }
        Ok(stations
            .iter()
            .map(|&f| {
                let weight = if blend == SweepBlend::Local {
                    f * f * (3. - 2. * f)
                } else {
                    f
                };
                let r = RADIUS_SCALE * (first + (last - first) * weight);
                let (sine, cosine) = (f * self.sweep).sin_cos();
                [
                    self.radius * cosine,
                    self.radius * sine,
                    r * cosine,
                    r * sine,
                    r,
                ]
            })
            .collect())
    }
}

fn mean(values: &[Real]) -> Real {
    (values[0] + values[1] + values[2]) / 3.
}

// The periodic cubic mask has diagonal 4 and links of 1. Remove the two
// corner links with a rank-one update and solve both systems in one pass.
fn interpolate_periodic(samples: &mut [[Real; 5]]) {
    let end = samples.len() - 1;
    let mut update = vec![0.; end + 1];
    let mut upper = vec![0.; end];
    update[0] = -0.5;
    upper[0] = 0.125;
    samples[0] = samples[0].map(|value| value * 0.75);
    for i in 1..=end {
        let divisor = (if i == end { 4.25 } else { 4. }) - upper[i - 1];
        if i != end {
            upper[i] = 1. / divisor;
        }
        let rhs = if i == end { 1. } else { 0. };
        update[i] = (rhs - update[i - 1]) / divisor;
        for j in 0..5 {
            samples[i][j] = (6. * samples[i][j] - samples[i - 1][j]) / divisor;
        }
    }
    for i in (0..end).rev() {
        update[i] -= upper[i] * update[i + 1];
        for j in 0..5 {
            samples[i][j] -= upper[i] * samples[i + 1][j];
        }
    }
    let divisor = 1. + update[0] - update[end] * 0.25;
    let coefficient: [Real; 5] =
        std::array::from_fn(|j| (samples[0][j] - samples[end][j] * 0.25) / divisor);
    for (row, update) in samples.iter_mut().zip(update) {
        for (value, coefficient) in row.iter_mut().zip(coefficient) {
            *value -= update * coefficient;
        }
    }
}
