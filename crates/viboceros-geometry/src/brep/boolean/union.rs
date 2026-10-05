//! Multiple operands without repeatedly treating a nonconvex intermediate as convex.
use super::*;

const MAX_INPUTS: usize = 128;

/// One connected material body, retaining any inward cavity shells.
#[derive(Clone, Debug)]
pub struct BrepUnionComponent {
    pub brep: Brep,
    /// Input indices whose interiors belong to this body, in input order.
    pub source_indices: Vec<usize>,
    /// Inputs with a positive-area patch on the material boundary, including
    /// coplanar patches assigned to another input. Strict interior sources are
    /// excluded. This supports metadata ownership independently of face seams.
    pub boundary_source_indices: Vec<usize>,
    /// Original input/face indices for each result face, before optional merging.
    pub face_sources: Vec<[usize; 2]>,
}

fn inputs<'a>(
    breps: &[&'a Brep],
    budget: &mut Budget,
) -> Result<Vec<Vec<Polygon<'a>>>, GeometryError> {
    if breps.len() > MAX_INPUTS {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    breps.iter().map(|brep| extract(brep, budget)).collect()
}

/// Boundary interactions of certified convex polyhedral operands.
///
/// A proper face crossing, a partial coplanar patch, or an opposing face-area
/// contact is included. Disjoint, strictly nested, equal, and point/edge-only
/// contacts are excluded. This is geometric evidence for command policy; it is
/// not a Boolean success prediction for arbitrary Rhino inputs.
pub fn convex_brep_boundary_interactions(
    breps: &[&Brep],
) -> Result<Vec<[usize; 2]>, GeometryError> {
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let operands = inputs(breps, &mut budget)?;
    let mut pairs = Vec::new();
    for a in 0..operands.len() {
        for b in a + 1..operands.len() {
            let mut interacts = false;
            for (polygons, cutters) in [(&operands[a], &operands[b]), (&operands[b], &operands[a])]
            {
                for polygon in polygons {
                    let (outside, inside) = partition(polygon, cutters, &mut budget)?;
                    if inside.is_some()
                        && (!outside.is_empty()
                            || coplanar_sense(polygon, cutters, &mut budget)? == Some(false))
                    {
                        interacts = true;
                        break;
                    }
                }
                if interacts {
                    break;
                }
            }
            if interacts {
                pairs.push([a, b]);
            }
        }
    }
    Ok(pairs)
}

