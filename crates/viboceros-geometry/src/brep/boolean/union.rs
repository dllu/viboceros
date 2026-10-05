//! Multiple operands without repeatedly treating a nonconvex intermediate as convex.
use super::*;

const MAX_INPUTS: usize = 128;

/// One connected material body, retaining any inward cavity shells.
#[derive(Clone, Debug)]
pub struct BrepUnionComponent {
    pub brep: Brep,
    /// Inputs contributing material to this body, in input order. A polyhedral
    /// input with disjoint shells can contribute to several components.
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
    boundary_interactions(breps, false)
}

/// Boundary interactions including positive-length edge contacts for subtraction.
/// Equal regions and point-only contacts remain excluded. Native Difference can
/// replace a target after an edge contact even if its material volume is unchanged.
pub fn convex_brep_subtraction_interactions(
    breps: &[&Brep],
) -> Result<Vec<[usize; 2]>, GeometryError> {
    boundary_interactions(breps, true)
}

fn boundary_interactions(
    breps: &[&Brep],
    include_edges: bool,
) -> Result<Vec<[usize; 2]>, GeometryError> {
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let operands = inputs(breps, &mut budget)?;
    let mut pairs = Vec::new();
    for a in 0..operands.len() {
        for b in a + 1..operands.len() {
            let (left, right) = (&operands[a], &operands[b]);
            let mut interacts = face_interaction(left, right, &mut budget)?
                || face_interaction(right, left, &mut budget)?;
            if !interacts && include_edges && !equal_planes(left, right, &mut budget)? {
                interacts = edge_contact(left, right, &mut budget)?
                    || edge_contact(right, left, &mut budget)?;
            }
            if interacts {
                pairs.push([a, b]);
            }
        }
    }
    Ok(pairs)
}

