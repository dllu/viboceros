//! Common intersection of original convex inputs, with original face ownership.
use super::*;

/// A common convex intersection before optional coplanar face/edge merging.
#[derive(Clone, Debug)]
pub struct BrepConvexIntersection {
    pub brep: Brep,
    /// Original input/face index for each result face, in result face order.
    pub face_sources: Vec<[usize; 2]>,
}

/// Common intersection of up to 128 certified convex polyhedral operands.
///
/// Each original face is clipped inside every other original body. No rounded
/// intermediate is recertified as an input. Same-facing coplanar overlap belongs
/// to the earliest input; opposing contact planes have no volumetric result.
/// Supporting surfaces and exact shared subdivisions are retained. Empty input
/// or an empty volumetric region returns `None`; inputs remain unchanged.
/// Certificates, exact arithmetic, resource limits and rounding validation match
/// `try_boolean_convex`. The nonempty result is a single convex material shell.
pub fn intersect_convex_breps(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Option<BrepConvexIntersection>, GeometryError> {
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    if breps.is_empty() {
        return Ok(None);
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let operands = breps
        .iter()
        .map(|b| extract(b, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    let output = common_polygons(
        &operands.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        &mut budget,
    )?;
    let sources = source_faces(breps, &output, &mut budget)?;
    if output.is_empty() {
        return Ok(None);
    }
    let brep = rebuild(output, tolerance, &mut budget)?;
    if !brep.is_solid() || brep.edge_connected_face_components().len() != 1 {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Ok(Some(BrepConvexIntersection {
        brep,
        face_sources: sources,
    }))
}

fn common_polygons<'a>(
    operands: &[&[Polygon<'a>]],
    budget: &mut Budget,
) -> Result<Vec<Polygon<'a>>, GeometryError> {
    let mut output = Vec::new();
    for (owner, polygons) in operands.iter().enumerate() {
        for polygon in *polygons {
            let mut piece = Some(polygon.clone());
            for (other, cutters) in operands.iter().enumerate() {
                if owner == other {
                    continue;
                }
                let shared = coplanar_sense(polygon, cutters, budget)?;
                if shared == Some(false) || (other < owner && shared == Some(true)) {
                    piece = None;
                    break;
                }
                piece = partition(piece.as_ref().expect("surviving fragment"), cutters, budget)?.1;
                if piece.is_none() {
                    break;
                }
            }
            if let Some(piece) = piece {
                output.push(piece);
                if output.len() > MAX_OUTPUT_FACES {
                    return Err(GeometryError::BrepBooleanWorkLimit);
                }
            }
        }
    }
    Ok(output)
}

/// One connected material component of the intersection of two unions.
#[derive(Clone, Debug)]
pub struct BrepSetIntersection {
    pub brep: Brep,
    /// Nonempty original first/second pair regions belonging to this component.
    pub pairs: Vec<[usize; 2]>,
    /// Pair regions not contained in another pair region. Ordering follows the
    /// first set and then the second set; equivalent regions keep the first.
    pub maximal_pairs: Vec<[usize; 2]>,
    /// Original combined input/face indices for result faces. First-set indices
    /// precede second-set indices, before optional coplanar merging.
    pub face_sources: Vec<[usize; 2]>,
}

/// Intersects the union of each of two sets of certified convex polyhedra.
///
/// Original pair intersections are constructed exactly and unioned without
/// rounding or re-extracting intermediate bodies. Connected material bodies
/// retain enclosed cavities. Empty sets/regions return no bodies. The combined
/// input count is at most 128; total intermediate fragments, output fragments,
/// exact work and rational coordinate size are bounded. Unsupported inputs or
/// unresolved topology return errors without changing inputs.
///
/// This is mathematical set behavior. Native command policies for nested/equal
/// inputs, deletion, selection and metadata belong in the command adapter.
pub fn intersect_convex_brep_sets(
    first: &[&Brep],
    second: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepSetIntersection>, GeometryError> {
    if first.len().saturating_add(second.len()) > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    if first.is_empty() || second.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let originals = first.iter().chain(second).copied().collect::<Vec<_>>();
    let operands = originals
        .iter()
        .map(|b| extract(b, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    let mut regions = Vec::new();
    let mut pairs = Vec::new();
    let mut count = 0usize;
    for a in 0..first.len() {
        for b in 0..second.len() {
            let region = common_polygons(&[&operands[a], &operands[first.len() + b]], &mut budget)?;
            count += region.len();
            if count > MAX_OUTPUT_FACES {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
            if !region.is_empty() {
                regions.push(region);
                pairs.push([a, b]);
            }
        }
    }
    if regions.is_empty() {
        return Ok(Vec::new());
    }
    let mut maximal = vec![true; regions.len()];
    for a in 0..regions.len() {
        for b in 0..regions.len() {
            if a != b
                && contained(&regions[a], &regions[b], &mut budget)?
                && (b < a || !contained(&regions[b], &regions[a], &mut budget)?)
            {
                maximal[a] = false;
                break;
            }
        }
    }
    let kept = regions
        .iter()
        .zip(&maximal)
        .filter_map(|(p, &keep)| keep.then_some(p.clone()))
        .collect::<Vec<_>>();
    let output = union::union_polygons(&kept, &mut budget)?;
    let face_sources = source_faces(&originals, &output, &mut budget)?;
    let witnesses = regions
        .iter()
        .map(|p| union::interior_witness(p, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        union::assemble_union(output, &witnesses, tolerance, &mut budget)?
            .into_iter()
            .map(|body| BrepSetIntersection {
                brep: body.brep,
                pairs: body.operand_indices.iter().map(|&i| pairs[i]).collect(),
                maximal_pairs: body
                    .operand_indices
                    .iter()
                    .filter(|&&i| maximal[i])
                    .map(|&i| pairs[i])
                    .collect(),
                face_sources: body.faces.iter().map(|&i| face_sources[i]).collect(),
            })
            .collect(),
    )
}

fn contained(
    inner: &[Polygon<'_>],
    outer: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for polygon in inner {
        for plane in outer {
            budget.spend(polygon.ring.len())?;
            if polygon
                .ring
                .iter()
                .any(|p| plane.plane_side(p).is_positive())
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
