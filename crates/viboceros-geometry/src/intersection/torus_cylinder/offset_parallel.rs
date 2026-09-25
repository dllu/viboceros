//! Cubic intersections of a ring torus and a finite parallel offset cylinder.

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
    offset: Real,
    radial_axis: [Real; 2],
}

#[derive(Clone, Copy)]
enum Branch {
    Full { sign: Real },
    Turned { start: Real, end: Real },
    CriticalOuter,
    CriticalInner,
    Crossing { sign: Real },
}

impl Branch {
    fn period(self) -> Real {
        match self {
            Self::CriticalOuter | Self::CriticalInner => 2.0 * TURN,
            _ => TURN,
        }
    }
}

#[derive(Clone, Copy)]
struct Sample {
    position: [Real; 3],
    derivative: [Real; 3],
}

pub(super) fn intersect(
    (torus_frame, major, minor): (Frame3, Real, Real),
    (cylinder_frame, cylinder_radius, cylinder_height): (Frame3, Real, Real),
    [offset_x, offset_y, cylinder_start]: [Real; 3],
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let offset = offset_x.hypot(offset_y);
    let section = Section {
        frame: torus_frame,
        major,
        minor,
        cylinder_radius,
        offset,
        radial_axis: [offset_x / offset, offset_y / offset],
    };
    let cylinder_axis = cylinder_frame.z_axis().as_vector();
    let torus_axis = torus_frame.z_axis().as_vector();
    let cylinder_end = torus_axis
        .dot(cylinder_axis)?
        .mul_add(cylinder_height, cylinder_start);
    let low = cylinder_start.min(cylinder_end);
    let high = cylinder_start.max(cylinder_end);
    if low > minor + fit_tolerance || high < -minor - fit_tolerance {
        return Ok(Vec::new());
    }

    let radial_minimum = (offset - cylinder_radius).abs();
    let radial_maximum = offset + cylinder_radius;
    let closest = major.clamp(radial_minimum, radial_maximum);
    let radial_miss = (closest - major).abs() - minor;
    if radial_miss > fit_tolerance {
        return Ok(Vec::new());
    }
    if radial_miss >= 0.0 {
        if low > fit_tolerance || high < -fit_tolerance {
            return Ok(Vec::new());
        }
        let angle = if (radial_minimum - major).abs() < (radial_maximum - major).abs() {
            std::f64::consts::PI
        } else {
            0.0
        };
        return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(
            section.frame.point_at(section.position(angle, 0.0))?,
        )]);
    }

    let outer = major + minor;
    let inner = major - minor;
    let critical_tolerance = (8.0 * Real::EPSILON * outer.max(radial_maximum))
        .max(fit_tolerance * fit_tolerance / (4.0 * minor));
    let critical_outer = (radial_maximum - outer).abs() <= critical_tolerance;
    let critical_inner = (radial_minimum - inner).abs() <= critical_tolerance;
    let at_outer_extremum = section.radicand(0.0);
    let at_inner_extremum = section.radicand(std::f64::consts::PI);
    let branches = if critical_outer && critical_inner {
        vec![
            Branch::Crossing { sign: 1.0 },
            Branch::Crossing { sign: -1.0 },
        ]
    } else if critical_outer && at_inner_extremum > 0.0 {
        vec![Branch::CriticalOuter]
    } else if critical_inner && at_outer_extremum > 0.0 {
        vec![Branch::CriticalInner]
    } else if critical_outer {
        let (start, end) = section.active_angular_intervals()[0];
        vec![
            Branch::Turned { start, end: 0.0 },
            Branch::Turned { start: 0.0, end },
        ]
    } else if critical_inner {
        let (start, end) = section.active_angular_intervals()[0];
        vec![
            Branch::Turned {
                start,
                end: std::f64::consts::PI,
            },
            Branch::Turned {
                start: std::f64::consts::PI,
                end,
            },
        ]
    } else {
        section
            .active_angular_intervals()
            .into_iter()
            .flat_map(|(start, end)| {
                if (end - start - TURN).abs() <= 32.0 * Real::EPSILON {
                    vec![Branch::Full { sign: 1.0 }, Branch::Full { sign: -1.0 }]
                } else {
                    vec![Branch::Turned { start, end }]
                }
            })
            .collect()
    };
    let mut events = Vec::new();
    for branch in branches {
        let active = section.active_parameter_intervals(branch, low, high);
        let cuts = section.parameter_cuts(branch, low, high);
        for parameter in cuts {
            let z = section.sample(branch, parameter).position[2];
            if (z - low).abs() <= fit_tolerance || (z - high).abs() <= fit_tolerance {
                let included = active.iter().any(|(start, end)| {
                    [
                        parameter - branch.period(),
                        parameter,
                        parameter + branch.period(),
                    ]
                    .into_iter()
                    .any(|value| {
                        value >= *start - 32.0 * Real::EPSILON
                            && value <= *end + 32.0 * Real::EPSILON
                    })
                });
                if !included {
                    let point = section
                        .frame
                        .point_at(section.sample(branch, parameter).position)?;
                    if !events.iter().any(|event| match event {
                        SurfaceSurfaceIntersectionEvent::Point(other) => point
                            .distance_to(*other)
                            .is_ok_and(|distance| distance <= fit_tolerance),
                        SurfaceSurfaceIntersectionEvent::Curve(_) => false,
                    }) {
                        events.push(SurfaceSurfaceIntersectionEvent::Point(point));
                    }
                }
            }
        }
        for (start, end) in active {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                section,
                branch,
                start,
                end,
                fit_tolerance,
            )?));
        }
    }
    Ok(events)
}

