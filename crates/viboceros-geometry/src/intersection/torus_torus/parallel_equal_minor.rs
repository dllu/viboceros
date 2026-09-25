//! Parallel ring tori at one height with equal tube radii and unequal ring radii.
//!
//! Subtracting the two torus equations factors into fixed difference or fixed
//! sum of distances to their projected ring centers. These are respectively a
//! hyperbola and an ellipse. Their vertical lifts are sampled on a meridian of
//! the first torus and fitted to cubic NURBS within the modelling tolerance.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, NurbsCurve, Real, Tolerance};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
enum Family {
    Difference,
    Sum,
}

#[derive(Clone, Copy)]
struct Section {
    frame: Frame3,
    average_major: Real,
    half_major_difference: Real,
    minor: Real,
    half_offset: Real,
    axis: [Real; 2],
    family: Family,
    lateral_scale: Real,
}

#[derive(Clone, Copy)]
enum Branch {
    Full { side: Real },
    SumVertical { side: Real },
    SumEllipseTurned { start: Real, end: Real },
    Turned { start: Real, end: Real },
    CriticalDifference,
    CriticalSum,
}

impl Branch {
    fn period(self) -> Real {
        match self {
            Self::CriticalDifference | Self::CriticalSum => 2.0 * TURN,
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
    (frame, first_major, minor): (Frame3, Real, Real),
    second_major: Real,
    [offset_x, offset_y]: [Real; 2],
    tolerance: Tolerance,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let half_offset = 0.5 * offset_x.hypot(offset_y);
    let average_major = 0.5 * first_major + 0.5 * second_major;
    let half_major_difference = 0.5 * first_major - 0.5 * second_major;
    let axis = [
        offset_x / (2.0 * half_offset),
        offset_y / (2.0 * half_offset),
    ];
    let mut events = Vec::new();
    let critical = (16.0 * Real::EPSILON * average_major.max(minor))
        .max(fit_tolerance * fit_tolerance / (2.0 * minor));

    // The difference-of-distances locus exists only when the focal spacing is
    // at least the prescribed distance difference. Equality is a ray, whose
    // lift is an exact meridian circle.
    if half_offset + critical >= half_major_difference.abs() {
        if (half_offset - half_major_difference.abs()).abs() <= critical {
            events.push(meridian_circle(
                frame,
                first_major,
                minor,
                axis,
                half_major_difference.signum(),
                tolerance,
            )?);
        } else {
            let section = Section::new(
                frame,
                average_major,
                half_major_difference,
                minor,
                half_offset,
                axis,
                Family::Difference,
            );
            let point_limit = average_major + minor;
            if (half_offset - point_limit).abs() <= critical {
                let center = frame.point_at([
                    (first_major + minor) * axis[0],
                    (first_major + minor) * axis[1],
                    0.0,
                ])?;
                events.push(SurfaceSurfaceIntersectionEvent::Point(center));
            } else if (half_offset - (average_major - minor)).abs() <= critical {
                events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                    section,
                    Branch::CriticalDifference,
                    fit_tolerance,
                )?));
            } else if half_offset < point_limit {
                section.append_curves(&mut events, fit_tolerance, critical)?;
            }
        }
    }

    // The sum-of-distances locus collapses to a segment when the projected
    // ring circles are externally tangent. Its lift is another meridian.
    if (half_offset - average_major).abs() <= critical {
        events.push(meridian_circle(
            frame,
            first_major,
            minor,
            axis,
            1.0,
            tolerance,
        )?);
    } else if half_offset < average_major {
        let section = Section::new(
            frame,
            average_major,
            half_major_difference,
            minor,
            half_offset,
            axis,
            Family::Sum,
        );
        let inner_critical = (minor - half_major_difference.abs()).abs();
        if inner_critical > critical && (half_offset - inner_critical).abs() <= critical {
            if half_major_difference.abs() > minor {
                let axial = half_major_difference.signum() * first_major - minor;
                let contact = frame.point_at([axial * axis[0], axial * axis[1], 0.0])?;
                events.push(SurfaceSurfaceIntersectionEvent::Point(contact));
            } else {
                section.append_inner_critical_curves(&mut events, fit_tolerance)?;
            }
        } else if half_offset < minor
            && (half_major_difference.abs() - minor).abs() < half_offset - critical
        {
            let root_value = half_major_difference - half_major_difference.signum() * minor;
            let angle = (root_value / half_offset).acos();
            let (start, end) = if half_major_difference > 0.0 {
                (TURN - angle, TURN + angle)
            } else {
                (angle, TURN - angle)
            };
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                section,
                Branch::SumEllipseTurned { start, end },
                fit_tolerance,
            )?));
        } else if half_major_difference.abs() + half_offset < minor - critical {
            for side in [1.0, -1.0] {
                events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                    section,
                    Branch::SumVertical { side },
                    fit_tolerance,
                )?));
            }
        } else if (half_offset - (half_major_difference.abs() + minor)).abs() <= critical {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                section,
                Branch::CriticalSum,
                fit_tolerance,
            )?));
        } else {
            section.append_curves(&mut events, fit_tolerance, critical)?;
        }
    }
    Ok(events)
}

