//! Exact finite intersections between a plane patch and a rational bilinear patch.

use super::{
    SurfaceSurfaceIntersectionEvent, interpolate_parameter, intersect_curve_with_planar_surface,
    surface_surface_distance_tolerance, weights_have_common_sign,
};
use crate::{
    GeometryError, NurbsCurve, NurbsSurface, Plane, Point3, Real, Tolerance, WeightedPoint3,
};

type HomogeneousPoint = [Real; 4];

pub(super) fn intersect(
    surface: &NurbsSurface,
    planar_surface: &NurbsSurface,
    plane: Plane,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    if !weights_have_common_sign(surface.control_points().iter().map(|point| point.weight())) {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "mixed-sign rational bilinear weights",
        });
    }
    let controls = [
        surface.control_point(0, 0).expect("bilinear control net"),
        surface.control_point(1, 0).expect("bilinear control net"),
        surface.control_point(0, 1).expect("bilinear control net"),
        surface.control_point(1, 1).expect("bilinear control net"),
    ];
    let weight_scale = controls
        .iter()
        .map(|control| control.weight().abs())
        .fold(0.0, Real::max);
    let weight_sign = controls[0].weight().signum();
    let signed_distances = controls
        .iter()
        .map(|control| plane.signed_distance_to(control.point()))
        .collect::<Result<Vec<_>, _>>()?;
    let distance_scale = signed_distances
        .iter()
        .copied()
        .map(Real::abs)
        .fold(0.0, Real::max);
    if distance_scale == 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "coincident bilinear surface and plane",
        });
    }
    let homogeneous = controls.map(|control| {
        let weight = control.weight() / weight_scale * weight_sign;
        let [x, y, z] = control.point().to_array();
        [x * weight, y * weight, z * weight, weight]
    });
    let values: [Real; 4] = std::array::from_fn(|index| {
        signed_distances[index] / distance_scale * homogeneous[index][3]
    });
    let [a0, a1, b0, b1] = values;
    let a_zero = a0 == 0.0 && a1 == 0.0;
    let b_zero = b0 == 0.0 && b1 == 0.0;
    let a_root = linear_root(a0, a1);
    let b_root = linear_root(b0, b1);
    let common_root = if a_zero {
        b_root
    } else if b_zero {
        a_root
    } else if let (Some(first), Some(second)) = (a_root, b_root) {
        ((first - second).abs() <= 128.0 * Real::EPSILON).then_some(first)
    } else {
        None
    };

    let mut events = Vec::new();
    let distance_tolerance = surface_surface_distance_tolerance(surface, planar_surface, tolerance);
    let u_domain = surface.domain_u();
    let v_domain = surface.domain_v();
    if let Some(root) = common_root {
        let u = interpolate_parameter(*u_domain.start(), *u_domain.end(), root);
        events.extend(intersect_curve_with_planar_surface(
            &surface.isocurve_v(u)?,
            planar_surface,
            tolerance,
        )?);
        let a_slope = a1 - a0;
        let b_slope = b1 - b0;
        if a_slope != b_slope {
            let fraction = a_slope / (a_slope - b_slope);
            if (0.0..=1.0).contains(&fraction) {
                let v = interpolate_parameter(*v_domain.start(), *v_domain.end(), fraction);
                events.extend(intersect_curve_with_planar_surface(
                    &surface.isocurve_u(v)?,
                    planar_surface,
                    tolerance,
                )?);
            }
        }
        return remove_redundant_points(events, tolerance, distance_tolerance);
    }
    if a_zero || b_zero {
        let v = if a_zero {
            *v_domain.start()
        } else {
            *v_domain.end()
        };
        events.extend(intersect_curve_with_planar_surface(
            &surface.isocurve_u(v)?,
            planar_surface,
            tolerance,
        )?);
        return remove_redundant_points(events, tolerance, distance_tolerance);
    }

    let mut breaks = vec![0.0, 1.0];
    breaks.extend(a_root.filter(|root| *root > 0.0 && *root < 1.0));
    breaks.extend(b_root.filter(|root| *root > 0.0 && *root < 1.0));
    breaks.sort_by(Real::total_cmp);
    breaks.dedup_by(|left, right| (*left - *right).abs() <= 128.0 * Real::EPSILON);
    for interval in breaks.windows(2) {
        let [start, end] = [interval[0], interval[1]];
        let middle = 0.5 * (start + end);
        if linear(a0, a1, middle) * linear(b0, b1, middle) >= 0.0 {
            continue;
        }
        let curve = conic_interval(
            homogeneous,
            values,
            start,
            end,
            [*u_domain.start(), *u_domain.end()],
        )?;
        events.extend(intersect_curve_with_planar_surface(
            &curve,
            planar_surface,
            tolerance,
        )?);
    }
    for (root, v) in [(a_root, *v_domain.start()), (b_root, *v_domain.end())] {
        if let Some(root) = root {
            let u = interpolate_parameter(*u_domain.start(), *u_domain.end(), root);
            let point = surface.evaluate(u, v)?;
            let (plane_u, plane_v) = planar_surface.closest_parameters(point, tolerance)?;
            if planar_surface
                .evaluate(plane_u, plane_v)?
                .distance_to(point)?
                <= distance_tolerance * 2.0
            {
                events.push(SurfaceSurfaceIntersectionEvent::Point(point));
            }
        }
    }
    remove_redundant_points(events, tolerance, distance_tolerance)
}

