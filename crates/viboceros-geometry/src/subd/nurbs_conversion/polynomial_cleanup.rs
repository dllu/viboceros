//! Invert Boehm insertion in linear work; accept only complete roundoff bounds.
use super::*;

pub(super) fn compact(surface: NurbsSurface) -> Result<NurbsSurface, GeometryError> {
    compact_protected(surface, [&[], &[]])
}

pub(super) fn compact_protected(
    mut surface: NurbsSurface,
    protected: [&[f64]; 2],
) -> Result<NurbsSurface, GeometryError> {
    if surface.control_points().iter().any(|p| p.weight() != 1.) {
        return Err(GeometryError::InvalidBrepTopology {
            context: "Polynomial knot cleanup requires unit weights",
        });
    }
    for (axis, protected_knots) in protected.into_iter().enumerate() {
        let mut mandatory = protected_knots.to_vec();
        mandatory.sort_by(f64::total_cmp);
        mandatory.dedup();
        let critical =
            (mandatory.len() >= 3).then(|| (mandatory[2], mandatory[mandatory.len() - 3]));
        let knots = if axis == 0 {
            surface.knots_u()
        } else {
            surface.knots_v()
        };
        let mut candidates = knots
            .iter()
            .copied()
            .filter(|&t| t > 0. && t < 1.)
            .collect::<Vec<_>>();
        // Remove the finest dyadic breaks first. This keeps neighboring span
        // ratios bounded while undoing subdivision's knot refinements.
        candidates.sort_by_cached_key(|&t| {
            let mut value = t;
            let mut depth = 0;
            while value.fract() != 0. && depth < 53 {
                value *= 2.;
                depth += 1;
            }
            std::cmp::Reverse(depth)
        });
        for t in candidates {
            if protected_knots.contains(&t) {
                continue;
            }
            if let Some(candidate) = remove(&surface, axis, t)? {
                surface = candidate;
            } else if critical.is_some_and(|(start, end)| t < start || t > end) {
                // Continuity uses the first three breaks. Resolve the rare
                // ill-conditioned inverse there with the existing basis solve,
                // under the same complete roundoff bound.
                let proposal = if axis == 0 {
                    surface.try_remove_knot_u_near(t)
                } else {
                    surface.try_remove_knot_v_near(t)
                };
                if let Ok(candidate) = proposal {
                    let knots = if axis == 0 {
                        surface.knots_u()
                    } else {
                        surface.knots_v()
                    };
                    let multiplicity = knots.iter().filter(|&&k| k == t).count();
                    if accepted(&surface, &candidate, axis, t, multiplicity)? {
                        surface = candidate;
                    }
                }
            }
        }
    }
    Ok(surface)
}

fn remove(
    surface: &NurbsSurface,
    axis: usize,
    t: f64,
) -> Result<Option<NurbsSurface>, GeometryError> {
    let (nu, nv) = (
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    );
    let (count, rows, degree, knots) = if axis == 0 {
        (nu, nv, surface.degree_u(), surface.knots_u())
    } else {
        (nv, nu, surface.degree_v(), surface.knots_v())
    };
    if count <= degree + 1 {
        return Ok(None);
    }
    let Some(at) = knots.iter().position(|&k| k == t) else {
        return Ok(None);
    };
    let multiplicity = knots.iter().filter(|&&k| k == t).count();
    let mut reduced = knots.to_vec();
    reduced.remove(at);
    let k = reduced.partition_point(|&v| v <= t) - 1;
    let left = k - degree;
    let right = k - (multiplicity - 1);
    let size = count - 1;
    let index = |i: usize, row: usize, n: usize| {
        if axis == 0 {
            row * n + i
        } else {
            i * rows + row
        }
    };
    let mut points = vec![[0.; 3]; size * rows];
    for row in 0..rows {
        for i in 0..=left {
            points[index(i, row, size)] = surface.control_points()[index(i, row, count)]
                .point()
                .to_array();
        }
        for i in right..size {
            points[index(i, row, size)] = surface.control_points()[index(i + 1, row, count)]
                .point()
                .to_array();
        }
        let width = right - left;
        let mut forward = vec![[0.; 3]; width + 1];
        let mut backward = forward.clone();
        let mut fscale = vec![1.; width + 1];
        let mut bscale = fscale.clone();
        forward[0] = points[index(left, row, size)];
        backward[width] = points[index(right, row, size)];
        for i in left + 1..right {
            let alpha = (t - reduced[i]) / (reduced[i + degree] - reduced[i]);
            if !(0. < alpha && alpha <= 1.) {
                return Ok(None);
            }
            let q = surface.control_points()[index(i, row, count)]
                .point()
                .to_array();
            let previous = forward[i - left - 1];
            forward[i - left] =
                std::array::from_fn(|a| q[a] + ((1. - alpha) / alpha) * (q[a] - previous[a]));
            let beta = (1. - alpha) / alpha;
            fscale[i - left] = 1. + beta + beta * fscale[i - left - 1];
        }
        for i in (left + 2..=right).rev() {
            let alpha = (t - reduced[i]) / (reduced[i + degree] - reduced[i]);
            if !(0. ..1.).contains(&alpha) {
                return Ok(None);
            }
            let q = surface.control_points()[index(i, row, count)]
                .point()
                .to_array();
            let next = backward[i - left];
            let beta = alpha / (1. - alpha);
            backward[i - left - 1] = std::array::from_fn(|a| q[a] + beta * (q[a] - next[a]));
            bscale[i - left - 1] = 1. + beta + beta * bscale[i - left];
        }
        for i in left + 1..right {
            points[index(i, row, size)] = if fscale[i - left] <= bscale[i - left] {
                forward[i - left]
            } else {
                backward[i - left]
            };
        }
    }
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return Ok(None);
    }
    let points = points
        .into_iter()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let candidate = if axis == 0 {
        NurbsSurface::try_new(
            surface.degree_u(),
            surface.degree_v(),
            size,
            nv,
            points,
            reduced,
            surface.knots_v().to_vec(),
        )?
    } else {
        NurbsSurface::try_new(
            surface.degree_u(),
            surface.degree_v(),
            nu,
            size,
            points,
            surface.knots_u().to_vec(),
            reduced,
        )?
    };
    Ok(accepted(surface, &candidate, axis, t, multiplicity)?.then_some(candidate))
}

fn accepted(
    surface: &NurbsSurface,
    candidate: &NurbsSurface,
    axis: usize,
    t: f64,
    multiplicity: usize,
) -> Result<bool, GeometryError> {
    let restored = if axis == 0 {
        candidate.try_insert_knot_u(t, multiplicity)?
    } else {
        candidate.try_insert_knot_v(t, multiplicity)?
    };
    if restored.knots_u() != surface.knots_u() || restored.knots_v() != surface.knots_v() {
        return Ok(false);
    }
    let anchor = surface.control_points()[0].point();
    let mut radius = 0_f64;
    let mut error = 0_f64;
    let mut weight_error = 0_f64;
    let mut minimum = f64::INFINITY;
    for (a, b) in restored
        .control_points()
        .iter()
        .zip(surface.control_points())
    {
        radius = radius.max(anchor.distance_to(b.point())?);
        error = error.max(a.point().distance_to(b.point())?);
        weight_error = weight_error.max((a.weight() - b.weight()).abs());
        minimum = minimum.min(a.weight());
    }
    // Positive unit weights make this a whole tensor-image bound.
    let bound = error + 2. * weight_error * radius / minimum;
    Ok(minimum > 0. && bound <= 64. * f64::EPSILON * radius.max(1.))
}