fn meridian_circle(
    frame: Frame3,
    major: Real,
    minor: Real,
    axis: [Real; 2],
    side: Real,
    tolerance: Tolerance,
) -> Result<SurfaceSurfaceIntersectionEvent, GeometryError> {
    let center = frame.point_at([side * major * axis[0], side * major * axis[1], 0.0])?;
    let normal = frame
        .vector_at([-axis[1], axis[0], 0.0])?
        .normalized(tolerance)?;
    let circle =
        Circle3::try_from_frame(center, minor, frame.z_axis(), normal, tolerance)?.to_nurbs()?;
    Ok(SurfaceSurfaceIntersectionEvent::Curve(circle))
}

impl Section {
    #[allow(clippy::too_many_arguments)]
    fn new(
        frame: Frame3,
        average_major: Real,
        half_major_difference: Real,
        minor: Real,
        half_offset: Real,
        axis: [Real; 2],
        family: Family,
    ) -> Self {
        let lateral_scale = match family {
            Family::Difference => {
                (half_offset - half_major_difference.abs())
                    * (half_offset + half_major_difference.abs())
            }
            Family::Sum => (average_major - half_offset) * (average_major + half_offset),
        }
        .max(0.0)
        .sqrt()
            / half_offset;
        Self {
            frame,
            average_major,
            half_major_difference,
            minor,
            half_offset,
            axis,
            family,
            lateral_scale,
        }
    }

    fn root_values(self) -> [Option<Real>; 2] {
        match self.family {
            Family::Difference => [Some(self.half_offset - self.average_major), None],
            Family::Sum => [
                Some(-self.half_offset - self.half_major_difference),
                Some(self.half_offset - self.half_major_difference),
            ],
        }
    }

    fn radicand(self, angle: Real) -> Real {
        let (half_sine, half_cosine) = (0.5 * angle).sin_cos();
        if half_sine.abs() <= half_cosine.abs() {
            let change = 2.0 * self.minor * half_sine * half_sine;
            match self.family {
                Family::Difference => {
                    let radial = self.average_major + self.minor;
                    (radial - self.half_offset - change) * (radial + self.half_offset - change)
                }
                Family::Sum => {
                    let radial = self.half_major_difference + self.minor;
                    (self.half_offset - radial + change) * (self.half_offset + radial - change)
                }
            }
        } else {
            let change = 2.0 * self.minor * half_cosine * half_cosine;
            match self.family {
                Family::Difference => {
                    let radial = self.average_major - self.minor;
                    (radial - self.half_offset + change) * (radial + self.half_offset + change)
                }
                Family::Sum => {
                    let radial = self.half_major_difference - self.minor;
                    (self.half_offset - radial - change) * (self.half_offset + radial + change)
                }
            }
        }
    }

