//! Optional knot-line constraints. Boundary station tables are never changed
//! independently of another incident face; unavailable anchors fail explicitly.
use super::*;
use num_rational::BigRational;

#[cfg(test)]
mod tests;

pub(super) fn unsupported(face: usize, axis: usize, knot: Real) -> GeometryError {
    GeometryError::UnsupportedBrepNormalCrease {
        face,
        direction: if axis == 0 { "U" } else { "V" },
        parameter: knot,
    }
}

fn coordinate(point: Point2, axis: usize) -> Real {
    point.to_array()[axis]
}

/// Knots at which a regular normal may have a discontinuity. Exact matching
/// homogeneous first derivative coefficients prove C1 even at a knot whose
/// multiplicity equals the degree. Other candidates are retained conservatively;
/// rational/projective G1 joins need not satisfy this sufficient C1 test.
pub(super) fn parameters(surface: &NurbsSurface) -> [Vec<Real>; 2] {
    std::array::from_fn(|axis| {
        let (knots, degree, domain) = if axis == 0 {
            (surface.knots_u(), surface.degree_u(), surface.domain_u())
        } else {
            (surface.knots_v(), surface.degree_v(), surface.domain_v())
        };
        let mut first = 0;
        let mut result = Vec::new();
        for group in knots.chunk_by(|a, b| a == b) {
            let knot = group[0];
            if knot > *domain.start()
                && knot < *domain.end()
                && group.len() >= degree
                && (group.len() != degree || !homogeneous_c1(surface, axis, first, degree))
            {
                result.push(knot);
            }
            first += group.len();
        }
        result
    })
}

fn homogeneous_c1(surface: &NurbsSurface, axis: usize, first: usize, degree: usize) -> bool {
    let knots = if axis == 0 {
        surface.knots_u()
    } else {
        surface.knots_v()
    };
    let exact = |x| BigRational::from_float(x).expect("validated finite NURBS coefficient");
    let left_width = exact(knots[first]) - exact(knots[first - 1]);
    let right_width = exact(knots[first + degree]) - exact(knots[first]);
    let count = if axis == 0 {
        surface.control_point_count_v()
    } else {
        surface.control_point_count_u()
    };
    let control = |along, other| {
        if axis == 0 {
            surface.control_point(along, other).unwrap()
        } else {
            surface.control_point(other, along).unwrap()
        }
    };
    for other in 0..count {
        let points = [
            control(first - 2, other),
            control(first - 1, other),
            control(first, other),
        ];
        for k in 0..4 {
            let h = points.map(|p| {
                let weight = exact(p.weight());
                if k == 3 {
                    weight
                } else {
                    exact(p.point().to_array()[k]) * weight
                }
            });
            if (&h[1] - &h[0]) * &right_width != (&h[2] - &h[1]) * &left_width {
                return false;
            }
        }
    }
    true
}

pub(super) fn triangulate(
    parameters: &[Point2],
    lengths: &[usize],
    surface: &NurbsSurface,
    face: usize,
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    let creases = self::parameters(surface);
    if creases.iter().all(Vec::is_empty) {
        return triangulate_trim_region(parameters, lengths);
    }
    let Some(normalization) = TrimParameterNormalization::try_for_triangulation(parameters)? else {
        return Ok(None);
    };
    let normalized = parameters
        .iter()
        .map(|&p| normalization.normalize(p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut ranges = Vec::new();
    let mut start = 0;
    for &length in lengths {
        let end = start + length;
        if end > parameters.len() || length < 3 {
            return Ok(None);
        }
        ranges.push(start..end);
        start = end;
    }
    if ranges.is_empty() {
        return Ok(None);
    }
    let epsilon = 64. * Real::EPSILON;
    let mut constraints = Vec::new();
    for (axis, knots) in creases.iter().enumerate() {
        for &knot in knots {
            for range in &ranges {
                for i in range.clone() {
                    let j = if i + 1 == range.end {
                        range.start
                    } else {
                        i + 1
                    };
                    let a = coordinate(parameters[i], axis);
                    let b = coordinate(parameters[j], axis);
                    if a.min(b) < knot && a.max(b) > knot {
                        // Adding a boundary point here would create a T-junction on a
                        // joined neighbor. Retry through the shared conforming table.
                        return Err(unsupported(face, axis, knot));
                    }
                }
            }
            let mut stations = parameters
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| {
                    (coordinate(p, axis) == knot).then_some((coordinate(p, 1 - axis), i))
                })
                .collect::<Vec<_>>();
            stations.sort_by(|a, b| a.0.total_cmp(&b.0));
            stations.dedup_by(|a, b| a.0 == b.0);
            for pair in stations.windows(2) {
                let a = parameters[pair[0].1];
                let b = parameters[pair[1].1];
                let midpoint = Point2::try_new(
                    crate::interpolate_scalar([a.x(), b.x()], [0., 1.], 0.5)?,
                    crate::interpolate_scalar([a.y(), b.y()], [0., 1.], 0.5)?,
                )?;
                let midpoint = normalization.normalize(midpoint)?;
                if point_in_trim_polygon(midpoint, &normalized[ranges[0].clone()], epsilon)
                    && !ranges[1..]
                        .iter()
                        .any(|r| point_in_trim_polygon(midpoint, &normalized[r.clone()], epsilon))
                    && !ranges
                        .iter()
                        .any(|r| point_on_trim_polygon(midpoint, &normalized[r.clone()], epsilon))
                {
                    constraints.push([pair[0].1, pair[1].1]);
                    if constraints.len() > MAX_CONSTRAINED_TRIM_VERTICES {
                        return Err(GeometryError::TooManyMeshVertices);
                    }
                }
            }
        }
    }
    let triangles = triangulate_trim_region_with_constraints(parameters, lengths, &constraints)?;
    if let Some(triangles) = &triangles {
        validate(parameters, triangles, &creases, face)?;
    }
    Ok(triangles)
}

pub(super) fn validate(
    parameters: &[Point2],
    triangles: &[[u32; 3]],
    creases: &[Vec<Real>; 2],
    face: usize,
) -> Result<(), GeometryError> {
    for triangle in triangles {
        for (axis, knots) in creases.iter().enumerate() {
            let values = triangle.map(|i| coordinate(parameters[i as usize], axis));
            let low = values.into_iter().fold(Real::INFINITY, Real::min);
            let high = values.into_iter().fold(Real::NEG_INFINITY, Real::max);
            let index = knots.partition_point(|&knot| knot <= low);
            if let Some(&knot) = knots.get(index)
                && knot < high
            {
                return Err(unsupported(face, axis, knot));
            }
        }
    }
    Ok(())
}
