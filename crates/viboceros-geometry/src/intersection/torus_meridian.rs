//! Cubic intersections of a ring torus with angular meridian lines.

use super::{SurfaceSurfaceIntersectionEvent, intersect_curve_with_planar_surface};
use crate::{Frame3, GeometryError, NurbsCurve, NurbsSurface, Plane, Real, Tolerance};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
pub(super) struct Section {
    pub(super) frame: Frame3,
    pub(super) major: Real,
    pub(super) minor: Real,
    pub(super) radial_base: Real,
    pub(super) radial_cosine: Real,
    pub(super) axial_coefficient: Real,
    pub(super) line_constant: Real,
    pub(super) radial_axis: [Real; 2],
}

#[derive(Clone, Copy)]
enum Branch {
    Full {
        sign: Real,
    },
    Turned {
        start: Real,
        end: Real,
        critical: Option<Real>,
    },
    Crossing {
        sign: Real,
    },
}

#[derive(Clone, Copy)]
struct Sample {
    position: [Real; 3],
    derivative: [Real; 3],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn intersect(
    frame: Frame3,
    major: Real,
    minor: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    signed_distance: Real,
    tolerance: Tolerance,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let normal = plane.normal().as_vector();
    let horizontal_x = normal.dot(frame.x_axis().as_vector())?;
    let horizontal_y = normal.dot(frame.y_axis().as_vector())?;
    let horizontal = horizontal_x.hypot(horizontal_y);
    let vertical = normal.dot(frame.z_axis().as_vector())?;
    let section = Section {
        frame,
        major,
        minor,
        radial_base: 0.0,
        radial_cosine: horizontal,
        axial_coefficient: vertical,
        line_constant: -signed_distance,
        radial_axis: [horizontal_x / horizontal, horizontal_y / horizontal],
    };
    let events = intersect_meridian(section, fit_tolerance)?;
    let mut clipped = Vec::new();
    for event in events {
        match event {
            SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                clipped.extend(intersect_curve_with_planar_surface(
                    &curve,
                    planar_surface,
                    tolerance,
                )?);
            }
            SurfaceSurfaceIntersectionEvent::Point(point) => {
                let (u, v) = planar_surface.closest_parameters(point, tolerance)?;
                if planar_surface.evaluate(u, v)?.distance_to(point)? <= fit_tolerance {
                    clipped.push(SurfaceSurfaceIntersectionEvent::Point(point));
                }
            }
        }
    }
    Ok(clipped)
}