pub(super) fn face_interaction(
    polygons: &[Polygon<'_>],
    cutters: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for polygon in polygons {
        let (outside, inside) = partition(polygon, cutters, budget)?;
        if inside.is_some()
            && (!outside.is_empty() || coplanar_sense(polygon, cutters, budget)? == Some(false))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn equal_planes(
    left: &[Polygon<'_>],
    right: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for (polygons, cutters) in [(left, right), (right, left)] {
        for p in polygons {
            if coplanar_sense(p, cutters, budget)? != Some(true) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn edge_contact(
    polygons: &[Polygon<'_>],
    cutters: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for p in polygons {
        for i in 0..p.ring.len() {
            let (a, b) = (&p.ring[i], &p.ring[(i + 1) % p.ring.len()]);
            let (mut lo, mut hi) = (Rational::zero(), rational(1.));
            let mut empty = false;
            for plane in cutters {
                budget.spend(1)?;
                let (sa, sb) = (plane.plane_side(a), plane.plane_side(b));
                if sa.is_positive() && sb.is_positive() {
                    empty = true;
                    break;
                }
                if sa.is_positive() || sb.is_positive() {
                    let t = &sa / (&sa - &sb);
                    check_scalar(&t)?;
                    if sa.is_positive() {
                        lo = lo.max(t);
                    } else {
                        hi = hi.min(t);
                    }
                }
                if hi <= lo {
                    empty = true;
                    break;
                }
            }
            if empty {
                continue;
            }
            let start = std::array::from_fn(|j| &a[j] + &lo * (&b[j] - &a[j]));
            let end = std::array::from_fn(|j| &a[j] + &hi * (&b[j] - &a[j]));
            check_point(&start)?;
            check_point(&end)?;
            for plane in cutters {
                budget.spend(1)?;
                if plane.plane_side(&start).is_zero() && plane.plane_side(&end).is_zero() {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Union of up to 128 certified convex polyhedral operands.
///
/// Each original face is clipped against the other original bodies. Nonconvex
/// intermediate results are never passed through a convex-input certificate.
/// Coplanar patches are owned by the earliest input. Disconnected bodies and
/// enclosed voids are separated using exact shell volume and retried exact ray
/// containment, using retained exact face fragments and validated shared topology. Each material component retains its
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
    let output = union_polygons(&operands, &mut budget)?;
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let face_sources = source_faces(breps, &output, &mut budget)?;
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
    let witnesses = operands
        .iter()
        .map(|p| interior_witness(p, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(assemble_union(output, &witnesses, tolerance, &mut budget)?
        .into_iter()
        .map(|body| BrepUnionComponent {
            brep: body.brep,
            boundary_source_indices: body
                .operand_indices
                .iter()
                .copied()
                .filter(|&i| boundary_sources[i])
                .collect(),
            source_indices: body.operand_indices,
            face_sources: body.faces.iter().map(|&i| face_sources[i]).collect(),
        })
        .collect())
}

pub(super) fn union_polygons<'a>(
    operands: &[Vec<Polygon<'a>>],
    budget: &mut Budget,
) -> Result<Vec<Polygon<'a>>, GeometryError> {
    let mut output = Vec::new();
    for (owner, polygons) in operands.iter().enumerate() {
        for polygon in polygons {
            let mut pieces = vec![polygon.clone()];
            for (other, cutters) in operands.iter().enumerate() {
                if owner == other
                    || (owner < other && coplanar_sense(polygon, cutters, budget)? == Some(true))
                {
                    continue;
                }
                let mut next = Vec::new();
                for piece in pieces {
                    next.extend(partition(&piece, cutters, budget)?.0);
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
    Ok(output)
}

pub(super) struct UnionBody {
    pub(super) brep: Brep,
    pub(super) operand_indices: Vec<usize>,
    pub(super) faces: Vec<usize>,
}

pub(super) fn interior_witness(
    polygons: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<ExactPoint, GeometryError> {
    let vertices = polygons
        .iter()
        .flat_map(|p| p.ring.iter().cloned())
        .collect::<BTreeSet<_>>();
    budget.spend(vertices.len())?;
    if vertices.is_empty() {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    let mut p: ExactPoint = std::array::from_fn(|_| Rational::zero());
    for vertex in &vertices {
        for (sum, value) in p.iter_mut().zip(vertex) {
            *sum += value;
        }
        // Pair intersections can introduce unrelated rational denominators.
        // Bound the accumulator before another addition grows it further.
        check_point(&p)?;
    }
    for value in &mut p {
        *value /= Rational::from_integer(vertices.len().into());
    }
    check_point(&p)?;
    Ok(p)
}

pub(super) struct MaterialBody {
    pub(super) brep: Brep,
    pub(super) faces: Vec<usize>,
}

pub(super) fn material_components(
    output: &[Polygon<'_>],
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<MaterialBody>, GeometryError> {
    let exact = output;
    let result = rebuild(output.to_vec(), tolerance, budget)?;
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
                check_scalar(&volume)?;
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
            if contains(p, faces, exact, budget)? {
                candidates.push(index);
            }
        }
        let mut owner = None;
        for &candidate in &candidates {
            let witness = &exact[outer[candidate][0]].ring[0];
            let mut innermost = true;
            for &other in &candidates {
                if other != candidate {
                    innermost &= contains(witness, &outer[other], exact, budget)?;
                }
            }
            if innermost && owner.replace(candidate).is_some() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
        attached[owner.ok_or(GeometryError::UnrepresentableBrepBoolean)?].push(cavity.clone());
    }
    let mut components = Vec::new();
    for (index, mut faces) in outer.into_iter().enumerate() {
        for cavity in &attached[index] {
            faces.extend(cavity);
        }
        faces.sort_unstable();
        let brep = result.duplicate_faces(&faces, tolerance)?;
        if !brep.is_solid() {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        components.push(MaterialBody { brep, faces });
    }
    Ok(components)
}

pub(super) fn assemble_union(
    output: Vec<Polygon<'_>>,
    witnesses: &[ExactPoint],
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<UnionBody>, GeometryError> {
    let bodies = material_components(&output, tolerance, budget)?;
    let mut sources = vec![Vec::new(); bodies.len()];
    for (input, p) in witnesses.iter().enumerate() {
        let mut owner = None;
        for (index, body) in bodies.iter().enumerate() {
            if contains(p, &body.faces, &output, budget)? && owner.replace(index).is_some() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
        sources[owner.ok_or(GeometryError::UnrepresentableBrepBoolean)?].push(input);
    }
    let mut components = Vec::new();
    for (index, body) in bodies.into_iter().enumerate() {
        if sources[index].is_empty() {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        components.push(UnionBody {
            brep: body.brep,
            faces: body.faces,
            operand_indices: std::mem::take(&mut sources[index]),
        });
    }
    components.sort_by_key(|component| component.operand_indices[0]);
    Ok(components)
}

/// Exact ray parity on convex face fragments; ambiguous rays are retried.
pub(super) fn contains(
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
