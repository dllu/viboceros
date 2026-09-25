//! Four branches where a perpendicular cylinder crosses the torus center.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Real};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
struct Section {
    frame: Frame3,
    major: Real,
    minor: Real,
    cylinder_radius: Real,
    direction: [Real; 2],
    axial_side: Real,
    branch_side: Real,
    regime: Regime,
}

#[derive(Clone, Copy)]
enum Regime {
    CylinderAngle,
    Meridian,
    Critical,
    InnerCritical,
    Turned {
        limit: Real,
    },
    FatInnerTurned {
        half_width: Real,
        root: Real,
        upper: Real,
    },
}

#[derive(Clone, Copy)]
struct Sample {
    position: [Real; 3],
    derivative: [Real; 3],
    axial: Real,
}

pub(super) fn intersect(
    (frame, major, minor): (Frame3, Real, Real),
    (cylinder_radius, cylinder_height): (Real, Real),
    (origin_x, origin_y, direction_x, direction_y): (Real, Real, Real, Real),
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let direction_length = direction_x.hypot(direction_y);
    let direction = [
        direction_x / direction_length,
        direction_y / direction_length,
    ];
    let start = origin_x.mul_add(direction[0], origin_y * direction[1]);
    let end = start + cylinder_height * direction_length;
    let low = start.min(end);
    let high = start.max(end);
    let section = |axial_side: Real, branch_side: Real, regime: Regime| Section {
        frame,
        major,
        minor,
        cylinder_radius,
        direction,
        axial_side,
        branch_side,
        regime,
    };
    let mut sections = Vec::with_capacity(4);
    if cylinder_radius > major - minor && cylinder_radius < minor {
        let root =
            (major * major + minor * minor - cylinder_radius * cylinder_radius) / (2.0 * major);
        let sine = (((minor - root) * (minor + root)).max(0.0)).sqrt() / cylinder_radius;
        let half_width = std::f64::consts::FRAC_PI_2 - sine.clamp(0.0, 1.0).asin();
        for axial_side in [1.0, -1.0] {
            sections.push(section(axial_side, 1.0, Regime::CylinderAngle));
        }
        for upper in [1.0, -1.0] {
            sections.push(section(
                1.0,
                -1.0,
                Regime::FatInnerTurned {
                    half_width,
                    root,
                    upper,
                },
            ));
        }
    } else {
        let regime = if cylinder_radius == major - minor {
            Regime::InnerCritical
        } else if cylinder_radius > major - minor {
            let cosine = (cylinder_radius * cylinder_radius - major * major - minor * minor)
                / (2.0 * major * minor);
            Regime::Turned {
                limit: cosine.clamp(-1.0, 1.0).acos(),
            }
        } else if cylinder_radius < minor {
            Regime::CylinderAngle
        } else if cylinder_radius > minor {
            Regime::Meridian
        } else {
            Regime::Critical
        };
        let axial_sides: &[Real] =
            if matches!(regime, Regime::Turned { .. } | Regime::InnerCritical) {
                &[1.0]
            } else {
                &[1.0, -1.0]
            };
        for &axial_side in axial_sides {
            for branch_side in [1.0, -1.0] {
                sections.push(section(axial_side, branch_side, regime));
            }
        }
    }
    let mut events = Vec::new();
    for section in sections {
        let period = section.period();
        let quarter = period / 4.0;
        let extrema = [
            section.sample(0.0).axial,
            section.sample(quarter).axial,
            section.sample(2.0 * quarter).axial,
            section.sample(3.0 * quarter).axial,
        ];
        let branch_low = extrema.into_iter().fold(Real::INFINITY, Real::min);
        let branch_high = extrema.into_iter().fold(Real::NEG_INFINITY, Real::max);
        if low > branch_high + fit_tolerance || high < branch_low - fit_tolerance {
            continue;
        }
        let mut cuts = vec![0.0, quarter, 2.0 * quarter, 3.0 * quarter, period];
        for quarter in 0..4 {
            let a = quarter as Real * period / 4.0;
            let b = (quarter + 1) as Real * period / 4.0;
            let at_a = section.sample(a).axial;
            let at_b = section.sample(b).axial;
            for rim in [low, high] {
                if rim > at_a.min(at_b) && rim < at_a.max(at_b) {
                    let mut left = a;
                    let mut right = b;
                    let increasing = at_b > at_a;
                    for _ in 0..60 {
                        let middle = 0.5 * (left + right);
                        if (section.sample(middle).axial < rim) == increasing {
                            left = middle;
                        } else {
                            right = middle;
                        }
                    }
                    cuts.push(0.5 * (left + right));
                }
            }
        }
        cuts.sort_by(Real::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() <= 32.0 * Real::EPSILON);
        let mut intervals: Vec<(Real, Real)> = Vec::new();
        for pair in cuts.windows(2) {
            let middle = 0.5 * (pair[0] + pair[1]);
            let axial = section.sample(middle).axial;
            if axial < low || axial > high {
                continue;
            }
            if let Some(last) = intervals.last_mut()
                && (last.1 - pair[0]).abs() <= 32.0 * Real::EPSILON
            {
                last.1 = pair[1];
            } else {
                intervals.push((pair[0], pair[1]));
            }
        }
        if intervals.len() > 1
            && intervals[0].0 == 0.0
            && intervals.last().is_some_and(|last| last.1 == period)
        {
            let first = intervals.remove(0);
            let last = intervals.pop().expect("at least two intervals");
            intervals.insert(0, (last.0 - period, first.1));
        }
        for &angle in &cuts {
            let axial = section.sample(angle).axial;
            let on_rim =
                (axial - low).abs() <= fit_tolerance || (axial - high).abs() <= fit_tolerance;
            let on_curve = intervals.iter().any(|&(a, b)| {
                [-period, 0.0, period]
                    .into_iter()
                    .any(|shift| angle + shift >= a && angle + shift <= b)
            });
            if on_rim && !on_curve {
                let point = section.frame.point_at(section.sample(angle).position)?;
                if !events.iter().any(|event| {
                        matches!(event, SurfaceSurfaceIntersectionEvent::Point(other)
                            if point.distance_to(*other).is_ok_and(|distance| distance <= fit_tolerance))
                    }) {
                        events.push(SurfaceSurfaceIntersectionEvent::Point(point));
                    }
            }
        }
        for (a, b) in intervals {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                section,
                a,
                b,
                fit_tolerance,
            )?));
        }
    }
    Ok(events)
}

