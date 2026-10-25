//! Conservative command normalization using the existing closest-point solver.
use super::*;
use crate::{Point2, UnitVector3};

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
            let Ok(direction) = point
                .vector_to(outside)
                .and_then(|d| d.normalized_nonzero())
            else {
                return false;
            };
            let inward = |normal: UnitVector3| {
                normal
                    .as_vector()
                    .dot(direction.as_vector())
                    .is_ok_and(|dot| dot < -tolerance.angular().max(1e-6))
            };
            match surface.normal_at(u, v) {
                Ok(normal) if inward(normal) => {}
                Ok(_) => return false,
                Err(_) => {
                    let Some(normals) = pole_normals(surface, u, v) else {
                        return false;
                    };
                    if !normals.into_iter().all(inward) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// A collapsed boundary has no regular normal. Reuse the renderer's converged
/// one-sided samples on several meridians; never relax normal_at's exact contract.
fn pole_normals(surface: &NurbsSurface, u: Real, v: Real) -> Option<Vec<UnitVector3>> {
    let nu = surface.control_point_count_u();
    let (column, span, fraction) = if u == *surface.domain_u().start()
        && crate::nurbs_surface::knots_are_clamped_at_start(surface.degree_u(), surface.knots_u())
    {
        (0, surface.spans_u().next()?, 0.01)
    } else if u == *surface.domain_u().end()
        && crate::nurbs_surface::knots_are_clamped_at_end(surface.degree_u(), surface.knots_u())
    {
        (nu - 1, surface.spans_u().last()?, 0.99)
    } else {
        return None;
    };
    let controls = surface.control_points();
    if !(1..surface.control_point_count_v())
        .all(|j| controls[j * nu + column].point() == controls[column].point())
    {
        return None;
    }
    let interior = crate::interpolate_scalar([span.0, span.1], [0., 1.], fraction).ok()?;
    let mut normals = Vec::with_capacity(4);
    for v in [
        v,
        surface.parameter_at_v(0.25).ok()?,
        surface.parameter_at_v(0.5).ok()?,
        surface.parameter_at_v(0.75).ok()?,
    ] {
        let p = Point2::try_new(u, v).ok()?;
        let neighbor = Point2::try_new(interior, v).ok()?;
        normals.push(crate::tessellation::limiting_normal(
            surface, p, neighbor, neighbor,
        )?);
    }
    Some(normals)
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