impl Section {
    fn radial(self, angle: Real) -> Real {
        let d = self.offset;
        let a = self.cylinder_radius;
        (d.mul_add(d, a.mul_add(a, 2.0 * d * a * angle.cos())))
            .max(0.0)
            .sqrt()
    }

    fn radicand(self, angle: Real) -> Real {
        let inner = self.major - self.minor;
        let outer = self.major + self.minor;
        let radial = self.radial(angle);
        let half_sine = (0.5 * angle).sin();
        let half_cosine = (0.5 * angle).cos();
        if half_sine.abs() < 0.1 {
            let maximum = self.offset + self.cylinder_radius;
            let deficit = 4.0 * self.offset * self.cylinder_radius * half_sine * half_sine
                / (maximum + radial);
            ((outer - maximum) + deficit) * ((maximum - inner) - deficit)
        } else if half_cosine.abs() < 0.1 {
            let minimum = (self.offset - self.cylinder_radius).abs();
            let denominator = radial + minimum;
            if denominator == 0.0 {
                return (outer - radial) * (radial - inner);
            }
            let increment =
                4.0 * self.offset * self.cylinder_radius * half_cosine * half_cosine / denominator;
            ((outer - minimum) - increment) * ((minimum - inner) + increment)
        } else {
            (outer - radial) * (radial - inner)
        }
    }

    fn position(self, angle: Real, height: Real) -> [Real; 3] {
        let (sine, cosine) = angle.sin_cos();
        let [ux, uy] = self.radial_axis;
        let axial = self.offset + self.cylinder_radius * cosine;
        let lateral = self.cylinder_radius * sine;
        [
            axial.mul_add(ux, -lateral * uy),
            axial.mul_add(uy, lateral * ux),
            height,
        ]
    }