/// Union of up to 128 certified convex polyhedral operands.
///
/// Each original face is clipped against the other original bodies. Nonconvex
/// intermediate results are never passed through a convex-input certificate.
/// Coplanar patches are owned by the earliest input. Disconnected bodies and
/// enclosed voids are separated using exact shell volume and retried exact ray
/// containment, before geometry is rounded. Each material component retains its
/// cavities. Supporting surfaces and shared edge subdivisions are preserved.
///
/// Input certificates and resource/output limits match `try_boolean_convex`.
/// Empty input returns no components; inputs are unchanged. Singular output
/// contacts and unresolved rounded topology return errors.
pub fn union_convex_breps(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepUnionComponent>, GeometryError> {
    if breps.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let operands = inputs(breps, &mut budget)?;
    let mut output = Vec::new();
    for (owner, polygons) in operands.iter().enumerate() {
        for polygon in polygons {
            let mut pieces = vec![polygon.clone()];
            for (other, cutters) in operands.iter().enumerate() {
                if owner == other
                    || (owner < other
                        && coplanar_sense(polygon, cutters, &mut budget)? == Some(true))
                {
                    continue;
                }
                let mut next = Vec::new();
                for piece in pieces {
                    next.extend(partition(&piece, cutters, &mut budget)?.0);
                    if next.len() + output.len() > MAX_OUTPUT_FACES {
                        return Err(GeometryError::BrepBooleanWorkLimit);
                    }
                }
                pieces = next;
                if pieces.is_empty() {
                    break;
                }
            }
            output.extend(pieces);
            if output.len() > MAX_OUTPUT_FACES {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
        }
    }
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let exact = output.clone();
    let face_sources = exact
        .iter()
        .map(|p| {
            breps
                .iter()
                .enumerate()
                .find_map(|(input, b)| {
                    b.faces
                        .iter()
                        .position(|f| std::ptr::eq(f, p.source))
                        .map(|face| [input, face])
                })
                .expect("each fragment retains an original face")
        })
        .collect::<Vec<_>>();
    let mut boundary_sources = vec![false; breps.len()];
    for source in &face_sources {
        boundary_sources[source[0]] = true;
    }
    for (owner, polygons) in operands.iter().enumerate() {
        if boundary_sources[owner] {
            continue;
        }
        for polygon in polygons {
            let mut pieces = vec![polygon.clone()];
            for (other, cutters) in operands.iter().enumerate() {
                if owner == other || coplanar_sense(polygon, cutters, &mut budget)? == Some(true) {
                    continue;
                }
                let mut next = Vec::new();
                for piece in pieces {
                    next.extend(partition(&piece, cutters, &mut budget)?.0);
                    if next.len() > MAX_OUTPUT_FACES {
                        return Err(GeometryError::BrepBooleanWorkLimit);
                    }
                }
                pieces = next;
                if pieces.is_empty() {
                    break;
                }
            }
            if !pieces.is_empty() {
                boundary_sources[owner] = true;
                break;
            }
        }
    }
    let result = rebuild(output, tolerance, &mut budget)?;
    let shells = result.edge_connected_face_components();
    let mut outer = Vec::new();
    let mut cavities = Vec::new();
    for faces in shells {
        let mut volume = Rational::zero();
        for &face in &faces {
            let ring = &exact[face].ring;
            budget.spend(ring.len())?;
            for i in 1..ring.len() - 1 {
                volume += dot(&ring[0], &cross(&ring[i], &ring[i + 1]));
            }
        }
        if volume.is_positive() {
            outer.push(faces);
        } else if volume.is_negative() {
            cavities.push(faces);
        } else {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
    }
    let mut attached = vec![Vec::new(); outer.len()];
    for cavity in &cavities {
        let p = &exact[cavity[0]].ring[0];
        let mut candidates = Vec::new();
        for (index, faces) in outer.iter().enumerate() {
            if contains(p, faces, &exact, &mut budget)? {
                candidates.push(index);
            }
        }
        let mut owner = None;
        for &candidate in &candidates {
            let witness = &exact[outer[candidate][0]].ring[0];
            let mut innermost = true;
            for &other in &candidates {
                if other != candidate {
                    innermost &= contains(witness, &outer[other], &exact, &mut budget)?;
                }
            }
            if innermost && owner.replace(candidate).is_some() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
        attached[owner.ok_or(GeometryError::UnrepresentableBrepBoolean)?].push(cavity.clone());
    }
    let mut sources = vec![Vec::new(); outer.len()];
    for (input, brep) in breps.iter().enumerate() {
        // The average of all vertices lies strictly inside a full-dimensional
        // convex operand, so it cannot be on the resulting union boundary.
        let mut p: ExactPoint = std::array::from_fn(|_| Rational::zero());
        budget.spend(brep.vertices.len())?;
        for vertex in &brep.vertices {
            for (sum, value) in p.iter_mut().zip(point(vertex.point)) {
                *sum += value;
            }
        }
        for value in &mut p {
            *value /= Rational::from_integer(brep.vertices.len().into());
        }
        let mut owner = None;
        for (index, faces) in outer.iter().enumerate() {
            if !contains(&p, faces, &exact, &mut budget)? {
                continue;
            }
            let mut inside_void = false;
            for cavity in &attached[index] {
                inside_void |= contains(&p, cavity, &exact, &mut budget)?;
            }
            if !inside_void && owner.replace(index).is_some() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
        sources[owner.ok_or(GeometryError::UnrepresentableBrepBoolean)?].push(input);
    }
    let mut components = Vec::new();
    for (index, mut faces) in outer.into_iter().enumerate() {
        for cavity in &attached[index] {
            faces.extend(cavity);
        }
        faces.sort_unstable();
        let brep = result.duplicate_faces(&faces, tolerance)?;
        if !brep.is_solid() || sources[index].is_empty() {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        components.push(BrepUnionComponent {
            brep,
            boundary_source_indices: sources[index]
                .iter()
                .copied()
                .filter(|&source| boundary_sources[source])
                .collect(),
            source_indices: std::mem::take(&mut sources[index]),
            face_sources: faces.iter().map(|&i| face_sources[i]).collect(),
        });
    }
    components.sort_by_key(|component| component.source_indices[0]);
    Ok(components)
}

/// Exact ray parity on convex face fragments; ambiguous rays are retried.
fn contains(
    p: &ExactPoint,
    faces: &[usize],
    polygons: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for k in 1..=32usize {
        let direction = [
            rational(1.),
            Rational::from_integer(k.into()),
            Rational::from_integer((k * k).into()),
        ];
        let mut hits = 0;
        let mut ambiguous = false;
        for &face in faces {
            budget.spend(1)?;
            let polygon = &polygons[face];
            let denominator = dot(&polygon.normal, &direction);
            let numerator = -polygon.plane_side(p);
            if denominator.is_zero() {
                if numerator.is_zero() {
                    ambiguous = true;
                    break;
                }
                continue;
            }
            let t = numerator / denominator;
            if !t.is_positive() {
                continue;
            }
            let q: ExactPoint = std::array::from_fn(|i| &p[i] + &t * &direction[i]);
            check_point(&q)?;
            let mut boundary = false;
            let mut inside = true;
            for i in 0..polygon.ring.len() {
                budget.spend(1)?;
                let a = &polygon.ring[i];
                let b = &polygon.ring[(i + 1) % polygon.ring.len()];
                let side = dot(&polygon.normal, &cross(&sub(b, a), &sub(&q, a)));
                if side.is_negative() {
                    inside = false;
                    break;
                }
                boundary |= side.is_zero();
            }
            if inside {
                if boundary {
                    ambiguous = true;
                    break;
                }
                hits += 1;
            }
        }
        if !ambiguous {
            return Ok(hits % 2 == 1);
        }
    }
    Err(GeometryError::UnrepresentableBrepBoolean)
}
