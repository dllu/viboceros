//! Cubic sections of a ring torus by a finite axis-parallel offset plane.

use super::{SurfaceSurfaceIntersectionEvent, intersect_curve_with_planar_surface};
use crate::{Frame3, GeometryError, NurbsCurve, NurbsSurface, Plane, Real, Tolerance};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
enum Branch {
    InnerSide,
    OuterSide,
    Joined,
}

#[derive(Clone, Copy)]
struct Section {
    major: Real,
    minor: Real,
    offset: Real,
    outer_lateral: Real,
    branch: Branch,
}

#[derive(Clone, Copy)]
struct Sample {
    lateral: Real,
    vertical: Real,
    lateral_derivative: Real,
    vertical_derivative: Real,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn intersect(
    torus_frame: Frame3,
    major: Real,
    minor: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    signed_distance: Real,
    tolerance: Tolerance,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let offset = signed_distance.abs();
    let outer = major + minor;
    if offset > outer + fit_tolerance {
        return Ok(Vec::new());
    }
    let normal = plane.normal().as_vector();
    let axis = torus_frame.z_axis().as_vector();
    let lateral = axis.cross(normal)?;
    let origin = torus_frame
        .origin()
        .translated(normal.scaled(-signed_distance)?)?;
    let section_frame = Frame3::try_from_directions(origin, lateral, axis, tolerance)?;
    if (offset - outer).abs() <= fit_tolerance {
        let point = section_frame.point_at([0.0, 0.0, 0.0])?;
        let (u, v) = planar_surface.closest_parameters(point, tolerance)?;
        if planar_surface.evaluate(u, v)?.distance_to(point)? <= fit_tolerance {
            return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(point)]);
        }
        return Ok(Vec::new());
    }
    if (offset - (major - minor)).abs() <= fit_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "pinched axis-parallel torus/plane section",
        });
    }

    let outer_lateral = ((outer - offset) * (outer + offset)).sqrt();
    let branches: &[Branch] = if offset < major - minor {
        &[Branch::OuterSide, Branch::InnerSide]
    } else {
        &[Branch::Joined]
    };
    let mut events = Vec::new();
    for &branch in branches {
        let section = Section {
            major,
            minor,
            offset,
            outer_lateral,
            branch,
        };
        let curve = fit(section, section_frame, fit_tolerance)?;
        events.extend(intersect_curve_with_planar_surface(
            &curve,
            planar_surface,
            tolerance,
        )?);
    }
    Ok(events)
}

impl Section {
    fn sample(self, angle: Real) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        match self.branch {
            Branch::OuterSide | Branch::InnerSide => {
                let radius = self.major + self.minor * cosine;
                let lateral_absolute = ((radius - self.offset) * (radius + self.offset)).sqrt();
                let sign = if matches!(self.branch, Branch::OuterSide) {
                    1.0
                } else {
                    -1.0
                };
                Sample {
                    lateral: sign * lateral_absolute,
                    vertical: self.minor * sine,
                    lateral_derivative: -sign * self.minor * radius * sine / lateral_absolute,
                    vertical_derivative: self.minor * cosine,
                }
            }
            Branch::Joined => {
                let lateral = self.outer_lateral * cosine;
                let lateral_derivative = -self.outer_lateral * sine;
                let radial = self.offset.hypot(lateral);
                let denominator = self.major + self.minor + radial;
                let numerator = radial - (self.major - self.minor);
                let root = (numerator / denominator).sqrt();
                let radial_derivative = lateral * lateral_derivative / radial;
                let root_derivative =
                    self.major * radial_derivative / (root * denominator * denominator);
                Sample {
                    lateral,
                    vertical: self.outer_lateral * sine * root,
                    lateral_derivative,
                    vertical_derivative: self.outer_lateral
                        * (cosine * root + sine * root_derivative),
                }
            }
        }
    }
}

