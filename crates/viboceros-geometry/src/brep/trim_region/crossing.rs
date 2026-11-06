//! Preserve source loops while splitting their sampled mesh constraints.
use super::*;
use spade::{Intersection, LineIntersectionIterator};
use std::cell::RefCell;
#[cfg(test)]
mod tests;

pub(in crate::brep) fn triangulate(
    parameters: &mut Vec<Point2>,
    lengths: &[usize],
    internal: &[[usize; 2]],
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    if let Some(triangles) =
        triangulate_trim_region_with_constraints(parameters, lengths, internal)?
    {
        return Ok(Some(triangles));
    }
    // Crossing sibling holes are admitted only after each complete hole passes
    // the existing single-hole validator. This does not repair self-crossing
    // loops, holes crossing the outer boundary or unsupported outer trims.
    if lengths.len() < 3
        || lengths.iter().any(|&n| n < 3)
        || lengths
            .iter()
            .try_fold(0usize, |sum, n| sum.checked_add(*n))
            .is_none_or(|sum| sum > parameters.len())
        || parameters.len() > MAX_CONSTRAINED_TRIM_VERTICES
    {
        return Ok(None);
    }
    let mut start = lengths[0];
    for &length in &lengths[1..] {
        let Some(end) = start
            .checked_add(length)
            .filter(|&end| end <= parameters.len())
        else {
            return Ok(None);
        };
        let pair = parameters[..lengths[0]]
            .iter()
            .chain(&parameters[start..end])
            .copied()
            .collect::<Vec<_>>();
        if triangulate_trim_region(&pair, &[lengths[0], length])?.is_none() {
            return Ok(None);
        }
        start = end;
    }
    let Some(normalization) = TrimParameterNormalization::try_for_triangulation(parameters)? else {
        return Ok(None);
    };
    let normalized = parameters
        .iter()
        .map(|&p| normalization.normalize(p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut ranges = Vec::with_capacity(lengths.len());
    let mut start = 0;
    for &length in lengths {
        ranges.push(start..start + length);
        start += length;
    }
    let epsilon = 64. * Real::EPSILON;
    let (vertices, representatives) = unique_trim_triangulation_vertices(&normalized, epsilon);
    let ranges = unique_loops(&ranges, &representatives);
    let Ok(mut cdt) =
        ConstrainedDelaunayTriangulation::<TrimTriangulationVertex>::bulk_load(vertices)
    else {
        return Ok(None);
    };
    let mut handles = vec![None; parameters.len()];
    for vertex in cdt.vertices() {
        handles[vertex.data().source_index] = Some(vertex.fix());
    }
    let Some(mut constraints) =
        trim_triangulation_constraints(&normalized, &ranges, &representatives, epsilon)
    else {
        return Ok(None);
    };
    constraints.extend_from_slice(internal);
    let added = RefCell::new(Vec::new());
    let mut work = 0usize;
    for [a, b] in constraints {
        let (Some(Some(a)), Some(Some(b))) = (
            representatives.get(a).and_then(|&i| handles.get(i)),
            representatives.get(b).and_then(|&i| handles.get(i)),
        ) else {
            return Ok(None);
        };
        let mut splits = 0usize;
        for intersection in LineIntersectionIterator::new_from_handles(&cdt, *a, *b) {
            work += 1;
            if work > 4_000_000 {
                return Err(GeometryError::TooManyMeshVertices);
            }
            if matches!(intersection,Intersection::EdgeIntersection(edge)if edge.is_constraint_edge())
            {
                splits += 1;
            }
        }
        if parameters.len() + added.borrow().len() + splits > MAX_CONSTRAINED_TRIM_VERTICES {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let recovered = cdt.add_constraint_and_split(*a, *b, |position| {
            let mut added = added.borrow_mut();
            let source_index = parameters.len() + added.len();
            added.push([position.x, position.y]);
            TrimTriangulationVertex {
                position,
                source_index,
            }
        });
        if recovered.is_empty() {
            return Ok(None);
        }
        work += recovered.len();
        if work > 4_000_000
            || parameters.len() + added.borrow().len() > MAX_CONSTRAINED_TRIM_VERTICES
        {
            return Err(GeometryError::TooManyMeshVertices);
        }
    }
    let inside = interior_faces(&cdt, &normalized, &ranges, epsilon);
    let mut triangles = Vec::new();
    let (mut area, mut correction) = (0., 0.);
    for face in cdt.inner_faces().filter(|f| inside[f.index()]) {
        let vertices = face.vertices();
        let p = vertices.map(|v| {
            let p = v.position();
            [p.x, p.y]
        });
        let twice = doubled_area(p, epsilon)?;
        if twice <= 0. {
            return Ok(None);
        }
        neumaier_add(&mut area, &mut correction, twice);
        triangles.push(vertices.map(|v| v.data().source_index as u32));
    }
    if triangles.is_empty() {
        return Ok(None);
    }
    // Audit the geometric boundary of the union, rather than subtracting the
    // model's repeated/overlapping loop areas. Every material/exterior side
    // must be an installed boundary constraint.
    let (mut boundary, mut boundary_correction) = (0., 0.);
    for edge in cdt.undirected_edges() {
        let edge = edge.as_directed();
        let (left, right) = (
            inside[edge.face().index()],
            inside[edge.rev().face().index()],
        );
        if left == right {
            continue;
        }
        if !edge.is_constraint_edge() {
            return Ok(None);
        }
        let [a, b] = [edge.from().position(), edge.to().position()];
        let value = a.x.mul_add(b.y, -a.y * b.x);
        neumaier_add(
            &mut boundary,
            &mut boundary_correction,
            if left { value } else { -value },
        );
    }
    let expected = boundary + boundary_correction;
    if expected <= epsilon
        || ((area + correction) - expected).abs()
            > 1024. * Real::EPSILON * (parameters.len() + added.borrow().len()) as Real
    {
        return Ok(None);
    }
    let added = added
        .into_inner()
        .into_iter()
        .map(|p| normalization.denormalize(p))
        .collect::<Result<Vec<_>, _>>()?;
    parameters.extend(added);
    Ok(Some(triangles))
}

// Crossing recovery can introduce valid thin facets in a much larger connected
// region. The polygon's global epsilon is not a minimum triangle area. Resolve
// cancellation from the original binary64 coordinates before accepting them.
fn doubled_area(p: [[Real; 2]; 3], epsilon: Real) -> Result<Real, GeometryError> {
    let fast = polygon_cross(p[0], p[1], p[2]);
    if fast > epsilon {
        return Ok(fast);
    }
    use crate::exact_scalar::{rational, scalar};
    let [a, b, c] = p.map(|p| p.map(rational));
    scalar(&((&b[0] - &a[0]) * (&c[1] - &a[1]) - (&b[1] - &a[1]) * (&c[0] - &a[0])))
}