impl Section {
    fn period(self) -> Real {
        if matches!(self.regime, Regime::InnerCritical) {
            2.0 * TURN
        } else {
            TURN
        }
    }

    fn sample(self, angle: Real) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        let (lateral, height, lateral_derivative, height_derivative, axial, axial_derivative) =
            match self.regime {
                Regime::CylinderAngle => {
                    let radius = self.cylinder_radius;
                    let lateral = radius * cosine;
                    let lateral_derivative = -radius * sine;
                    let height = radius * sine;
                    let height_derivative = radius * cosine;
                    let tube_radial = (self.minor * self.minor - height * height).sqrt();
                    let radial = self.major + self.branch_side * tube_radial;
                    let radial_derivative =
                        -self.branch_side * height * height_derivative / tube_radial;
                    let axial_magnitude =
                        ((radial - lateral.abs()) * (radial + lateral.abs())).sqrt();
                    let axial = self.axial_side * axial_magnitude;
                    let axial_derivative = self.axial_side
                        * (radial * radial_derivative - lateral * lateral_derivative)
                        / axial_magnitude;
                    (
                        lateral,
                        height,
                        lateral_derivative,
                        height_derivative,
                        axial,
                        axial_derivative,
                    )
                }
                Regime::Meridian | Regime::Critical => {
                    let height = self.minor * sine;
                    let height_derivative = self.minor * cosine;
                    let (lateral, lateral_derivative) = if matches!(self.regime, Regime::Critical) {
                        (
                            self.branch_side * self.minor * cosine,
                            -self.branch_side * height,
                        )
                    } else {
                        let lateral_magnitude =
                            (self.cylinder_radius * self.cylinder_radius - height * height).sqrt();
                        (
                            self.branch_side * lateral_magnitude,
                            -self.branch_side * height * height_derivative / lateral_magnitude,
                        )
                    };
                    let axial_magnitude = (self.major * self.major + self.minor * self.minor
                        - self.cylinder_radius * self.cylinder_radius
                        + 2.0 * self.major * self.minor * cosine)
                        .sqrt();
                    let axial = self.axial_side * axial_magnitude;
                    let axial_derivative =
                        -self.axial_side * self.major * self.minor * sine / axial_magnitude;
                    (
                        lateral,
                        height,
                        lateral_derivative,
                        height_derivative,
                        axial,
                        axial_derivative,
                    )
                }
                Regime::InnerCritical => {
                    let height = self.minor * sine;
                    let height_derivative = self.minor * cosine;
                    let lateral_magnitude =
                        (self.cylinder_radius * self.cylinder_radius - height * height).sqrt();
                    let lateral = self.branch_side * lateral_magnitude;
                    let lateral_derivative =
                        -self.branch_side * height * height_derivative / lateral_magnitude;
                    let amplitude = 2.0 * (self.major * self.minor).sqrt();
                    let axial = amplitude * (0.5 * angle).cos();
                    let axial_derivative = -0.5 * amplitude * (0.5 * angle).sin();
                    (
                        lateral,
                        height,
                        lateral_derivative,
                        height_derivative,
                        axial,
                        axial_derivative,
                    )
                }
                Regime::FatInnerTurned {
                    half_width,
                    root,
                    upper,
                } => {
                    let center = if upper > 0.0 {
                        std::f64::consts::FRAC_PI_2
                    } else {
                        3.0 * std::f64::consts::FRAC_PI_2
                    };
                    let cylinder_angle = center - half_width * cosine;
                    let cylinder_angle_derivative = half_width * sine;
                    let (cylinder_sine, cylinder_cosine) = cylinder_angle.sin_cos();
                    let height = self.cylinder_radius * cylinder_sine;
                    let lateral = self.cylinder_radius * cylinder_cosine;
                    let height_derivative =
                        self.cylinder_radius * cylinder_cosine * cylinder_angle_derivative;
                    let lateral_derivative =
                        -self.cylinder_radius * cylinder_sine * cylinder_angle_derivative;
                    let tube_radial = (self.minor * self.minor - height * height).sqrt();
                    // Rationalizing the radial root and using sinc avoids a 0/0
                    // derivative at both joins of the positive and negative sheets.
                    let a = half_width * (1.0 + cosine);
                    let b = half_width * (1.0 - cosine);
                    let axial = self.cylinder_radius
                        * half_width
                        * (2.0 * self.major / (root + tube_radial)).sqrt()
                        * sine
                        * (sinc(a) * sinc(b)).sqrt();
                    let axial_derivative = if sine.abs() <= 1.0e-8 {
                        let endpoint_scale = self.cylinder_radius
                            * (self.major * half_width * half_width.sin() * half_width.cos()
                                / root)
                                .sqrt();
                        cosine * endpoint_scale
                    } else {
                        let radial_derivative = -height * height_derivative / tube_radial;
                        -self.major * radial_derivative / axial
                    };
                    (
                        lateral,
                        height,
                        lateral_derivative,
                        height_derivative,
                        axial,
                        axial_derivative,
                    )
                }
                Regime::Turned { limit } => {
                    let meridian = limit * cosine;
                    let meridian_derivative = -limit * sine;
                    let (meridian_sine, meridian_cosine) = meridian.sin_cos();
                    let height = self.minor * meridian_sine;
                    let height_derivative = self.minor * meridian_cosine * meridian_derivative;
                    let lateral_magnitude =
                        (self.cylinder_radius * self.cylinder_radius - height * height).sqrt();
                    let lateral = self.branch_side * lateral_magnitude;
                    let lateral_derivative =
                        -self.branch_side * height * height_derivative / lateral_magnitude;
                    // The sinc form keeps the joined branch smooth at both axial turnarounds.
                    let a = 0.5 * limit * (1.0 + cosine);
                    let b = 0.5 * limit * (1.0 - cosine);
                    let axial = (self.major * self.minor).sqrt()
                        * limit
                        * sine
                        * (sinc(a) * sinc(b)).sqrt();
                    let axial_derivative = if sine.abs() <= 1.0e-8 {
                        cosine * (self.major * self.minor * limit * limit.sin()).sqrt()
                    } else {
                        -self.major * self.minor * meridian_sine * meridian_derivative / axial
                    };
                    (
                        lateral,
                        height,
                        lateral_derivative,
                        height_derivative,
                        axial,
                        axial_derivative,
                    )
                }
            };
        let [ux, uy] = self.direction;
        Sample {
            position: [
                axial.mul_add(ux, -lateral * uy),
                axial.mul_add(uy, lateral * ux),
                height,
            ],
            derivative: [
                axial_derivative.mul_add(ux, -lateral_derivative * uy),
                axial_derivative.mul_add(uy, lateral_derivative * ux),
                height_derivative,
            ],
            axial,
        }
    }
}

