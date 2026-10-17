//! Choose a consecutive contact interval on a fitted supporting circle.
use crate::{Circle3, CircularArc3, GeometryError, Point3, Real, Tolerance};
use std::f64::consts::TAU;

fn angle(circle: Circle3, point: Point3, tolerance: Tolerance) -> Result<Real, GeometryError> {
    let radial = circle.center().vector_to(point)?;
    let x = radial.dot(circle.x_axis().as_vector())?;
    let y = radial.dot(circle.y_axis().as_vector())?;
    if x.hypot(y) <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "tangent arc choice has no radial direction",
        });
    }
    Ok(y.atan2(x).rem_euclid(TAU))
}

/// The picked direction chooses the interval between neighboring contacts,
/// retaining the supporting circle's orientation. It may exclude other
/// contacts from the returned arc. Contacts must lie on the supporting circle;
/// the choice point is projected into its plane.
pub fn try_tangent_arc(
    circle: Circle3,
    contacts: &[Point3],
    choice: Point3,
    tolerance: Tolerance,
) -> Result<CircularArc3, GeometryError> {
    let mut angles = Vec::with_capacity(contacts.len());
    let distance = tolerance
        .absolute()
        .max(128. * Real::EPSILON * circle.radius());
    for &p in contacts {
        let radial = circle.center().vector_to(p)?;
        if (radial.length()? - circle.radius()).abs() > distance
            || radial.dot(circle.normal()?.as_vector())?.abs() > distance
        {
            return Err(GeometryError::Degenerate {
                context: "tangent arc contact is not on its circle",
            });
        }
        angles.push(angle(circle, p, tolerance)?);
    }
    angles.sort_by(Real::total_cmp);
    let angular = tolerance.angular().max(distance / circle.radius());
    angles.dedup_by(|a, b| (*a - *b).abs() <= angular);
    if angles.len() > 1 && angles[0] + TAU - angles[angles.len() - 1] <= angular {
        angles.pop();
    }
    if angles.len() < 2 {
        return Err(GeometryError::Degenerate {
            context: "tangent arc needs distinct contacts",
        });
    }
    let picked = angle(circle, choice, tolerance)?;
    let next = angles.partition_point(|a| *a <= picked);
    let (start, end) = if next == 0 {
        (angles[angles.len() - 1] - TAU, angles[0])
    } else if next == angles.len() {
        (angles[next - 1], angles[0] + TAU)
    } else {
        (angles[next - 1], angles[next])
    };
    CircularArc3::try_from_circle_angles(circle, start..=end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;
    #[test]
    fn tangent_arc_chooses_all_contact_intervals_in_spatial_frames() {
        let tol = Tolerance::DEFAULT;
        let circle = Circle3::try_new(
            Point3::try_new(7., -3., 9.).unwrap(),
            3.,
            Vector3::try_new(2., -3., 4.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tol,
        )
        .unwrap();
        let contacts = [0., 0.8, 4.2].map(|a| circle.point_at_angle(a).unwrap());
        for (a, b) in [(0., 0.8), (0.8, 4.2), (4.2, TAU)] {
            let pick = circle.point_at_angle((a + b) * 0.5).unwrap();
            let arc = try_tangent_arc(circle, &contacts, pick, tol).unwrap();
            assert!((arc.sweep_radians() - (b - a)).abs() < 1e-12);
            assert!(
                arc.start()
                    .unwrap()
                    .distance_to(circle.point_at_angle(a).unwrap())
                    .unwrap()
                    < 1e-12
            );
            assert!(
                arc.end()
                    .unwrap()
                    .distance_to(circle.point_at_angle(b).unwrap())
                    .unwrap()
                    < 1e-12
            );
            assert!(
                arc.normal()
                    .unwrap()
                    .as_vector()
                    .dot(circle.normal().unwrap().as_vector())
                    .unwrap()
                    > 1. - 1e-14
            );
        }
    }
    #[test]
    fn tangent_arc_coalesces_duplicate_seam_contacts_and_rejects_invalid_choices() {
        let tol = Tolerance::DEFAULT;
        let center = Point3::try_new(0., 0., 0.).unwrap();
        let circle = Circle3::try_new(
            center,
            2.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tol,
        )
        .unwrap();
        let a = circle.point_at_angle(0.).unwrap();
        let b = circle.point_at_angle(2.).unwrap();
        let contacts = [a, circle.point_at_angle(TAU - 1e-13).unwrap(), b];
        let arc =
            try_tangent_arc(circle, &contacts, circle.point_at_angle(1.).unwrap(), tol).unwrap();
        assert!((arc.sweep_radians() - 2.).abs() < 1e-12);
        assert!(try_tangent_arc(circle, &[a, a], b, tol).is_err());
        assert!(try_tangent_arc(circle, &[a, b], center, tol).is_err());
        assert!(try_tangent_arc(circle, &[a, center], b, tol).is_err());
    }
}
