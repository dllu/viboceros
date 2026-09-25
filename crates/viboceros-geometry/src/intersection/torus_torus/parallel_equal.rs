//! Equal parallel tori: a symmetry-plane section and an elliptic section.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, NurbsCurve, NurbsSurface, Plane, Real, Tolerance};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
struct EllipticSection {
    frame: Frame3,
    major: Real,
    minor: Real,
    half_offset: Real,
    transverse_radius: Real,
    axis: [Real; 2],
}

#[derive(Clone, Copy)]
enum Branch {
    Full { sign: Real },
    Turned { start: Real, end: Real },
    Crossing { sign: Real },
}

#[derive(Clone, Copy)]
struct Sample {
    position: [Real; 3],
    derivative: [Real; 3],
}

pub(super) fn intersect(
    (frame, major, minor): (Frame3, Real, Real),
    [offset_x, offset_y]: [Real; 2],
    tolerance: Tolerance,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let offset = offset_x.hypot(offset_y);
    let half_offset = 0.5 * offset;
    let axis = [offset_x / offset, offset_y / offset];
    let [ux, uy] = axis;
    let plane_origin = frame.point_at([half_offset * ux, half_offset * uy, 0.0])?;
    let plane_normal = frame.vector_at([ux, uy, 0.0])?.normalized(tolerance)?;
    let span = 2.0 * (major + minor);
    let corner = |lateral: Real, axial: Real| {
        frame.point_at([
            half_offset.mul_add(ux, -lateral * uy),
            half_offset.mul_add(uy, lateral * ux),
            axial,
        ])
    };
    let patch = NurbsSurface::try_bilinear([
        corner(-span, -span)?,
        corner(span, -span)?,
        corner(span, span)?,
        corner(-span, span)?,
    ])?;
    let mut events = super::super::torus_plane::intersect(
        (frame, major, minor),
        &patch,
        Plane::new(plane_origin, plane_normal),
        tolerance,
    )?;

    // Equal torus equations factor into either equal distances from the two
    // ring centers (the plane above) or a sum of those distances equal to 2R.
    // The latter locus is an ellipse in the plane perpendicular to both axes.
    let degeneracy =
        (8.0 * Real::EPSILON * major).max(fit_tolerance * fit_tolerance / (2.0 * major));
    if half_offset > major + degeneracy {
        return Ok(events);
    }
    if (half_offset - major).abs() <= degeneracy {
        let center = frame.point_at([major * ux, major * uy, 0.0])?;
        let normal = frame.vector_at([-uy, ux, 0.0])?.normalized(tolerance)?;
        let circle = Circle3::try_from_frame(center, minor, frame.z_axis(), normal, tolerance)?
            .to_nurbs()?;
        events.push(SurfaceSurfaceIntersectionEvent::Curve(circle));
        return Ok(events);
    }

    let section = EllipticSection {
        frame,
        major,
        minor,
        half_offset,
        transverse_radius: ((major - half_offset) * (major + half_offset)).sqrt(),
        axis,
    };
    let critical = (8.0 * Real::EPSILON * minor).max(fit_tolerance * fit_tolerance / (2.0 * minor));
    let branches = if half_offset < minor - critical {
        vec![Branch::Full { sign: 1.0 }, Branch::Full { sign: -1.0 }]
    } else if (half_offset - minor).abs() <= critical {
        vec![
            Branch::Crossing { sign: 1.0 },
            Branch::Crossing { sign: -1.0 },
        ]
    } else {
        let sine_half = ((half_offset - minor) / (2.0 * half_offset)).sqrt();
        let angle = 2.0 * sine_half.asin();
        vec![
            Branch::Turned {
                start: angle,
                end: std::f64::consts::PI - angle,
            },
            Branch::Turned {
                start: std::f64::consts::PI + angle,
                end: TURN - angle,
            },
        ]
    };
    for branch in branches {
        events.push(SurfaceSurfaceIntersectionEvent::Curve(fit(
            section,
            branch,
            fit_tolerance,
        )?));
    }
    Ok(events)
}

impl EllipticSection {
    fn radicand(self, angle: Real) -> Real {
        let (sine, cosine) = (0.5 * angle).sin_cos();
        let first = (self.minor - self.half_offset) + 2.0 * self.half_offset * sine * sine;
        let second = (self.minor - self.half_offset) + 2.0 * self.half_offset * cosine * cosine;
        first * second
    }

    fn radicand_derivative(self, angle: Real) -> Real {
        let (sine, cosine) = angle.sin_cos();
        2.0 * self.half_offset * self.half_offset * sine * cosine
    }