pub(super) fn intersect_meridian(
    section: Section,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let major = section.major;
    let minor = section.minor;
    let unconstrained_vertex = section.vertex();
    let vertex = unconstrained_vertex.clamp(-1.0, 1.0);
    let maximum = section.discriminant(vertex);
    if maximum < 0.0 {
        let a = section.radial_cosine.mul_add(vertex, section.radial_base);
        let q = a.mul_add(a, section.axial_coefficient * section.axial_coefficient);
        let delta = section.line_constant - section.major * a;
        let miss = delta.abs() / q.sqrt() - section.minor;
        if miss <= fit_tolerance {
            return tangent_points(section, vertex);
        }
        return Ok(Vec::new());
    }
    let minimum_cosine = if section.discriminant(-1.0) >= 0.0 {
        -1.0
    } else {
        bisect_discriminant(section, -1.0, vertex)
    };
    let maximum_cosine = if section.discriminant(1.0) >= 0.0 {
        1.0
    } else {
        bisect_discriminant(section, vertex, 1.0)
    };
    let angular_roundoff = 64.0 * Real::EPSILON * (major + minor).powi(2);
    let singular_negative = section.discriminant(-1.0).abs() <= angular_roundoff;
    let singular_positive = section.discriminant(1.0).abs() <= angular_roundoff;
    if singular_negative && singular_positive {
        let mut events = Vec::new();
        for sign in [1.0, -1.0] {
            let curve = fit(section, Branch::Crossing { sign }, fit_tolerance)?;
            events.push(SurfaceSurfaceIntersectionEvent::Curve(curve));
        }
        return Ok(events);
    }
    if singular_positive {
        if unconstrained_vertex >= 1.0 {
            return tangent_points(section, 1.0);
        }
        let angle = minimum_cosine.acos();
        return fit_branches(
            section,
            [
                Branch::Turned {
                    start: -angle,
                    end: 0.0,
                    critical: Some(1.0),
                },
                Branch::Turned {
                    start: 0.0,
                    end: angle,
                    critical: Some(1.0),
                },
            ],
            fit_tolerance,
        );
    }
    if singular_negative {
        if unconstrained_vertex <= -1.0 {
            return tangent_points(section, -1.0);
        }
        let angle = maximum_cosine.acos();
        return fit_branches(
            section,
            [
                Branch::Turned {
                    start: angle,
                    end: std::f64::consts::PI,
                    critical: Some(-1.0),
                },
                Branch::Turned {
                    start: std::f64::consts::PI,
                    end: TURN - angle,
                    critical: Some(-1.0),
                },
            ],
            fit_tolerance,
        );
    }
    let radial_scale = section
        .radial_cosine
        .mul_add(vertex, section.radial_base)
        .hypot(section.axial_coefficient);
    if maximum <= fit_tolerance * fit_tolerance * radial_scale * radial_scale {
        return tangent_points(section, vertex);
    }
    let branches = if minimum_cosine <= -1.0 && maximum_cosine >= 1.0 {
        vec![Branch::Full { sign: 1.0 }, Branch::Full { sign: -1.0 }]
    } else if minimum_cosine <= -1.0 {
        let angle = maximum_cosine.acos();
        vec![Branch::Turned {
            start: angle,
            end: TURN - angle,
            critical: None,
        }]
    } else if maximum_cosine >= 1.0 {
        let angle = minimum_cosine.acos();
        vec![Branch::Turned {
            start: -angle,
            end: angle,
            critical: None,
        }]
    } else {
        let first = maximum_cosine.acos();
        let second = minimum_cosine.acos();
        vec![
            Branch::Turned {
                start: first,
                end: second,
                critical: None,
            },
            Branch::Turned {
                start: TURN - second,
                end: TURN - first,
                critical: None,
            },
        ]
    };
    fit_branches(section, branches, fit_tolerance)
}

fn fit_branches(
    section: Section,
    branches: impl IntoIterator<Item = Branch>,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let mut events = Vec::new();
    for branch in branches {
        let curve = fit(section, branch, fit_tolerance)?;
        events.push(SurfaceSurfaceIntersectionEvent::Curve(curve));
    }
    Ok(events)
}

fn tangent_points(
    section: Section,
    cosine: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let angle = cosine.acos();
    let angles: &[Real] = if angle.abs() <= 32.0 * Real::EPSILON
        || (angle - std::f64::consts::PI).abs() <= 32.0 * Real::EPSILON
    {
        &[angle]
    } else {
        &[angle, TURN - angle]
    };
    let mut events = Vec::new();
    for &angle in angles {
        let local = section
            .sample_at_angle(angle, 1.0, 0.0, Some(0.0), None)
            .position;
        let point = section.frame.point_at(local)?;
        events.push(SurfaceSurfaceIntersectionEvent::Point(point));
    }
    Ok(events)
}

fn bisect_discriminant(section: Section, mut low: Real, mut high: Real) -> Real {
    let low_positive = section.discriminant(low) >= 0.0;
    for _ in 0..70 {
        let middle = 0.5 * (low + high);
        if middle == low || middle == high {
            break;
        }
        if (section.discriminant(middle) >= 0.0) == low_positive {
            low = middle;
        } else {
            high = middle;
        }
    }
    0.5 * (low + high)
}

impl Section {
    fn vertex(self) -> Real {
        let radius_difference = (self.major - self.minor) * (self.major + self.minor);
        (self.line_constant * self.major / radius_difference - self.radial_base)
            / self.radial_cosine
    }