fn linear(first: Real, second: Real, parameter: Real) -> Real {
    first * (1.0 - parameter) + second * parameter
}

fn linear_root(first: Real, second: Real) -> Option<Real> {
    if first == 0.0 {
        Some(0.0)
    } else if second == 0.0 {
        Some(1.0)
    } else if first.signum() != second.signum() {
        Some(first / (first - second))
    } else {
        None
    }
}

fn conic_interval(
    homogeneous: [HomogeneousPoint; 4],
    [a0, a1, b0, b1]: [Real; 4],
    start: Real,
    end: Real,
    u_domain: [Real; 2],
) -> Result<NurbsCurve, GeometryError> {
    let a_start = linear(a0, a1, start);
    let a_end = linear(a0, a1, end);
    let b_start = linear(b0, b1, start);
    let b_end = linear(b0, b1, end);
    let bottom_start = homogeneous_lerp(homogeneous[0], homogeneous[1], start);
    let bottom_end = homogeneous_lerp(homogeneous[0], homogeneous[1], end);
    let top_start = homogeneous_lerp(homogeneous[2], homogeneous[3], start);
    let top_end = homogeneous_lerp(homogeneous[2], homogeneous[3], end);
    let first = combine(a_start, top_start, b_start, bottom_start);
    let last = combine(a_end, top_end, b_end, bottom_end);
    let middle = std::array::from_fn(|axis| {
        0.5 * (a_start * top_end[axis] + a_end * top_start[axis]
            - b_start * bottom_end[axis]
            - b_end * bottom_start[axis])
    });
    let mut controls = [first, middle, last];
    let sign = controls[1][3].signum();
    let weight_scale = controls
        .iter()
        .map(|control| control[3].abs())
        .fold(0.0, Real::max);
    if sign == 0.0 || weight_scale == 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "degenerate bilinear plane section",
        });
    }
    let controls = controls
        .iter_mut()
        .map(|control| {
            if control[3] * sign <= 0.0 {
                return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                    context: "degenerate bilinear plane section",
                });
            }
            let point = Point3::try_new(
                control[0] / control[3],
                control[1] / control[3],
                control[2] / control[3],
            )?;
            WeightedPoint3::try_new(point, control[3] * sign / weight_scale)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let u_start = interpolate_parameter(u_domain[0], u_domain[1], start);
    let u_end = interpolate_parameter(u_domain[0], u_domain[1], end);
    NurbsCurve::try_new_rational(
        2,
        controls,
        vec![u_start, u_start, u_start, u_end, u_end, u_end],
    )
}

fn homogeneous_lerp(
    first: HomogeneousPoint,
    second: HomogeneousPoint,
    t: Real,
) -> HomogeneousPoint {
    std::array::from_fn(|axis| linear(first[axis], second[axis], t))
}

fn combine(a: Real, top: HomogeneousPoint, b: Real, bottom: HomogeneousPoint) -> HomogeneousPoint {
    std::array::from_fn(|axis| a * top[axis] - b * bottom[axis])
}

fn remove_redundant_points(
    events: Vec<SurfaceSurfaceIntersectionEvent>,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let curves = events
        .iter()
        .filter_map(|event| match event {
            SurfaceSurfaceIntersectionEvent::Curve(curve) => Some(curve.clone()),
            SurfaceSurfaceIntersectionEvent::Point(_) => None,
        })
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for event in events {
        if let SurfaceSurfaceIntersectionEvent::Point(point) = event {
            let covered = curves.iter().try_fold(false, |covered, curve| {
                if covered {
                    return Ok::<bool, GeometryError>(true);
                }
                let parameter = curve.closest_parameter(point, tolerance)?;
                Ok(curve.evaluate(parameter)?.distance_to(point)? <= distance_tolerance * 2.0)
            })?;
            if covered || result.iter().any(|other| {
                matches!(other, SurfaceSurfaceIntersectionEvent::Point(previous)
                    if previous.distance_to(point).is_ok_and(|distance| distance <= distance_tolerance * 2.0))
            }) {
                continue;
            }
        }
        result.push(event);
    }
    Ok(result)
}