fn sinc(value: Real) -> Real {
    if value.abs() < 1.0e-4 {
        let square = value * value;
        1.0 - square / 6.0 + square * square / 120.0
    } else {
        value.sin() / value
    }
}

fn fit(
    section: Section,
    start: Real,
    end: Real,
    tolerance: Real,
) -> Result<NurbsCurve, GeometryError> {
    let mut segments = Vec::new();
    let count = ((end - start) / (TURN / 8.0)).ceil().max(1.0) as usize;
    for index in 0..count {
        fit_interval(
            section,
            start + (end - start) * index as Real / count as Real,
            start + (end - start) * (index + 1) as Real / count as Real,
            tolerance,
            0,
            &mut segments,
        )?;
    }
    let first = section.frame.point_at(section.sample(start).position)?;
    let closed = (end - start - section.period()).abs() <= 64.0 * Real::EPSILON;
    let mut controls = Vec::with_capacity(3 * segments.len() + 1);
    let mut knots = Vec::with_capacity(3 * segments.len() + 5);
    controls.push(first);
    knots.extend([start; 4]);
    let count = segments.len();
    for (index, (parameter, control)) in segments.into_iter().enumerate() {
        controls.push(section.frame.point_at(control[1])?);
        controls.push(section.frame.point_at(control[2])?);
        controls.push(if closed && index + 1 == count {
            first
        } else {
            section.frame.point_at(control[3])?
        });
        knots.extend([parameter; 3]);
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
}

fn fit_interval(
    section: Section,
    start: Real,
    end: Real,
    tolerance: Real,
    depth: usize,
    segments: &mut Vec<(Real, [[Real; 3]; 4])>,
) -> Result<(), GeometryError> {
    let first = section.sample(start);
    let last = section.sample(end);
    let handle = (end - start) / 3.0;
    let control = [
        first.position,
        std::array::from_fn(|i| first.derivative[i].mul_add(handle, first.position[i])),
        std::array::from_fn(|i| (-last.derivative[i]).mul_add(handle, last.position[i])),
        last.position,
    ];
    let accurate = [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875]
        .into_iter()
        .all(|fraction| {
            let expected = section.sample(start + (end - start) * fraction).position;
            let actual = bezier(control, fraction);
            (actual[0] - expected[0])
                .hypot(actual[1] - expected[1])
                .hypot(actual[2] - expected[2])
                <= tolerance * 0.25
        });
    if accurate {
        if segments.len() >= MAX_SEGMENTS {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        segments.push((end, control));
        return Ok(());
    }
    if depth >= 32 || segments.len() >= MAX_SEGMENTS || end - start <= 16.0 * Real::EPSILON {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let middle = 0.5 * (start + end);
    fit_interval(section, start, middle, tolerance, depth + 1, segments)?;
    fit_interval(section, middle, end, tolerance, depth + 1, segments)
}

fn bezier(control: [[Real; 3]; 4], fraction: Real) -> [Real; 3] {
    let complement = 1.0 - fraction;
    let basis = [
        complement.powi(3),
        3.0 * complement * complement * fraction,
        3.0 * complement * fraction * fraction,
        fraction.powi(3),
    ];
    std::array::from_fn(|i| {
        basis
            .iter()
            .zip(control)
            .map(|(weight, point)| weight * point[i])
            .sum()
    })
}