    fn discriminant(self, cosine: Real) -> Real {
        let a = self.radial_cosine.mul_add(cosine, self.radial_base);
        let q = a.mul_add(a, self.axial_coefficient * self.axial_coefficient);
        let delta = self.line_constant - self.major * a;
        self.minor * self.minor * q - delta * delta
    }

    fn sample(self, branch: Branch, parameter: Real) -> Sample {
        match branch {
            Branch::Full { sign } => self.sample_at_angle(parameter, sign, 1.0, None, None),
            Branch::Crossing { sign } => self.sample_crossing(parameter, sign),
            Branch::Turned {
                start,
                end,
                critical,
            } => {
                let middle = 0.5 * (start + end);
                let half = 0.5 * (end - start);
                let (angle, sign, angle_derivative, endpoint_factor_derivative) =
                    if parameter <= std::f64::consts::PI {
                        let angle = middle - half * parameter.cos();
                        let derivative = half * parameter.sin();
                        let endpoint = if parameter == 0.0 {
                            Some(self.endpoint_factor_derivative(start, half, 1.0))
                        } else if parameter == std::f64::consts::PI {
                            Some(self.endpoint_factor_derivative(end, half, -1.0))
                        } else {
                            None
                        };
                        (angle, 1.0, derivative, endpoint)
                    } else {
                        let return_angle = parameter - std::f64::consts::PI;
                        let angle = middle + half * return_angle.cos();
                        let derivative = -half * return_angle.sin();
                        let endpoint = if parameter == TURN {
                            Some(self.endpoint_factor_derivative(start, half, -1.0))
                        } else {
                            None
                        };
                        (angle, -1.0, derivative, endpoint)
                    };
                self.sample_at_angle(
                    angle,
                    sign,
                    angle_derivative,
                    endpoint_factor_derivative,
                    critical,
                )
            }
        }
    }

