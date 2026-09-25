//! Fitted intersections of a ring torus with an oblique finite planar patch.

use super::{SurfaceSurfaceIntersectionEvent, intersect_curve_with_planar_surface};
use crate::{Frame3, GeometryError, NurbsCurve, NurbsSurface, Plane, Real, Tolerance};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
struct Section {
    frame: Frame3,
    major: Real,
    minor: Real,
    horizontal: Real,
    vertical: Real,
    constant: Real,
    radial_axis: [Real; 2],
}

#[derive(Clone, Copy)]
enum Branch {
    Full { sign: Real },
    Turned { start: Real, end: Real },
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
        horizontal,
        vertical,
        constant: -signed_distance,
        radial_axis: [horizontal_x / horizontal, horizontal_y / horizontal],
    };
    let vertex = section.vertex().clamp(-1.0, 1.0);
    let maximum = section.discriminant(vertex);
    if maximum < 0.0 {
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
    if section.discriminant(-1.0).abs() <= angular_roundoff
        || section.discriminant(1.0).abs() <= angular_roundoff
    {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "singular oblique torus/plane section",
        });
    }
    let radial_scale = (section.horizontal * vertex).hypot(section.vertical);
    if maximum <= fit_tolerance * fit_tolerance * radial_scale * radial_scale {
        return tangent_points(section, vertex, planar_surface, tolerance, fit_tolerance);
    }
    let branches = if minimum_cosine <= -1.0 && maximum_cosine >= 1.0 {
        vec![Branch::Full { sign: 1.0 }, Branch::Full { sign: -1.0 }]
    } else if minimum_cosine <= -1.0 {
        let angle = maximum_cosine.acos();
        vec![Branch::Turned {
            start: angle,
            end: TURN - angle,
        }]
    } else if maximum_cosine >= 1.0 {
        let angle = minimum_cosine.acos();
        vec![Branch::Turned {
            start: -angle,
            end: angle,
        }]
    } else {
        let first = maximum_cosine.acos();
        let second = minimum_cosine.acos();
        vec![
            Branch::Turned {
                start: first,
                end: second,
            },
            Branch::Turned {
                start: TURN - second,
                end: TURN - first,
            },
        ]
    };
    let mut events = Vec::new();
    for branch in branches {
        let curve = fit(section, branch, fit_tolerance)?;
        events.extend(intersect_curve_with_planar_surface(
            &curve,
            planar_surface,
            tolerance,
        )?);
    }
    Ok(events)
}

fn tangent_points(
    section: Section,
    cosine: Real,
    planar_surface: &NurbsSurface,
    tolerance: Tolerance,
    fit_tolerance: Real,
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
        let local = section.sample_at_angle(angle, 1.0, 0.0, Some(0.0)).position;
        let point = section.frame.point_at(local)?;
        let (u, v) = planar_surface.closest_parameters(point, tolerance)?;
        if planar_surface.evaluate(u, v)?.distance_to(point)? <= fit_tolerance {
            events.push(SurfaceSurfaceIntersectionEvent::Point(point));
        }
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
        self.constant * self.major
            / (self.horizontal * (self.major - self.minor) * (self.major + self.minor))
    }

    fn discriminant(self, cosine: Real) -> Real {
        let a = self.horizontal * cosine;
        let q = a.mul_add(a, self.vertical * self.vertical);
        let delta = self.constant - self.major * a;
        self.minor * self.minor * q - delta * delta
    }

    fn sample(self, branch: Branch, parameter: Real) -> Sample {
        match branch {
            Branch::Full { sign } => self.sample_at_angle(parameter, sign, 1.0, None),
            Branch::Turned { start, end } => {
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
                self.sample_at_angle(angle, sign, angle_derivative, endpoint_factor_derivative)
            }
        }
    }

    fn endpoint_factor_derivative(self, angle: Real, half: Real, direction: Real) -> Real {
        let (sine, cosine) = angle.sin_cos();
        let a = self.horizontal * cosine;
        let a_derivative = -self.horizontal * sine;
        let q = a.mul_add(a, self.vertical * self.vertical);
        let delta = self.constant - self.major * a;
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
    ) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        let a = self.horizontal * cosine;
        let a_derivative = -self.horizontal * sine;
        let q = a.mul_add(a, self.vertical * self.vertical);
        let q_derivative = 2.0 * a * a_derivative;
        let delta = self.constant - self.major * a;
        let delta_derivative = -self.major * a_derivative;
        let discriminant = (self.minor * self.minor * q - delta * delta).max(0.0);
        let root = if endpoint_factor_derivative.is_some() {
            0.0
        } else {
            discriminant.sqrt()
        };
        let root_factor = root / q;
        let discriminant_derivative =
            2.0 * a_derivative * (self.minor * self.minor * a + self.major * delta);
        let factor_derivative = endpoint_factor_derivative.unwrap_or_else(|| {
            angle_derivative
                * (discriminant_derivative / (2.0 * root * q) - root * q_derivative / (q * q))
        });
        let foot_radial = self.major + a * delta / q;
        let foot_height = self.vertical * delta / q;
        let foot_radial_derivative = ((a_derivative * delta + a * delta_derivative) * q
            - a * delta * q_derivative)
            / (q * q);
        let foot_height_derivative =
            self.vertical * (delta_derivative * q - delta * q_derivative) / (q * q);
        let radial = foot_radial - sign * self.vertical * root_factor;
        let height = foot_height + sign * a * root_factor;
        let radial_derivative =
            foot_radial_derivative * angle_derivative - sign * self.vertical * factor_derivative;
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
    fn singular_oblique_transition_is_reported_explicitly() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let slope = 1.0 / 15.0_f64.sqrt();
        assert!(matches!(
            surface_surface_intersection_events(&torus, &patch(slope, 0.0), Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }
}