    fn radicand_near_root(self, root: Real, delta: Real) -> Real {
        let delta_q = self.meridian_change(root, delta);
        match self.family {
            Family::Difference => delta_q * (2.0 * self.half_offset + delta_q),
            Family::Sum => {
                let radial_at_root = self.sum_root_sign(root) * self.half_offset;
                -delta_q * (2.0 * radial_at_root + delta_q)
            }
        }
    }

    fn meridian_change(self, root: Real, delta: Real) -> Real {
        -2.0 * self.minor * (root + 0.5 * delta).sin() * (0.5 * delta).sin()
    }

    fn sum_root_sign(self, root: Real) -> Real {
        (self.half_major_difference + self.minor * root.cos()).signum()
    }

    fn radicand_derivative_at_root(self, root: Real) -> Real {
        let radial = match self.family {
            Family::Difference => self.half_offset,
            Family::Sum => -self.sum_root_sign(root) * self.half_offset,
        };
        2.0 * radial * (-self.minor * root.sin())
    }

    fn radicand_derivative(self, angle: Real) -> Real {
        let q = self.minor * angle.cos();
        let dq = -self.minor * angle.sin();
        match self.family {
            Family::Difference => 2.0 * (self.average_major + q) * dq,
            Family::Sum => -2.0 * (self.half_major_difference + q) * dq,
        }
    }