    fn sample_crossing(self, angle: Real, sign: Real) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        let a = self.radial_cosine.mul_add(cosine, self.radial_base);
        let a_derivative = -self.radial_cosine * sine;
        let q = a.mul_add(a, self.axial_coefficient * self.axial_coefficient);
        let q_derivative = 2.0 * a * a_derivative;
        let delta = self.line_constant - self.major * a;
        let delta_derivative = -self.major * a_derivative;
        let coefficient = self.radial_cosine.abs()
            * ((self.major - self.minor) * (self.major + self.minor)).sqrt();
        let signed_factor = sign * coefficient * sine / q;
        let signed_factor_derivative =
            sign * coefficient * (cosine / q - sine * q_derivative / (q * q));
        let foot_radial = self.major + a * delta / q;
        let foot_height = self.axial_coefficient * delta / q;
        let foot_radial_derivative = ((a_derivative * delta + a * delta_derivative) * q
            - a * delta * q_derivative)
            / (q * q);
        let foot_height_derivative =
            self.axial_coefficient * (delta_derivative * q - delta * q_derivative) / (q * q);
        let radial = foot_radial - self.axial_coefficient * signed_factor;
        let height = foot_height + a * signed_factor;
        let radial_derivative =
            foot_radial_derivative - self.axial_coefficient * signed_factor_derivative;
        let height_derivative =
            foot_height_derivative + a_derivative * signed_factor + a * signed_factor_derivative;
        let [ux, uy] = self.radial_axis;
        let radial_x = cosine.mul_add(ux, -sine * uy);
        let radial_y = cosine.mul_add(uy, sine * ux);
        let azimuth_x = -sine * ux - cosine * uy;
        let azimuth_y = -sine * uy + cosine * ux;
        Sample {
            position: [radial * radial_x, radial * radial_y, height],
            derivative: [
                radial_derivative * radial_x + radial * azimuth_x,
                radial_derivative * radial_y + radial * azimuth_y,
                height_derivative,
            ],
        }
    }

    fn endpoint_factor_derivative(self, angle: Real, half: Real, direction: Real) -> Real {
        let (sine, cosine) = angle.sin_cos();
        let a = self.radial_cosine.mul_add(cosine, self.radial_base);
        let a_derivative = -self.radial_cosine * sine;
        let q = a.mul_add(a, self.axial_coefficient * self.axial_coefficient);
        let delta = self.line_constant - self.major * a;
        let discriminant_derivative =
            2.0 * a_derivative * (self.minor * self.minor * a + self.major * delta);
        direction * (discriminant_derivative.abs() * half * 0.5).sqrt() / q
    }

    fn sample_at_angle(
        self,
        angle: Real,
        sign: Real,
        angle_derivative: Real,
        endpoint_factor_derivative: Option<Real>,
        critical: Option<Real>,
    ) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        let a = self.radial_cosine.mul_add(cosine, self.radial_base);
        let a_derivative = -self.radial_cosine * sine;
        let q = a.mul_add(a, self.axial_coefficient * self.axial_coefficient);
        let q_derivative = 2.0 * a * a_derivative;
        let delta = self.line_constant - self.major * a;
        let delta_derivative = -self.major * a_derivative;
        let discriminant_derivative =
            2.0 * a_derivative * (self.minor * self.minor * a + self.major * delta);
        let (root, root_angle_derivative) = if let Some(critical_cosine) = critical {
            let k = self.radial_cosine
                * self.radial_cosine
                * (self.major - self.minor)
                * (self.major + self.minor);
            let radius_difference = (self.major - self.minor) * (self.major + self.minor);
            let other_cosine = 2.0
                * self.radial_cosine
                * (self.line_constant * self.major - radius_difference * self.radial_base)
                / k
                - critical_cosine;
            let (half_sine, half_cosine) = (0.5 * angle).sin_cos();
            let scale = (2.0 * k).sqrt();
            if critical_cosine > 0.0 {
                let distance = (cosine - other_cosine).max(0.0).sqrt();
                let root = scale * half_sine.abs() * distance;
                let derivative = if distance > 0.0 {
                    scale
                        * (0.5 * half_cosine * half_sine.signum() * distance
                            - half_sine.abs() * sine / (2.0 * distance))
                } else {
                    0.0
                };
                (root, derivative)
            } else {
                let distance = (other_cosine - cosine).max(0.0).sqrt();
                let root = scale * half_cosine.abs() * distance;
                let derivative = if distance > 0.0 {
                    scale
                        * (-0.5 * half_sine * half_cosine.signum() * distance
                            + half_cosine.abs() * sine / (2.0 * distance))
                } else {
                    0.0
                };
                (root, derivative)
            }
        } else {
            let root = (self.minor * self.minor * q - delta * delta)
                .max(0.0)
                .sqrt();
            (root, discriminant_derivative / (2.0 * root))
        };
        let root = if endpoint_factor_derivative.is_some() {
            0.0
        } else {
            root
        };
        let root_factor = root / q;
        let factor_derivative = endpoint_factor_derivative.unwrap_or_else(|| {
            angle_derivative * (root_angle_derivative / q - root * q_derivative / (q * q))
        });
        let foot_radial = self.major + a * delta / q;
        let foot_height = self.axial_coefficient * delta / q;
        let foot_radial_derivative = ((a_derivative * delta + a * delta_derivative) * q
            - a * delta * q_derivative)
            / (q * q);
        let foot_height_derivative =
            self.axial_coefficient * (delta_derivative * q - delta * q_derivative) / (q * q);
        let radial = foot_radial - sign * self.axial_coefficient * root_factor;
        let height = foot_height + sign * a * root_factor;
        let radial_derivative = foot_radial_derivative * angle_derivative
            - sign * self.axial_coefficient * factor_derivative;
        let height_derivative = foot_height_derivative * angle_derivative
            + sign * (a_derivative * angle_derivative * root_factor + a * factor_derivative);
        let [ux, uy] = self.radial_axis;
        let radial_x = cosine.mul_add(ux, -sine * uy);
        let radial_y = cosine.mul_add(uy, sine * ux);
        let azimuth_x = -sine * ux - cosine * uy;
        let azimuth_y = -sine * uy + cosine * ux;
        Sample {
            position: [radial * radial_x, radial * radial_y, height],
            derivative: [
                radial_derivative * radial_x + radial * azimuth_x * angle_derivative,
                radial_derivative * radial_y + radial * azimuth_y * angle_derivative,
                height_derivative,
            ],
        }
    }
}

