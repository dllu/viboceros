//! Match the open loft's boundary handles to the selected edge's tangent plane.
use super::*;
use viboceros_geometry::{GeometryError, Vector3};

pub(super) fn requested_supports(
    active: [bool; 2],
    supports: [Option<&TangentSupport>; 2],
) -> Result<[Option<&TangentSupport>; 2], CommandError> {
    let [start, end] = supports;
    Ok([
        if active[0] {
            Some(start.ok_or(CommandError::Usage(
                "MatchStartTangent requires a start edge with one supporting face",
            ))?)
        } else {
            None
        },
        if active[1] {
            Some(end.ok_or(CommandError::Usage(
                "MatchEndTangent requires an end edge with one supporting face",
            ))?)
        } else {
            None
        },
    ])
}

pub(super) fn surface(
    curves: &[NurbsCurve],
    style: LoftStyle,
    ends: LoftEndPoints,
    supports: [Option<&TangentSupport>; 2],
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    let surface = viboceros_smlib::loft_surface_with_matched_ends(
        curves,
        style,
        ends,
        supports.map(|support| support.is_some()),
        |side, point, guide| {
            let support = supports[side].expect("active Loft tangent support");
            let normal = normal_on_support(&support.target.surface, point, tolerance)?;
            Ok(normal.cross(guide.normalized_nonzero()?.as_vector())?)
        },
    )
    .map_err(knot_refinement::native_geometry_error)?;
    for (u, support) in [*surface.domain_u().start(), *surface.domain_u().end()]
        .into_iter()
        .zip(supports)
    {
        if let Some(support) = support {
            validate_match(&surface, u, support, tolerance)?;
        }
    }
    Ok(surface)
}

fn normal_on_support(
    support: &NurbsSurface,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Vector3, GeometryError> {
    let (u, v) = support.closest_parameters(point, tolerance)?;
    if point.distance_to(support.evaluate(u, v)?)? > tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "Loft tangent boundary does not follow its supporting surface",
        });
    }
    Ok(support.normal_at(u, v)?.as_vector())
}

fn validate_match(
    loft: &NurbsSurface,
    u: Real,
    support: &TangentSupport,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    // Check the interior of every active span. An exact knot can belong to
    // different incident spans on the loft and on the original face.
    let domain = loft.domain_v();
    let limit = tolerance.angular().min(std::f64::consts::FRAC_PI_2).sin();
    for interval in loft.knots_v().windows(2) {
        let [a, b] = [interval[0], interval[1]];
        if a >= b || a < *domain.start() || b > *domain.end() {
            continue;
        }
        for fraction in [0.0625, 0.25, 0.5, 0.75, 0.9375] {
            let v = a + fraction * (b - a);
            let point = loft.evaluate(u, v)?;
            let edge = support.guide.curve_ref();
            let closest = edge.closest_parameter(point, tolerance)?;
            if point.distance_to(edge.evaluate(closest)?)? > tolerance.absolute() {
                return Err(GeometryError::Degenerate {
                    context: "Loft section compatibility changed the matched boundary edge",
                });
            }
            let normal = normal_on_support(&support.target.surface, point, tolerance)?;
            if normal.cross(loft.normal_at(u, v)?.as_vector())?.length()? > limit {
                return Err(GeometryError::Degenerate {
                    context: "Loft could not match the supporting face within angular tolerance",
                });
            }
        }
    }
    Ok(())
}