    fn sample(self, branch: Branch, parameter: Real) -> Sample {
        let (angle, sign, angle_derivative, endpoint_derivative, approximate) = match branch {
            Branch::Full { sign } => (parameter, sign, 1.0, None, None),
            Branch::Crossing { sign } => {
                let angle = parameter;
                let height = sign * self.minor * angle.sin();
                let derivative = sign * self.minor * angle.cos();
                return self.sample_components(angle, 1.0, height, derivative);
            }
            Branch::Turned { start, end } => {
                let t = parameter;
                let middle = 0.5 * (start + end);
                let half = 0.5 * (end - start);
                if t <= std::f64::consts::PI {
                    let angle = middle - half * t.cos();
                    let from_start = 2.0 * half * (0.5 * t).sin().powi(2);
                    let from_end = 2.0 * half * (0.5 * t).cos().powi(2);
                    let approximate = if from_start < 1.0e-8 {
                        Some(self.near_root(start, from_start))
                    } else if from_end < 1.0e-8 {
                        Some(self.near_root(end, -from_end))
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
                    (angle, 1.0, half * t.sin(), endpoint, approximate)
                } else {
                    let return_angle = t - std::f64::consts::PI;
                    let angle = middle + half * return_angle.cos();
                    let from_end = 2.0 * half * (0.5 * return_angle).sin().powi(2);
                    let from_start = 2.0 * half * (0.5 * return_angle).cos().powi(2);
                    let approximate = if from_end < 1.0e-8 {
                        Some(self.near_root(end, -from_end))
                    } else if from_start < 1.0e-8 {
                        Some(self.near_root(start, from_start))
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
                        approximate,
                    )
                }
            }
        };
        let root = approximate
            .unwrap_or_else(|| self.radicand(angle).max(0.0))
            .sqrt();
        let height = sign * root;
        let derivative = endpoint_derivative.unwrap_or_else(|| {
            if root > 0.0 {
                sign * self.radicand_derivative(angle) * angle_derivative / (2.0 * root)
            } else {
                0.0
            }
        });
        self.sample_components(angle, angle_derivative, height, derivative)
    }

    fn near_root(self, angle: Real, delta: Real) -> Real {
        let first = self.radicand_derivative(angle) * delta;
        let second = self.half_offset * self.half_offset * (2.0 * angle).cos() * delta * delta;
        (first + second).max(0.0)
    }

    fn sample_components(
        self,
        angle: Real,
        angle_derivative: Real,
        height: Real,
        height_derivative: Real,
    ) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        let axial = self.half_offset + self.major * cosine;
        let lateral = self.transverse_radius * sine;
        let axial_derivative = -self.major * sine * angle_derivative;
        let lateral_derivative = self.transverse_radius * cosine * angle_derivative;
        let [ux, uy] = self.axis;
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
        }
    }
}

fn fit(
    section: EllipticSection,
    branch: Branch,
    tolerance: Real,
) -> Result<NurbsCurve, GeometryError> {
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
    section: EllipticSection,
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

    fn frame(center: Point3) -> Frame3 {
        Frame3::try_from_normal(
            center,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn equal_parallel_offset_tori_form_plane_and_ellipse_sections() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (offset, expected) in [
            (0.5, 4),
            (2.0, 4),
            (3.0, 4),
            (7.0, 3),
            (8.0, 2),
            (10.0, 1),
            (11.0, 0),
        ] {
            let second = NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), 4.0, 1.0).unwrap();
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("offset={offset}: {error:?}"));
                assert_eq!(events.len(), expected, "offset={offset}");
                for event in events {
                    match event {
                        SurfaceSurfaceIntersectionEvent::Point(location) => {
                            assert!((offset - 10.0).abs() < 1e-12);
                            assert!(location.distance_to(point(5.0, 0.0, 0.0)).unwrap() < 5e-9);
                        }
                        SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                            assert!(curve.is_closed().unwrap(), "offset={offset}");
                            for index in 0..=64 {
                                let domain = curve.domain();
                                let parameter = if index == 64 {
                                    *domain.end()
                                } else {
                                    *domain.start()
                                        + (*domain.end() - *domain.start()) * (index as Real / 64.0)
                                };
                                let location = curve.evaluate(parameter).unwrap();
                                let first_radial = location.x().hypot(location.y());
                                let second_radial = (location.x() - offset).hypot(location.y());
                                assert!(
                                    ((first_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9
                                );
                                assert!(
                                    ((second_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn equal_parallel_offset_tori_keep_near_critical_topology() {
        let first = NurbsSurface::try_torus(frame(point(0.0, 0.0, 0.0)), 4.0, 1.0).unwrap();
        for (half_offset, expected) in [
            (1.0 - 1.0e-12, 4),
            (1.0 + 1.0e-12, 4),
            (4.0 - 1.0e-12, 3),
            (4.0 + 1.0e-12, 1),
        ] {
            let offset = 2.0 * half_offset;
            let second = NurbsSurface::try_torus(frame(point(offset, 0.0, 0.0)), 4.0, 1.0).unwrap();
            let events = surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("half_offset={half_offset}: {error:?}"));
            assert_eq!(events.len(), expected, "half_offset={half_offset}");
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("near-critical intersection should be a curve")
                };
                let domain = curve.domain();
                for fraction in [0.0, 0.125, 0.25, 0.5, 0.75, 1.0] {
                    let parameter = if fraction == 1.0 {
                        *domain.end()
                    } else {
                        *domain.start() + (*domain.end() - *domain.start()) * fraction
                    };
                    let location = curve.evaluate(parameter).unwrap();
                    let first_radial = location.x().hypot(location.y());
                    let second_radial = (location.x() - offset).hypot(location.y());
                    assert!(((first_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!(((second_radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn equal_parallel_offset_tori_handle_distant_rotated_frames() {
        let first_frame = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_center = first_frame.point_at([0.5, 0.0, 0.0]).unwrap();
        let first = NurbsSurface::try_torus(first_frame, 4.0, 1.0).unwrap();
        let second =
            NurbsSurface::try_torus(first_frame.with_origin(second_center), 4.0, 1.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated offset tori should intersect in loops")
            };
            for index in 0..=16 {
                let domain = curve.domain();
                let parameter = if index == 16 {
                    *domain.end()
                } else {
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0)
                };
                let location = curve.evaluate(parameter).unwrap();
                let local_first = first_frame.coordinates_of(location).unwrap();
                let local_second = first_frame
                    .with_origin(second_center)
                    .coordinates_of(location)
                    .unwrap();
                assert!(
                    ((local_first[0].hypot(local_first[1]) - 4.0).hypot(local_first[2]) - 1.0)
                        .abs()
                        < 4e-7
                );
                assert!(
                    ((local_second[0].hypot(local_second[1]) - 4.0).hypot(local_second[2]) - 1.0)
                        .abs()
                        < 4e-7
                );
            }
        }
    }
}
