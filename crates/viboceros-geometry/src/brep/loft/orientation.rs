//! Conservative command normalization using the existing closest-point solver.
use super::*;

/// Only reverse when six outside closest-point probes agree. Conflicting or
/// unsupported probes retain the supplied sense, including self-overlap cases.
/// This sampled command policy is separate from the exact solid classifier.
pub(super) fn all_outside_probes_point_inward(
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> bool {
    let controls = surface.control_points();
    let sign = controls[0].weight().is_sign_positive();
    if controls
        .iter()
        .any(|p| p.weight().is_sign_positive() != sign)
    {
        return false;
    }
    // Same-sign rational weights make the control hull a conservative bound.
    let Ok(bounds) = crate::BoundingBox3::from_points(controls.iter().map(|p| p.point())) else {
        return false;
    };
    let Ok(center) = bounds.center() else {
        return false;
    };
    let low = bounds.min().to_array();
    let high = bounds.max().to_array();
    let extent = (0..3)
        .map(|axis| high[axis] - low[axis])
        .fold(0_f64, Real::max);
    if !extent.is_finite() || extent <= 0. {
        return false;
    }
    for axis in 0..3 {
        for side in [-1., 1.] {
            let mut outside = center.to_array();
            outside[axis] = if side < 0. {
                low[axis] - extent
            } else {
                high[axis] + extent
            };
            let Ok(outside) = Point3::try_from(outside) else {
                return false;
            };
            let Ok((u, v)) = surface.closest_parameters(outside, tolerance) else {
                return false;
            };
            let Ok(point) = surface.evaluate(u, v) else {
                return false;
            };
            let Ok(normal) = surface.normal_at(u, v) else {
                return false;
            };
            let Ok(direction) = point
                .vector_to(outside)
                .and_then(|d| d.normalized_nonzero())
            else {
                return false;
            };
            let Ok(dot) = normal.as_vector().dot(direction.as_vector()) else {
                return false;
            };
            if dot >= -tolerance.angular().max(1e-6) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inward_torus_is_reversed_without_changing_the_outward_surface() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_points(
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
            tolerance,
        )
        .unwrap();
        let outward = NurbsSurface::try_torus(frame, 3., 1.).unwrap();
        let before = outward.clone();
        assert!(outward.normal_at(0., 0.).unwrap().as_vector().x() > 0.99);
        assert!(!all_outside_probes_point_inward(&outward, tolerance));
        let inward = outward.try_reversed_v().unwrap();
        assert!(all_outside_probes_point_inward(&inward, tolerance));
        assert_eq!(outward, before);
    }
}