    fn active_intervals(self, critical: Real) -> Result<Vec<(Real, Real)>, GeometryError> {
        let mut cuts = vec![0.0, TURN];
        for root_value in self.root_values().into_iter().flatten() {
            if (root_value.abs() - self.minor).abs() <= critical {
                return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                    context: "critical unequal-major torus section",
                });
            }
            if root_value.abs() < self.minor {
                let angle = (root_value / self.minor).acos();
                cuts.extend([angle, TURN - angle]);
            }
        }
        cuts.sort_by(Real::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() <= 16.0 * Real::EPSILON);
        let mut intervals = cuts
            .windows(2)
            .filter_map(|pair| {
                let (start, end) = (pair[0], pair[1]);
                (self.radicand(0.5 * (start + end)) > 0.0).then_some((start, end))
            })
            .collect::<Vec<_>>();
        if intervals.len() >= 2
            && intervals[0].0 == 0.0
            && intervals.last().is_some_and(|interval| interval.1 == TURN)
        {
            let first = intervals.remove(0);
            let last = intervals.pop().unwrap();
            intervals.push((last.0, first.1 + TURN));
        }
        Ok(intervals)
    }

    fn append_curves(
        self,
        events: &mut Vec<SurfaceSurfaceIntersectionEvent>,
        fit_tolerance: Real,
        critical: Real,
    ) -> Result<(), GeometryError> {
        for (start, end) in self.active_intervals(critical)? {
            let full = (end - start - TURN).abs() <= 32.0 * Real::EPSILON;
            if full {
                for side in [1.0, -1.0] {
                    events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                        self,
                        Branch::Full { side },
                        fit_tolerance,
                    )?));
                }
            } else {
                events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                    self,
                    Branch::Turned { start, end },
                    fit_tolerance,
                )?));
            }
        }
        Ok(())
    }

    fn append_inner_critical_curves(
        self,
        events: &mut Vec<SurfaceSurfaceIntersectionEvent>,
        fit_tolerance: Real,
    ) -> Result<(), GeometryError> {
        debug_assert!(matches!(self.family, Family::Sum));
        let other_root = if self.half_major_difference > 0.0 {
            self.half_offset - self.half_major_difference
        } else {
            -self.half_offset - self.half_major_difference
        };
        let angle = (other_root / self.minor).acos();
        let intervals = if self.half_major_difference > 0.0 {
            [
                (angle, std::f64::consts::PI),
                (std::f64::consts::PI, TURN - angle),
            ]
        } else {
            [(0.0, angle), (TURN - angle, TURN)]
        };
        for (start, end) in intervals {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
                self,
                Branch::Turned { start, end },
                fit_tolerance,
            )?));
        }
        Ok(())
    }

    fn sample(self, branch: Branch, parameter: Real) -> Sample {
        if let Branch::SumEllipseTurned { start, end } = branch {
            return self.sample_sum_ellipse_turned(start, end, parameter);
        }
        if let Branch::SumVertical { side } = branch {
            let (sine, cosine) = parameter.sin_cos();
            let transverse = ((self.average_major - self.half_offset)
                * (self.average_major + self.half_offset))
                .sqrt();
            let radial = self.half_offset * cosine - self.half_major_difference;
            let height = ((self.minor - radial) * (self.minor + radial)).sqrt();
            let longitudinal = self.half_offset + self.average_major * cosine;
            let lateral = transverse * sine;
            let longitudinal_derivative = -self.average_major * sine;
            let lateral_derivative = transverse * cosine;
            let height_derivative = side * radial * self.half_offset * sine / height;
            let [ux, uy] = self.axis;
            return Sample {
                position: [
                    longitudinal.mul_add(ux, -lateral * uy),
                    longitudinal.mul_add(uy, lateral * ux),
                    side * height,
                ],
                derivative: [
                    longitudinal_derivative.mul_add(ux, -lateral_derivative * uy),
                    longitudinal_derivative.mul_add(uy, lateral_derivative * ux),
                    height_derivative,
                ],
            };
        }
        if matches!(branch, Branch::CriticalDifference | Branch::CriticalSum) {
            let (half_sine, half_cosine) = (0.5 * parameter).sin_cos();
            let (root, root_derivative) = match branch {
                Branch::CriticalDifference => {
                    let critical_offset = self.average_major - self.minor;
                    let middle = critical_offset + self.minor * half_cosine * half_cosine;
                    (
                        2.0 * self.minor.sqrt() * half_cosine * middle.sqrt(),
                        -self.minor.sqrt()
                            * half_sine
                            * (critical_offset + 2.0 * self.minor * half_cosine * half_cosine)
                            / middle.sqrt(),
                    )
                }
                Branch::CriticalSum if self.half_major_difference > 0.0 => {
                    let middle =
                        self.half_major_difference + self.minor * half_cosine * half_cosine;
                    (
                        2.0 * self.minor.sqrt() * half_sine * middle.sqrt(),
                        self.minor.sqrt()
                            * half_cosine
                            * (self.half_major_difference + self.minor * parameter.cos())
                            / middle.sqrt(),
                    )
                }
                Branch::CriticalSum => {
                    let middle = -self.half_major_difference + self.minor * half_sine * half_sine;
                    (
                        2.0 * self.minor.sqrt() * half_cosine * middle.sqrt(),
                        self.minor.sqrt()
                            * half_sine
                            * (self.half_major_difference + self.minor * parameter.cos())
                            / middle.sqrt(),
                    )
                }
                _ => unreachable!(),
            };
            let q = self.minor * parameter.cos();
            let dq = -self.minor * parameter.sin();
            let longitudinal = self.half_offset
                + match branch {
                    Branch::CriticalDifference => {
                        self.half_major_difference * (self.average_major + q) / self.half_offset
                    }
                    Branch::CriticalSum => {
                        self.average_major * (self.half_major_difference + q) / self.half_offset
                    }
                    _ => unreachable!(),
                };
            let longitudinal_derivative = match branch {
                Branch::CriticalDifference => self.half_major_difference * dq / self.half_offset,
                Branch::CriticalSum => self.average_major * dq / self.half_offset,
                _ => unreachable!(),
            };
            let lateral = self.lateral_scale * root;
            let lateral_derivative = self.lateral_scale * root_derivative;
            let [ux, uy] = self.axis;
            return Sample {
                position: [
                    longitudinal.mul_add(ux, -lateral * uy),
                    longitudinal.mul_add(uy, lateral * ux),
                    self.minor * parameter.sin(),
                ],
                derivative: [
                    longitudinal_derivative.mul_add(ux, -lateral_derivative * uy),
                    longitudinal_derivative.mul_add(uy, lateral_derivative * ux),
                    self.minor * parameter.cos(),
                ],
            };
        }
        let (angle, side, angle_derivative, root) = match branch {
            Branch::Full { side } => (parameter, side, 1.0, None),
            Branch::SumVertical { .. } => unreachable!(),
            Branch::SumEllipseTurned { .. } => unreachable!(),
            Branch::Turned { start, end } => {
                let middle = 0.5 * (start + end);
                let half = 0.5 * (end - start);
                if parameter <= std::f64::consts::PI {
                    let angle = if parameter == 0.0 {
                        start
                    } else if parameter == std::f64::consts::PI {
                        end
                    } else {
                        middle - half * parameter.cos()
                    };
                    let from_start = 2.0 * half * (0.5 * parameter).sin().powi(2);
                    let from_end = 2.0 * half * (0.5 * parameter).cos().powi(2);
                    let root = if from_start <= from_end && from_start < 1.0e-4 {
                        Some((start, from_start))
                    } else if from_end < 1.0e-4 {
                        Some((end, -from_end))
                    } else {
                        None
                    };
                    (angle, 1.0, half * parameter.sin(), root)
                } else {
                    let back = parameter - std::f64::consts::PI;
                    let angle = if parameter == TURN {
                        start
                    } else {
                        middle + half * back.cos()
                    };
                    let from_end = 2.0 * half * (0.5 * back).sin().powi(2);
                    let from_start = 2.0 * half * (0.5 * back).cos().powi(2);
                    let root = if from_end <= from_start && from_end < 1.0e-4 {
                        Some((end, -from_end))
                    } else if from_start < 1.0e-4 {
                        Some((start, from_start))
                    } else {
                        None
                    };
                    (angle, -1.0, -half * back.sin(), root)
                }
            }
            Branch::CriticalDifference => unreachable!(),
            Branch::CriticalSum => unreachable!(),
        };
        let q = self.minor * angle.cos();
        let dq = -self.minor * angle.sin();
        let radial = root
            .map(|(at, delta)| {
                let radial_at_root = match self.family {
                    Family::Difference => self.half_offset,
                    Family::Sum => self.sum_root_sign(at) * self.half_offset,
                };
                radial_at_root + self.meridian_change(at, delta)
            })
            .unwrap_or_else(|| match self.family {
                Family::Difference => self.average_major + q,
                Family::Sum => self.half_major_difference + q,
            });
        let longitudinal = self.half_offset
            + match self.family {
                Family::Difference => self.half_major_difference * radial / self.half_offset,
                Family::Sum => self.average_major * radial / self.half_offset,
            };
        let longitudinal_derivative = match self.family {
            Family::Difference => self.half_major_difference * dq / self.half_offset,
            Family::Sum => self.average_major * dq / self.half_offset,
        } * angle_derivative;
        let radicand = root
            .map(|(at, delta)| self.radicand_near_root(at, delta))
            .unwrap_or_else(|| self.radicand(angle))
            .max(0.0);
        let root_height = radicand.sqrt();
        let lateral = side * self.lateral_scale * root_height;
        let lateral_derivative = match branch {
            Branch::Turned { start, end }
                if parameter == 0.0 || parameter == std::f64::consts::PI || parameter == TURN =>
            {
                let half = 0.5 * (end - start);
                if parameter == std::f64::consts::PI {
                    -self.lateral_scale
                        * (-self.radicand_derivative_at_root(end) * half * 0.5)
                            .max(0.0)
                            .sqrt()
                } else {
                    self.lateral_scale
                        * (self.radicand_derivative_at_root(start) * half * 0.5)
                            .max(0.0)
                            .sqrt()
                }
            }
            _ if root_height > 0.0 => {
                side * self.lateral_scale * self.radicand_derivative(angle) * angle_derivative
                    / (2.0 * root_height)
            }
            _ => 0.0,
        };
        let [ux, uy] = self.axis;
        Sample {
            position: [
                longitudinal.mul_add(ux, -lateral * uy),
                longitudinal.mul_add(uy, lateral * ux),
                self.minor * angle.sin(),
            ],
            derivative: [
                longitudinal_derivative.mul_add(ux, -lateral_derivative * uy),
                longitudinal_derivative.mul_add(uy, lateral_derivative * ux),
                self.minor * angle.cos() * angle_derivative,
            ],
        }
    }

    fn sample_sum_ellipse_turned(self, start: Real, end: Real, parameter: Real) -> Sample {
        let middle = 0.5 * (start + end);
        let half = 0.5 * (end - start);
        let (angle, side, angle_derivative, root) = if parameter <= std::f64::consts::PI {
            let angle = if parameter == 0.0 {
                start
            } else if parameter == std::f64::consts::PI {
                end
            } else {
                middle - half * parameter.cos()
            };
            let from_start = 2.0 * half * (0.5 * parameter).sin().powi(2);
            let from_end = 2.0 * half * (0.5 * parameter).cos().powi(2);
            let root = if from_start <= from_end && from_start < 1.0e-4 {
                Some((start, from_start))
            } else if from_end < 1.0e-4 {
                Some((end, -from_end))
            } else {
                None
            };
            (angle, 1.0, half * parameter.sin(), root)
        } else {
            let back = parameter - std::f64::consts::PI;
            let angle = if parameter == TURN {
                start
            } else {
                middle + half * back.cos()
            };
            let from_end = 2.0 * half * (0.5 * back).sin().powi(2);
            let from_start = 2.0 * half * (0.5 * back).cos().powi(2);
            let root = if from_end <= from_start && from_end < 1.0e-4 {
                Some((end, -from_end))
            } else if from_start < 1.0e-4 {
                Some((start, from_start))
            } else {
                None
            };
            (angle, -1.0, -half * back.sin(), root)
        };
        let (sine, cosine) = angle.sin_cos();
        let transverse = ((self.average_major - self.half_offset)
            * (self.average_major + self.half_offset))
            .sqrt();
        let meridian = self.half_offset * cosine - self.half_major_difference;
        let radical = if let Some((at, delta)) = root {
            let meridian_at_root =
                (self.half_offset * at.cos() - self.half_major_difference).signum() * self.minor;
            let change = -2.0 * self.half_offset * (at + 0.5 * delta).sin() * (0.5 * delta).sin();
            -change * (2.0 * meridian_at_root + change)
        } else {
            (self.minor - meridian) * (self.minor + meridian)
        }
        .max(0.0);
        let height = radical.sqrt();
        let longitudinal = self.half_offset + self.average_major * cosine;
        let lateral = transverse * sine;
        let longitudinal_derivative = -self.average_major * sine * angle_derivative;
        let lateral_derivative = transverse * cosine * angle_derivative;
        let height_derivative =
            if parameter == 0.0 || parameter == std::f64::consts::PI || parameter == TURN {
                let endpoint = if parameter == std::f64::consts::PI {
                    end
                } else {
                    start
                };
                let meridian_at_root =
                    (self.half_offset * endpoint.cos() - self.half_major_difference).signum()
                        * self.minor;
                let radial_derivative = 2.0 * meridian_at_root * self.half_offset * endpoint.sin();
                if parameter == std::f64::consts::PI {
                    -(-radial_derivative * half * 0.5).max(0.0).sqrt()
                } else {
                    (radial_derivative * half * 0.5).max(0.0).sqrt()
                }
            } else if height > 0.0 {
                side * meridian * self.half_offset * sine * angle_derivative / height
            } else {
                0.0
            };
        let [ux, uy] = self.axis;
        Sample {
            position: [
                longitudinal.mul_add(ux, -lateral * uy),
                longitudinal.mul_add(uy, lateral * ux),
                side * height,
            ],
            derivative: [
                longitudinal_derivative.mul_add(ux, -lateral_derivative * uy),
                longitudinal_derivative.mul_add(uy, lateral_derivative * ux),
                height_derivative,
            ],
        }
    }
}