fn fit(section: Section, branch: Branch, tolerance: Real) -> Result<NurbsCurve, GeometryError> {
    let mut segments = Vec::new();
    for index in 0..8 {
        fit_interval(
            section,
            branch,
            TURN * index as Real / 8.0,
            TURN * (index + 1) as Real / 8.0,
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
    knots.push(TURN);
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
    use crate::{Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame() -> Frame3 {
        Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn patch(slope: Real, intercept: Real) -> NurbsSurface {
        let corner = |x: Real, y: Real| point(x, y, slope.mul_add(x, intercept));
        NurbsSurface::try_bilinear([
            corner(-6.0, -6.0),
            corner(6.0, -6.0),
            corner(6.0, 6.0),
            corner(-6.0, 6.0),
        ])
        .unwrap()
    }

    #[test]
    fn oblique_torus_plane_sections_form_full_and_turned_loops() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (slope, intercept, expected) in
            [(0.1, 0.0, 2), (1.0, 0.0, 2), (0.1, 1.0, 1), (0.1, -1.0, 1)]
        {
            let patch = patch(slope, intercept);
            for (left, right) in [(&torus, &patch), (&patch, &torus)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| {
                        panic!("slope={slope} intercept={intercept}: {error:?}")
                    });
                assert_eq!(events.len(), expected);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("oblique torus/plane section should be a loop")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    for index in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        let radial = location.x().hypot(location.y());
                        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                        assert!(
                            (location.z() - slope.mul_add(location.x(), intercept)).abs() < 5e-9
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn oblique_sections_clip_to_finite_patches_and_skip_distant_planes() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let corner = |x: Real, y: Real| point(x, y, 0.1 * x);
        let half_patch = NurbsSurface::try_bilinear([
            corner(0.0, -6.0),
            corner(6.0, -6.0),
            corner(6.0, 6.0),
            corner(0.0, 6.0),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&torus, &half_patch, Tolerance::DEFAULT).unwrap();
        assert!(!events.is_empty());
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("half oblique patch should clip loops to arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.x() >= -5e-9);
                assert!((location.z() - 0.1 * location.x()).abs() < 5e-9);
            }
        }
        assert!(
            surface_surface_intersection_events(&torus, &patch(0.1, 2.0), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn oblique_sections_handle_distant_rotated_frames() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        for slope in [0.1, 1.0] {
            let corner = |x: Real, y: Real| rotated.point_at([x, y, slope * x]).unwrap();
            let patch = NurbsSurface::try_bilinear([
                corner(-6.0, -6.0),
                corner(6.0, -6.0),
                corner(6.0, 6.0),
                corner(-6.0, 6.0),
            ])
            .unwrap();
            let events =
                surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("rotated oblique section should retain loops")
                };
                for index in 0..=32 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                    let local = rotated
                        .coordinates_of(curve.evaluate(parameter).unwrap())
                        .unwrap();
                    assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                    assert!((local[2] - slope * local[0]).abs() < 4e-7);
                }
            }
        }
    }

    #[test]
    fn oblique_sections_respect_torus_and_plane_over_varied_tilts() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (slope, intercept) in [
            (0.05, 0.3),
            (0.2, 0.7),
            (0.4, 0.0),
            (0.7, -0.5),
            (1.0, 1.0),
            (2.0, 0.0),
        ] {
            let events = surface_surface_intersection_events(
                &torus,
                &patch(slope, intercept),
                Tolerance::DEFAULT,
            )
            .unwrap_or_else(|error| panic!("slope={slope} intercept={intercept}: {error:?}"));
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("smooth oblique section should be a curve")
                };
                for index in 0..=128 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 128.0);
                    let location = curve.evaluate(parameter).unwrap();
                    let radial = location.x().hypot(location.y());
                    assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!((location.z() - slope.mul_add(location.x(), intercept)).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn centered_critical_oblique_plane_forms_two_crossing_loops() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let slope = 1.0 / 15.0_f64.sqrt();
        let events =
            surface_surface_intersection_events(&torus, &patch(slope, 0.0), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("critical oblique section should retain crossing loops")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            for index in 0..=128 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 128.0);
                let location = curve.evaluate(parameter).unwrap();
                let radial = location.x().hypot(location.y());
                assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                assert!((location.z() - slope * location.x()).abs() < 5e-9);
            }
        }
    }

    #[test]
    fn critical_oblique_section_handles_distant_rotated_frame() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let slope = 1.0 / 15.0_f64.sqrt();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let corner = |x: Real, y: Real| rotated.point_at([x, y, slope * x]).unwrap();
        let patch = NurbsSurface::try_bilinear([
            corner(-6.0, -6.0),
            corner(6.0, -6.0),
            corner(6.0, 6.0),
            corner(-6.0, 6.0),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated critical section should retain crossing loops")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let local = rotated
                    .coordinates_of(curve.evaluate(parameter).unwrap())
                    .unwrap();
                assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                assert!((local[2] - slope * local[0]).abs() < 4e-7);
            }
        }
    }

    #[test]
    fn one_sided_critical_planes_create_paired_loops_or_tangent_points() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for intercept in [1.75, -1.75] {
            let section = patch(0.75, intercept);
            for (left, right) in [(&torus, &section), (&section, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), 2);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("one-sided critical plane should create paired loops")
                    };
                    assert!(curve.is_closed().unwrap());
                    for index in 0..=128 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 128.0);
                        let location = curve.evaluate(parameter).unwrap();
                        let radial = location.x().hypot(location.y());
                        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                        assert!(
                            (location.z() - 0.75_f64.mul_add(location.x(), intercept)).abs() < 5e-9
                        );
                    }
                }
            }
        }
        for intercept in [4.25, -4.25] {
            let events = surface_surface_intersection_events(
                &torus,
                &patch(0.75, intercept),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
                panic!("oblique tangent plane should touch at one point")
            };
            let radial = contact.x().hypot(contact.y());
            assert!(((radial - 4.0).hypot(contact.z()) - 1.0).abs() < 5e-9);
            assert!((contact.z() - 0.75_f64.mul_add(contact.x(), intercept)).abs() < 5e-9);
        }
    }

    #[test]
    fn one_sided_critical_sections_handle_distant_rotated_frame() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        for (intercept, expected) in [(1.75, 2), (4.25, 1)] {
            let corner = |x: Real, y: Real| {
                rotated
                    .point_at([x, y, 0.75_f64.mul_add(x, intercept)])
                    .unwrap()
            };
            let patch = NurbsSurface::try_bilinear([
                corner(-6.0, -6.0),
                corner(6.0, -6.0),
                corner(6.0, 6.0),
                corner(-6.0, 6.0),
            ])
            .unwrap();
            let events = surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("intercept={intercept}: {error:?}"));
            assert_eq!(events.len(), expected);
            for event in events {
                match event {
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                        for index in 0..=32 {
                            let domain = curve.domain();
                            let parameter = *domain.start()
                                + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                            let local = rotated
                                .coordinates_of(curve.evaluate(parameter).unwrap())
                                .unwrap();
                            assert!(
                                ((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs()
                                    < 4e-7
                            );
                            assert!(
                                (local[2] - 0.75_f64.mul_add(local[0], intercept)).abs() < 4e-7
                            );
                        }
                    }
                    SurfaceSurfaceIntersectionEvent::Point(contact) => {
                        let local = rotated.coordinates_of(contact).unwrap();
                        assert!(
                            ((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7
                        );
                        assert!((local[2] - 0.75_f64.mul_add(local[0], intercept)).abs() < 4e-7);
                    }
                }
            }
        }
    }
}