    fn sample(self, branch: Branch, parameter: Real) -> Sample {
        if matches!(
            branch,
            Branch::CriticalOuter | Branch::CriticalInner | Branch::Crossing { .. }
        ) {
            return self.sample_critical(branch, parameter);
        }
        let (angle, sign, angle_derivative, endpoint_height_derivative, root_approximation) =
            match branch {
                Branch::Full { sign } => (parameter, sign, 1.0, None, None),
                Branch::Turned { start, end } => {
                    let mut t = parameter.rem_euclid(TURN);
                    if parameter > 0.0 && t == 0.0 {
                        t = TURN;
                    }
                    let middle = 0.5 * (start + end);
                    let half = 0.5 * (end - start);
                    if t <= std::f64::consts::PI {
                        let angle = middle - half * t.cos();
                        let from_start = 2.0 * half * (0.5 * t).sin().powi(2);
                        let from_end = 2.0 * half * (0.5 * t).cos().powi(2);
                        let root_approximation = if from_start < Self::root_neighborhood(start) {
                            Some(self.radicand_near_root(start, from_start))
                        } else if from_end < Self::root_neighborhood(end) {
                            Some(self.radicand_near_root(end, -from_end))
                        } else {
                            None
                        };
                        let endpoint = if t == 0.0 {
                            Some(
                                (self.radicand_derivative(start) * half * 0.5)
                                    .max(0.0)
                                    .sqrt(),
                            )
                        } else if t == std::f64::consts::PI {
                            Some(
                                -(-self.radicand_derivative(end) * half * 0.5)
                                    .max(0.0)
                                    .sqrt(),
                            )
                        } else {
                            None
                        };
                        (angle, 1.0, half * t.sin(), endpoint, root_approximation)
                    } else {
                        let return_angle = t - std::f64::consts::PI;
                        let angle = middle + half * return_angle.cos();
                        let from_end = 2.0 * half * (0.5 * return_angle).sin().powi(2);
                        let from_start = 2.0 * half * (0.5 * return_angle).cos().powi(2);
                        let root_approximation = if from_end < Self::root_neighborhood(end) {
                            Some(self.radicand_near_root(end, -from_end))
                        } else if from_start < Self::root_neighborhood(start) {
                            Some(self.radicand_near_root(start, from_start))
                        } else {
                            None
                        };
                        let endpoint = if t == TURN {
                            Some(
                                (self.radicand_derivative(start) * half * 0.5)
                                    .max(0.0)
                                    .sqrt(),
                            )
                        } else {
                            None
                        };
                        (
                            angle,
                            -1.0,
                            -half * return_angle.sin(),
                            endpoint,
                            root_approximation,
                        )
                    }
                }
                _ => unreachable!("critical branches are evaluated separately"),
            };
        let (sine, cosine) = angle.sin_cos();
        let radicand = root_approximation.unwrap_or_else(|| self.radicand(angle).max(0.0));
        let height_root = radicand.sqrt();
        let height = sign * height_root;
        let height_derivative = endpoint_height_derivative.unwrap_or_else(|| {
            if height_root > 0.0 {
                sign * self.radicand_derivative(angle) * angle_derivative / (2.0 * height_root)
            } else {
                0.0
            }
        });
        let [ux, uy] = self.radial_axis;
        let x_derivative = self.cylinder_radius * (-sine * ux - cosine * uy) * angle_derivative;
        let y_derivative = self.cylinder_radius * (-sine * uy + cosine * ux) * angle_derivative;
        let axial = self.offset + self.cylinder_radius * cosine;
        let lateral = self.cylinder_radius * sine;
        Sample {
            position: [
                axial.mul_add(ux, -lateral * uy),
                axial.mul_add(uy, lateral * ux),
                height,
            ],
            derivative: [x_derivative, y_derivative, height_derivative],
        }
    }

    fn sample_critical(self, branch: Branch, parameter: Real) -> Sample {
        let (sine, cosine) = parameter.sin_cos();
        let radial = self.radial(parameter);
        let radial_derivative = -self.offset * self.cylinder_radius * sine / radial;
        let inner = self.major - self.minor;
        let outer = self.major + self.minor;
        let coefficient = 2.0 * (self.offset * self.cylinder_radius).sqrt();
        let (height, height_derivative) = match branch {
            Branch::CriticalOuter => {
                let factor = ((radial - inner) / (outer + radial)).max(0.0).sqrt();
                let factor_derivative = factor
                    * 0.5
                    * radial_derivative
                    * (1.0 / (radial - inner) - 1.0 / (outer + radial));
                let (half_sine, half_cosine) = (0.5 * parameter).sin_cos();
                (
                    coefficient * half_sine * factor,
                    coefficient * (0.5 * half_cosine * factor + half_sine * factor_derivative),
                )
            }
            Branch::CriticalInner => {
                let factor = ((outer - radial) / (radial + inner)).max(0.0).sqrt();
                let factor_derivative = -factor
                    * 0.5
                    * radial_derivative
                    * (1.0 / (outer - radial) + 1.0 / (radial + inner));
                let (half_sine, half_cosine) = (0.5 * parameter).sin_cos();
                (
                    coefficient * half_cosine * factor,
                    coefficient * (-0.5 * half_sine * factor + half_cosine * factor_derivative),
                )
            }
            Branch::Crossing { sign } => {
                let denominator = ((outer + radial) * (inner + radial)).sqrt();
                let denominator_derivative =
                    radial_derivative * (self.major + radial) / denominator;
                let coefficient = sign * 2.0 * self.offset * self.cylinder_radius;
                (
                    coefficient * sine / denominator,
                    coefficient
                        * (cosine / denominator
                            - sine * denominator_derivative / (denominator * denominator)),
                )
            }
            _ => unreachable!("smooth branches are evaluated separately"),
        };
        let [ux, uy] = self.radial_axis;
        let axial = self.offset + self.cylinder_radius * cosine;
        let lateral = self.cylinder_radius * sine;
        Sample {
            position: [
                axial.mul_add(ux, -lateral * uy),
                axial.mul_add(uy, lateral * ux),
                height,
            ],
            derivative: [
                self.cylinder_radius * (-sine * ux - cosine * uy),
                self.cylinder_radius * (-sine * uy + cosine * ux),
                height_derivative,
            ],
        }
    }

