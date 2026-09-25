//! Equal centered tori with different axes meet in two planar sections.
//!
//! A torus at the origin obeys |p|² - 2Rρ + R² - r² = 0, where ρ is the
//! distance from its axis. Subtracting two equal torus equations gives equal
//! radial distances, hence (p·n₁)² = (p·n₂)². Both planes p·(n₁±n₂) = 0
//! must be intersected with the first torus.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsSurface, Plane, Real, Tolerance, Vector3};

pub(super) fn intersect(
    (frame, major, minor): (Frame3, Real, Real),
    second_frame: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_axis = frame.z_axis().as_vector().to_array();
    let second_axis = second_frame.z_axis().as_vector().to_array();
    let span = 2.0 * (major + minor);
    let mut events = Vec::new();
    for sign in [1.0, -1.0] {
        let normal = Vector3::try_new(
            first_axis[0] + sign * second_axis[0],
            first_axis[1] + sign * second_axis[1],
            first_axis[2] + sign * second_axis[2],
        )?;
        let plane_frame = Frame3::try_from_normal(frame.origin(), normal, tolerance)?;
        let corner = |x: Real, y: Real| plane_frame.point_at([x, y, 0.0]);
        let patch = NurbsSurface::try_bilinear([
            corner(-span, -span)?,
            corner(span, -span)?,
            corner(span, span)?,
            corner(-span, span)?,
        ])?;
        events.extend(super::super::torus_plane::intersect(
            (frame, major, minor),
            &patch,
            Plane::new(frame.origin(), plane_frame.z_axis()),
            tolerance,
        )?);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(center: Point3, normal: Vector3) -> Frame3 {
        Frame3::try_from_normal(center, normal, Tolerance::DEFAULT).unwrap()
    }

    fn residual(location: Point3, frame: Frame3) -> Real {
        let [x, y, z] = frame.coordinates_of(location).unwrap();
        ((x.hypot(y) - 4.0).hypot(z) - 1.0).abs()
    }

    #[test]
    fn equal_centered_tori_with_crossed_axes_form_four_closed_sections() {
        let origin = point(0.0, 0.0, 0.0);
        let first_frame = frame(origin, Vector3::try_new(0.0, 0.0, 1.0).unwrap());
        let first = NurbsSurface::try_torus(first_frame, 4.0, 1.0).unwrap();
        for direction in [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 2.0, 3.0],
            [1.0, 0.0, -1.0],
            [1.0e-5, 0.0, 1.0],
            [1.0e-8, 0.0, 1.0],
        ] {
            let second_frame = frame(
                origin,
                Vector3::try_new(direction[0], direction[1], direction[2]).unwrap(),
            );
            let second = NurbsSurface::try_torus(second_frame, 4.0, 1.0).unwrap();
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("direction={direction:?}: {error:?}"));
                assert_eq!(events.len(), 4, "direction={direction:?}");
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("crossed torus axes should form section curves")
                    };
                    assert!(curve.is_closed().unwrap());
                    let domain = curve.domain();
                    for index in 0..=64 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        assert!(residual(location, first_frame) < 5e-9);
                        assert!(residual(location, second_frame) < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn crossed_tori_handle_distant_rotated_frames() {
        let origin = point(1.0e8, -1.0e8, 1.0e8);
        let first_frame = frame(origin, Vector3::try_new(1.0, 2.0, 3.0).unwrap());
        let second_frame = frame(origin, Vector3::try_new(2.0, -1.0, 0.0).unwrap());
        let first = NurbsSurface::try_torus(first_frame, 4.0, 1.0).unwrap();
        let second = NurbsSurface::try_torus(second_frame, 4.0, 1.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated crossed tori should form section curves")
            };
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!(residual(location, first_frame) < 4e-7);
                assert!(residual(location, second_frame) < 4e-7);
            }
        }
    }
}