fn fit(section: Section, branch: Branch, tolerance: Real) -> Result<NurbsCurve, GeometryError> {
    let mut segments = Vec::new();
    let period = branch.period();
    let interval_count = if period > TURN { 16 } else { 8 };
    for index in 0..interval_count {
        fit_interval(
            section,
            branch,
            period * index as Real / interval_count as Real,
            period * (index + 1) as Real / interval_count as Real,
            tolerance,
            0,
            &mut segments,
        )?;
    }
    let first = section
        .frame
        .point_at(section.sample(branch, 0.0).position)?;
    let mut controls = Vec::with_capacity(3 * segments.len() + 1);
    let mut knots = Vec::with_capacity(3 * segments.len() + 5);
    controls.push(first);
    knots.extend([0.0; 4]);
    let count = segments.len();
    for (index, (end, control)) in segments.into_iter().enumerate() {
        controls.push(section.frame.point_at(control[1])?);
        controls.push(section.frame.point_at(control[2])?);
        controls.push(if index + 1 == count {
            first
        } else {
            section.frame.point_at(control[3])?
        });
        knots.extend([end; 3]);
    }
    knots.push(period);
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
    fit_interval(section, branch, middle, end, tolerance, depth + 1, segments)
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
    use crate::{NurbsSurface, Point3, Vector3, surface_surface_intersection_events};

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

    fn check_residuals(
        events: &[SurfaceSurfaceIntersectionEvent],
        offset: Real,
        second_major: Real,
    ) {
        for event in events {
            let mut points = Vec::new();
            match event {
                SurfaceSurfaceIntersectionEvent::Point(point) => points.push(*point),
                SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                    assert!(curve.is_closed().unwrap(), "offset={offset}");
                    let domain = curve.domain();
                    for index in 0..=64 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        points.push(curve.evaluate(parameter).unwrap());
                    }
                }
            }
            for location in points {
                let first = (location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0;
                let second = ((location.x() - offset).hypot(location.y()) - second_major)
                    .hypot(location.z())
                    - 1.0;
                assert!(first.abs() < 5e-9, "offset={offset}, first={first}");
                assert!(second.abs() < 5e-9, "offset={offset}, second={second}");
            }
        }
    }

    #[test]
    fn unequal_ring_radii_cover_nested_crossing_tangent_and_disjoint_sections() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (offset, expected) in [
            (0.4, 2),
            (0.5, 3),
            (1.0, 4),
            (1.5, 4),
            (2.5, 3),
            (3.0, 4),
            (6.5, 3),
            (7.0, 3),
            (8.5, 2),
            (10.5, 1),
            (11.0, 0),
        ] {
            for normal_z in [1.0, -1.0] {
                let second_frame = Frame3::try_from_normal(
                    point(offset, 0.0, 0.0),
                    Vector3::try_new(0.0, 0.0, normal_z).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap();
                let second = NurbsSurface::try_torus(second_frame, 4.5, 1.0).unwrap();
                for (left, right) in [(&first, &second), (&second, &first)] {
                    let events =
                        surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                            .unwrap_or_else(|error| panic!("offset={offset}: {error:?}"));
                    assert_eq!(events.len(), expected, "offset={offset}");
                    check_residuals(&events, offset, 4.5);
                    if offset == 1.5 {
                        let contact = point(5.0, 0.0, 0.0);
                        let touching = events
                            .iter()
                            .filter(|event| match event {
                                SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                                    let domain = curve.domain();
                                    [
                                        *domain.start(),
                                        0.5 * (*domain.start() + *domain.end()),
                                        *domain.end(),
                                    ]
                                    .into_iter()
                                    .any(|parameter| {
                                        curve
                                            .evaluate(parameter)
                                            .unwrap()
                                            .distance_to(contact)
                                            .unwrap()
                                            < 5e-9
                                    })
                                }
                                SurfaceSurfaceIntersectionEvent::Point(_) => false,
                            })
                            .count();
                        assert_eq!(touching, 2);
                    }
                }
            }
        }
    }

    #[test]
    fn unequal_ring_radii_handle_distant_rotated_frames() {
        let first_frame = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_center = first_frame.point_at([1.0, 0.0, 0.0]).unwrap();
        let first = NurbsSurface::try_torus(first_frame, 4.0, 1.0).unwrap();
        let second =
            NurbsSurface::try_torus(first_frame.with_origin(second_center), 4.5, 1.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated torus sections should be curves")
            };
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                let first_local = first_frame.coordinates_of(location).unwrap();
                let second_local = first_frame
                    .with_origin(second_center)
                    .coordinates_of(location)
                    .unwrap();
                assert!(
                    ((first_local[0].hypot(first_local[1]) - 4.0).hypot(first_local[2]) - 1.0)
                        .abs()
                        < 4e-7
                );
                assert!(
                    ((second_local[0].hypot(second_local[1]) - 4.5).hypot(second_local[2]) - 1.0)
                        .abs()
                        < 4e-7
                );
            }
        }
    }

    #[test]
    fn unequal_ring_radii_offset_grid_keeps_both_torus_residuals_small() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for second_major in [3.5, 4.5, 6.0] {
            for step in 0..13 {
                let offset = 0.37 + step as Real;
                let second =
                    NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), second_major, 1.0)
                        .unwrap();
                let events =
                    surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                        .unwrap_or_else(|error| {
                            panic!("major={second_major}, offset={offset}: {error:?}")
                        });
                check_residuals(&events, offset, second_major);
            }
        }
    }

    #[test]
    fn unequal_ring_radii_keep_near_critical_topology() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (half_offset, expected) in [
            (0.75 - 1.0e-12, 4),
            (0.75 + 1.0e-12, 3),
            (1.25 - 1.0e-12, 3),
            (1.25 + 1.0e-12, 4),
            (3.25 - 1.0e-12, 4),
            (3.25 + 1.0e-12, 3),
        ] {
            let offset = 2.0 * half_offset;
            let second = NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), 4.5, 1.0).unwrap();
            let events = surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("half_offset={half_offset}: {error:?}"));
            assert_eq!(events.len(), expected, "half_offset={half_offset}");
            check_residuals(&events, offset, 4.5);
        }
    }

    #[test]
    fn unequal_ring_radii_approach_coaxial_circles() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for offset in [1.0e-4, 1.0e-6, 1.0e-8] {
            let second = NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), 4.5, 1.0).unwrap();
            let events = surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("offset={offset}: {error:?}"));
            assert_eq!(events.len(), 2, "offset={offset}");
            check_residuals(&events, offset, 4.5);
        }
    }

    #[test]
    fn unequal_ring_radii_inner_contact_is_one_point() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 7.0, 1.0).unwrap();
        for offset in [0.5, 1.0 - 2.0e-12, 1.0, 1.0 + 2.0e-12, 1.5] {
            let second = NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), 4.0, 1.0).unwrap();
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("offset={offset}: {error:?}"));
                if offset == 1.0 {
                    let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice()
                    else {
                        panic!("inner tangency should be one point")
                    };
                    assert!(contact.distance_to(point(6.0, 0.0, 0.0)).unwrap() < 5e-9);
                } else if offset < 1.0 {
                    assert!(events.is_empty());
                } else {
                    assert_eq!(events.len(), 1);
                    assert!(matches!(
                        events[0],
                        SurfaceSurfaceIntersectionEvent::Curve(_)
                    ));
                }
            }
        }
    }

    #[test]
    fn unequal_ring_radii_resolve_near_coaxial_tangent_circle() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for offset in [1.0e-4, 1.0e-6, 1.0e-8] {
            let second = NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), 6.0, 1.0).unwrap();
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("offset={offset}: {error:?}"));
                assert_eq!(events.len(), 1, "offset={offset}");
                check_residuals(&events, offset, 6.0);
            }
        }
        let offset = 1.0e-4;
        for second_major in [5.99998, 6.00002] {
            let second =
                NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), second_major, 1.0).unwrap();
            let events =
                surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 1);
            check_residuals(&events, offset, second_major);
        }
    }
}