fn fit(section: Section, frame: Frame3, fit_tolerance: Real) -> Result<NurbsCurve, GeometryError> {
    let mut segments = 8;
    loop {
        let step = TURN / segments as Real;
        let mut controls = Vec::with_capacity(3 * segments + 1);
        let mut knots = Vec::with_capacity(3 * segments + 5);
        knots.extend([0.0; 4]);
        let first = section.sample(0.0);
        controls.push(frame.point_at([first.lateral, first.vertical, 0.0])?);
        let mut previous = first;
        let mut acceptable = true;
        for segment in 0..segments {
            let start = step * segment as Real;
            let end = step * (segment + 1) as Real;
            let next = if segment + 1 == segments {
                first
            } else {
                section.sample(end)
            };
            let handle = step / 3.0;
            let control = [
                [previous.lateral, previous.vertical],
                [
                    previous.lateral + handle * previous.lateral_derivative,
                    previous.vertical + handle * previous.vertical_derivative,
                ],
                [
                    next.lateral - handle * next.lateral_derivative,
                    next.vertical - handle * next.vertical_derivative,
                ],
                [next.lateral, next.vertical],
            ];
            for fraction in [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875] {
                let expected = section.sample(start + step * fraction);
                let actual = bezier(control, fraction);
                if (actual[0] - expected.lateral).hypot(actual[1] - expected.vertical)
                    > fit_tolerance * 0.25
                {
                    acceptable = false;
                    break;
                }
            }
            controls.push(frame.point_at([control[1][0], control[1][1], 0.0])?);
            controls.push(frame.point_at([control[2][0], control[2][1], 0.0])?);
            controls.push(frame.point_at([control[3][0], control[3][1], 0.0])?);
            knots.extend([end; 3]);
            previous = next;
        }
        knots.push(TURN);
        if acceptable {
            return NurbsCurve::try_new(3, controls, knots);
        }
        if segments >= MAX_SEGMENTS {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        segments *= 2;
    }
}

fn bezier(control: [[Real; 2]; 4], fraction: Real) -> [Real; 2] {
    let complement = 1.0 - fraction;
    let basis = [
        complement.powi(3),
        3.0 * complement * complement * fraction,
        3.0 * complement * fraction * fraction,
        fraction.powi(3),
    ];
    [
        basis
            .iter()
            .zip(control)
            .map(|(b, point)| b * point[0])
            .sum(),
        basis
            .iter()
            .zip(control)
            .map(|(b, point)| b * point[1])
            .sum(),
    ]
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

    fn plane(offset: Real, low: Real, high: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(offset, low, -2.0),
            point(offset, high, -2.0),
            point(offset, high, 2.0),
            point(offset, low, 2.0),
        ])
        .unwrap()
    }

    #[test]
    fn parallel_offset_plane_sections_have_two_or_one_smooth_loops() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (offset, expected) in [(1.0, 2), (3.5, 1), (-3.5, 1), (5.5, 0)] {
            let patch = plane(offset, -6.0, 6.0);
            for (left, right) in [(&torus, &patch), (&patch, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), expected);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("parallel offset torus section should be a curve")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    for index in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        let radial = location.x().hypot(location.y());
                        assert!((location.x() - offset).abs() < 5e-9);
                        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_offset_plane_clips_loops_and_keeps_outer_tangent_point() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let clipped =
            surface_surface_intersection_events(&torus, &plane(1.0, 3.5, 6.0), Tolerance::DEFAULT)
                .unwrap();
        assert!(!clipped.is_empty());
        for event in clipped {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite offset plane should retain arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=16 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.y() >= 3.5 - 5e-9);
            }
        }
        let tangent =
            surface_surface_intersection_events(&torus, &plane(5.0, -6.0, 6.0), Tolerance::DEFAULT)
                .unwrap();
        assert!(matches!(
            tangent.as_slice(),
            [SurfaceSurfaceIntersectionEvent::Point(_)]
        ));
        assert!(matches!(
            surface_surface_intersection_events(&torus, &plane(3.0, -6.0, 6.0), Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }

    #[test]
    fn parallel_offset_plane_handles_distant_rotated_coordinates() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let corner = |y: Real, z: Real| rotated.point_at([1.0, y, z]).unwrap();
        let patch = NurbsSurface::try_bilinear([
            corner(-6.0, -2.0),
            corner(6.0, -2.0),
            corner(6.0, 2.0),
            corner(-6.0, 2.0),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated parallel offset section should be a loop")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let local = rotated
                    .coordinates_of(curve.evaluate(parameter).unwrap())
                    .unwrap();
                assert!((local[0] - 1.0).abs() < 4e-7);
                assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
            }
        }
    }

    #[test]
    fn fitted_sections_respect_dense_meridian_samples() {
        for (offset, branch) in [
            (2.75, Branch::OuterSide),
            (2.75, Branch::InnerSide),
            (3.25, Branch::Joined),
            (4.75, Branch::Joined),
        ] {
            let section = Section {
                major: 4.0,
                minor: 1.0,
                offset,
                outer_lateral: ((5.0 - offset) * (5.0 + offset)).sqrt(),
                branch,
            };
            let curve = fit(section, frame(), 1.0e-9).unwrap();
            for index in 0..=1024 {
                let angle = TURN * index as Real / 1024.0;
                let expected = section.sample(angle);
                let actual = curve.evaluate(angle).unwrap();
                assert!(
                    (actual.x() - expected.lateral).hypot(actual.y() - expected.vertical) < 1.0e-9
                );
            }
        }
    }
}