    fn radicand_derivative(self, angle: Real) -> Real {
        let radial = self.radial(angle);
        let radial_derivative = -self.offset * self.cylinder_radius * angle.sin() / radial;
        -2.0 * (radial - self.major) * radial_derivative
    }

    fn angle_for_radial(self, target: Real) -> Option<Real> {
        let minimum = (self.offset - self.cylinder_radius).abs();
        let maximum = self.offset + self.cylinder_radius;
        if target < minimum || target > maximum {
            return None;
        }
        let denominator = 4.0 * self.offset * self.cylinder_radius;
        let sine_squared = ((maximum - target) * (maximum + target) / denominator).clamp(0.0, 1.0);
        let cosine_squared =
            ((target - minimum) * (target + minimum) / denominator).clamp(0.0, 1.0);
        Some(if sine_squared <= 0.5 {
            2.0 * sine_squared.sqrt().asin()
        } else {
            std::f64::consts::PI - 2.0 * cosine_squared.sqrt().asin()
        })
    }

    fn radicand_near_root(self, angle: Real, delta: Real) -> Real {
        let first = self.radicand_derivative(angle) * delta;
        let second = if angle.sin().abs() <= 32.0 * Real::EPSILON {
            let radial = self.radial(angle);
            let radial_second = -self.offset * self.cylinder_radius * angle.cos() / radial;
            -(radial - self.major) * radial_second * delta * delta
        } else {
            0.0
        };
        (first + second).max(0.0)
    }

    fn root_neighborhood(angle: Real) -> Real {
        if angle.sin().abs() <= 32.0 * Real::EPSILON {
            1.0e-4
        } else {
            1.0e-8
        }
    }

    fn active_angular_intervals(self) -> Vec<(Real, Real)> {
        let mut cuts = vec![0.0, std::f64::consts::PI, TURN];
        for target in [self.major - self.minor, self.major + self.minor] {
            if let Some(angle) = self.angle_for_radial(target)
                && angle > 0.0
                && angle < std::f64::consts::PI
            {
                cuts.extend([angle, TURN - angle]);
            }
        }
        cuts.sort_by(Real::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
        let mut intervals: Vec<(Real, Real)> = Vec::new();
        for pair in cuts.windows(2) {
            if self.radicand(0.5 * (pair[0] + pair[1])) > 0.0 {
                if let Some(last) = intervals.last_mut()
                    && (last.1 - pair[0]).abs() <= 32.0 * Real::EPSILON
                {
                    last.1 = pair[1];
                } else {
                    intervals.push((pair[0], pair[1]));
                }
            }
        }
        if intervals.len() > 1
            && intervals[0].0 == 0.0
            && intervals.last().is_some_and(|last| last.1 == TURN)
        {
            let first = intervals.remove(0);
            let last = intervals.pop().expect("two active intervals");
            intervals.insert(0, (last.0 - TURN, first.1));
        }
        intervals
    }

    fn parameter_cuts(self, branch: Branch, low: Real, high: Real) -> Vec<Real> {
        let period = branch.period();
        let mut cuts = vec![0.0, period];
        if matches!(branch, Branch::Turned { .. }) {
            cuts.push(std::f64::consts::PI);
        }
        if matches!(branch, Branch::CriticalOuter | Branch::CriticalInner) {
            cuts.push(TURN);
        }
        for height in [low, high] {
            if height.abs() > self.minor {
                continue;
            }
            let radial_step = ((self.minor - height.abs()) * (self.minor + height.abs())).sqrt();
            for radial in [self.major - radial_step, self.major + radial_step] {
                let Some(angle) = self.angle_for_radial(radial) else {
                    continue;
                };
                for phi in [angle, TURN - angle] {
                    match branch {
                        Branch::Full { .. } | Branch::Crossing { .. } => cuts.push(phi),
                        Branch::CriticalOuter | Branch::CriticalInner => {
                            cuts.extend([phi, phi + TURN]);
                        }
                        Branch::Turned { start, end } => {
                            let middle = 0.5 * (start + end);
                            let half = 0.5 * (end - start);
                            for shifted in [phi - TURN, phi, phi + TURN] {
                                if shifted >= start && shifted <= end {
                                    let parameter =
                                        ((middle - shifted) / half).clamp(-1.0, 1.0).acos();
                                    cuts.extend([parameter, TURN - parameter]);
                                }
                            }
                        }
                    }
                }
            }
        }
        cuts.sort_by(Real::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
        cuts
    }

    fn active_parameter_intervals(
        self,
        branch: Branch,
        low: Real,
        high: Real,
    ) -> Vec<(Real, Real)> {
        let period = branch.period();
        let cuts = self.parameter_cuts(branch, low, high);
        let mut intervals: Vec<(Real, Real)> = Vec::new();
        for pair in cuts.windows(2) {
            let height = self.sample(branch, 0.5 * (pair[0] + pair[1])).position[2];
            if height >= low && height <= high {
                if let Some(last) = intervals.last_mut()
                    && (last.1 - pair[0]).abs() <= 32.0 * Real::EPSILON
                {
                    last.1 = pair[1];
                } else {
                    intervals.push((pair[0], pair[1]));
                }
            }
        }
        if intervals.len() > 1
            && intervals[0].0 == 0.0
            && intervals.last().is_some_and(|last| last.1 == period)
        {
            let first = intervals.remove(0);
            let last = intervals.pop().expect("two active intervals");
            intervals.insert(0, (last.0 - period, first.1));
        }
        intervals
    }
}

fn fit(
    section: Section,
    branch: Branch,
    start: Real,
    end: Real,
    tolerance: Real,
) -> Result<NurbsCurve, GeometryError> {
    let mut segments = Vec::new();
    let initial = if end - start > std::f64::consts::PI {
        8
    } else {
        4
    };
    for index in 0..initial {
        fit_interval(
            section,
            branch,
            start + (end - start) * index as Real / initial as Real,
            if index + 1 == initial {
                end
            } else {
                start + (end - start) * (index + 1) as Real / initial as Real
            },
            tolerance,
            0,
            &mut segments,
        )?;
    }
    let first = section
        .frame
        .point_at(section.sample(branch, start).position)?;
    let closed = (end - start - branch.period()).abs() <= 32.0 * Real::EPSILON;
    let mut controls = Vec::with_capacity(3 * segments.len() + 1);
    let mut knots = Vec::with_capacity(3 * segments.len() + 5);
    controls.push(first);
    knots.extend([start; 4]);
    let count = segments.len();
    for (index, (segment_end, control)) in segments.into_iter().enumerate() {
        controls.push(section.frame.point_at(control[1])?);
        controls.push(section.frame.point_at(control[2])?);
        controls.push(if closed && index + 1 == count {
            first
        } else {
            section.frame.point_at(control[3])?
        });
        knots.extend([segment_end; 3]);
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
}

fn fit_interval(
    section: Section,
    branch: Branch,
    start: Real,
    end: Real,
    tolerance: Real,
    depth: usize,
    segments: &mut Vec<(Real, [[Real; 3]; 4])>,
) -> Result<(), GeometryError> {
    let first = section.sample(branch, start);
    let last = section.sample(branch, end);
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
            let expected = section
                .sample(branch, start + (end - start) * fraction)
                .position;
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
    fit_interval(
        section,
        branch,
        start,
        middle,
        tolerance,
        depth + 1,
        segments,
    )?;
    fit_interval(section, branch, middle, end, tolerance, depth + 1, segments)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, Point3, Tolerance, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(center: Point3) -> Frame3 {
        Frame3::try_from_normal(
            center,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn offset_parallel_cylinders_form_full_and_turned_torus_sections() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (offset, radius, expected) in
            [(0.5, 4.0, 2), (2.0, 4.0, 2), (0.5, 5.0, 1), (0.5, 1.0, 0)]
        {
            let cylinder =
                NurbsSurface::try_cylinder(frame(point(offset, 0.0, -2.0)), radius, 0.0, 4.0)
                    .unwrap();
            for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("offset={offset} radius={radius}: {error:?}"));
                assert_eq!(events.len(), expected, "offset={offset} radius={radius}");
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("smooth offset torus/cylinder section should be a curve")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    for index in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        let torus_radial = location.x().hypot(location.y());
                        assert!(((torus_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                        assert!(
                            ((location.x() - offset).hypot(location.y()) - radius).abs() < 5e-9
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn offset_parallel_cylinder_clips_curves_to_both_rims() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        let cylinder =
            NurbsSurface::try_cylinder(frame(point(0.5, 0.0, 0.4)), 4.0, 0.0, 0.5).unwrap();
        let events =
            surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT).unwrap();
        assert!(!events.is_empty());
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite offset cylinder should create arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter = if index == 32 {
                    *domain.end()
                } else {
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0)
                };
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.z() >= 0.4 - 5e-9 && location.z() <= 0.9 + 5e-9);
                let torus_radial = location.x().hypot(location.y());
                assert!(((torus_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                assert!(((location.x() - 0.5).hypot(location.y()) - 4.0).abs() < 5e-9);
            }
        }
    }

    #[test]
    fn offset_parallel_cylinder_can_touch_torus_at_one_point() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        let cylinder =
            NurbsSurface::try_cylinder(frame(point(7.0, 0.0, -2.0)), 2.0, 0.0, 4.0).unwrap();
        let events =
            surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(touch)] = events.as_slice() else {
            panic!("tangent offset cylinder should touch the torus once: {events:?}")
        };
        assert!(touch.distance_to(point(5.0, 0.0, 0.0)).unwrap() < 5e-9);
    }

    #[test]
    fn offset_parallel_cylinder_rim_can_touch_two_height_maxima() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        let cylinder =
            NurbsSurface::try_cylinder(frame(point(0.5, 0.0, 1.0)), 4.0, 0.0, 1.0).unwrap();
        let events =
            surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(touch) = event else {
                panic!("cylinder rim should touch the upper torus section at points")
            };
            assert!((touch.z() - 1.0).abs() < 5e-9);
            assert!(((touch.x().hypot(touch.y()) - 4.0).hypot(touch.z()) - 1.0).abs() < 5e-9);
            assert!(((touch.x() - 0.5).hypot(touch.y()) - 4.0).abs() < 5e-9);
        }
    }

    #[test]
    fn critical_offset_cylinders_preserve_single_and_crossing_loops() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (offset, radius, expected) in [(0.5, 4.5, 1), (0.5, 3.5, 1), (1.0, 4.0, 2)] {
            let cylinder =
                NurbsSurface::try_cylinder(frame(point(offset, 0.0, -2.0)), radius, 0.0, 4.0)
                    .unwrap();
            let events = surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("offset={offset} radius={radius}: {error:?}"));
            assert_eq!(events.len(), expected);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("critical torus/cylinder section should be a loop")
                };
                assert!(curve.is_closed().unwrap());
                for index in 0..=64 {
                    let domain = curve.domain();
                    let parameter = if index == 64 {
                        *domain.end()
                    } else {
                        *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0)
                    };
                    let location = curve.evaluate(parameter).unwrap();
                    let torus_radial = location.x().hypot(location.y());
                    assert!(((torus_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!(((location.x() - offset).hypot(location.y()) - radius).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn critical_offset_cylinders_preserve_pinched_loops() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (offset, radius) in [(3.0, 2.0), (5.0, 2.0)] {
            let cylinder =
                NurbsSurface::try_cylinder(frame(point(offset, 0.0, -2.0)), radius, 0.0, 4.0)
                    .unwrap();
            let events = surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("offset={offset} radius={radius}: {error:?}"));
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("critical pinched section should be a loop")
                };
                assert!(curve.is_closed().unwrap());
                for index in 0..=32 {
                    let domain = curve.domain();
                    let parameter = if index == 32 {
                        *domain.end()
                    } else {
                        *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0)
                    };
                    let location = curve.evaluate(parameter).unwrap();
                    let torus_radial = location.x().hypot(location.y());
                    assert!(((torus_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!(((location.x() - offset).hypot(location.y()) - radius).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn offset_parallel_cylinder_handles_distant_rotated_and_opposed_axes() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let cylinder_center = rotated.point_at([0.5, 0.0, 2.0]).unwrap();
        let opposed = Frame3::try_from_normal(
            cylinder_center,
            Vector3::try_new(-1.0, -2.0, -3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(opposed, 4.0, 0.0, 4.0).unwrap();
        for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("rotated offset section should be a loop")
                };
                for index in 0..=32 {
                    let domain = curve.domain();
                    let parameter = if index == 32 {
                        *domain.end()
                    } else {
                        *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0)
                    };
                    let location = curve.evaluate(parameter).unwrap();
                    let local = rotated.coordinates_of(location).unwrap();
                    assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                    assert!(((local[0] - 0.5).hypot(local[1]) - 4.0).abs() < 4e-7);
                }
            }
        }
    }

    #[test]
    fn critical_offset_sections_clip_to_finite_height() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (offset, radius) in [(0.5, 4.5), (0.5, 3.5), (1.0, 4.0), (3.0, 2.0), (5.0, 2.0)] {
            let cylinder =
                NurbsSurface::try_cylinder(frame(point(offset, 0.0, 0.2)), radius, 0.0, 0.6)
                    .unwrap();
            let events = surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("offset={offset} radius={radius}: {error:?}"));
            assert!(!events.is_empty(), "offset={offset} radius={radius}");
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("finite critical section should be an arc")
                };
                assert!(!curve.is_closed().unwrap());
                for index in 0..=16 {
                    let domain = curve.domain();
                    let parameter = if index == 16 {
                        *domain.end()
                    } else {
                        *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0)
                    };
                    let location = curve.evaluate(parameter).unwrap();
                    assert!(location.z() >= 0.2 - 5e-9 && location.z() <= 0.8 + 5e-9);
                    let torus_radial = location.x().hypot(location.y());
                    assert!(((torus_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!(((location.x() - offset).hypot(location.y()) - radius).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn varied_parallel_offsets_stay_on_both_surfaces() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for offset in [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
            for radius in [1.0, 2.0, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0] {
                let cylinder =
                    NurbsSurface::try_cylinder(frame(point(offset, 0.0, -2.0)), radius, 0.0, 4.0)
                        .unwrap();
                let events =
                    surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT)
                        .unwrap_or_else(|error| {
                            panic!("offset={offset} radius={radius}: {error:?}")
                        });
                for event in events {
                    let location = match event {
                        SurfaceSurfaceIntersectionEvent::Point(point) => point,
                        SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                            let domain = curve.domain();
                            curve
                                .evaluate(0.5 * *domain.start() + 0.5 * *domain.end())
                                .unwrap()
                        }
                    };
                    let torus_radial = location.x().hypot(location.y());
                    assert!(
                        ((torus_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9,
                        "offset={offset} radius={radius}"
                    );
                    assert!(
                        ((location.x() - offset).hypot(location.y()) - radius).abs() < 5e-9,
                        "offset={offset} radius={radius}"
                    );
                }
            }
        }
    }

    #[test]
    fn near_critical_offsets_keep_distinct_loop_topology() {
        let torus = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (radius, expected) in [
            (4.5 - 1.0e-12, 2),
            (4.5 + 1.0e-12, 1),
            (3.5 + 1.0e-12, 2),
            (3.5 - 1.0e-12, 1),
        ] {
            let cylinder =
                NurbsSurface::try_cylinder(frame(point(0.5, 0.0, -2.0)), radius, 0.0, 4.0).unwrap();
            let events = surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("radius={radius}: {error:?}"));
            assert_eq!(events.len(), expected, "radius={radius}");
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("near-critical section should remain a loop")
                };
                for index in 0..=32 {
                    let domain = curve.domain();
                    let parameter = if index == 32 {
                        *domain.end()
                    } else {
                        *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0)
                    };
                    let location = curve.evaluate(parameter).unwrap();
                    let radial = location.x().hypot(location.y());
                    assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!(((location.x() - 0.5).hypot(location.y()) - radius).abs() < 5e-9);
                }
            }
        }
    }
}
