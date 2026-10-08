//! Open planar targets use shared and unshared oriented boundary expressions.
use super::*;
#[cfg(test)]
mod tests;

/// Split an open planar target with certified solids and finite planar sheets.
/// Outputs may be closed material bodies or open boundaries with cutter faces.
/// The target's normal chooses its half-space. Physical sheet coverage gates
/// each operation; virtual planning rectangles never become output geometry.
pub fn split_open_polyhedral_brep(
    target: &Brep,
    cutters: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
    if target.is_solid() {
        return Err(unsupported("open planar target required"));
    }
    let refs = std::iter::once(target)
        .chain(cutters.iter().copied())
        .collect::<Vec<_>>();
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let (built, groups) = surface_split::build(&refs, tolerance, &mut budget)?;
    let coplanar = groups
        .iter()
        .filter(|g| {
            !refs[g[0]].is_solid()
                && zero(&cross(
                    &built.operands[0][0].normal,
                    &built.operands[g[0]][0].normal,
                ))
                && built.operands[0][0]
                    .plane_side(&built.operands[g[0]][0].ring[0])
                    .is_zero()
        })
        .collect::<Vec<_>>();
    if !coplanar.is_empty() {
        if groups.len() != 1 {
            return Err(unsupported("mixed coplanar open-target stages"));
        }
        return coplanar_split(&built, coplanar[0], tolerance, &mut budget);
    }
    let mut plan = BrepPolyhedralBooleanPlan::from_arrangement(built, tolerance, budget);
    let inputs = (0..refs.len())
        .map(|i| plan.input(i))
        .collect::<Result<Vec<_>, _>>()?;
    let mut regions = vec![inputs[0].clone()];
    let mut split = false;
    for group in groups {
        let closed = refs[group[0]].is_solid();
        if closed && !plan.sheet_covers_region(&inputs[group[0]], &[0])? {
            continue;
        }
        let mut next = Vec::new();
        for region in regions {
            if !closed && !plan.sheet_covers_boundary_section(&region, &group)? {
                next.push(region);
                continue;
            }
            let inside = plan.combine(
                BrepBooleanOperation::Intersection,
                &[&region, &inputs[group[0]]],
            )?;
            let outside = plan.combine(
                BrepBooleanOperation::Difference,
                &[&region, &inputs[group[0]]],
            )?;
            if plan.is_empty(&inside)? || plan.is_empty(&outside)? {
                next.push(region);
                continue;
            }
            split = true;
            next.push(inside);
            next.push(outside);
        }
        regions = next;
    }
    if !split {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for region in regions {
        for body in plan.export_open_boundary(&region)? {
            result.push(BrepPolyhedralBooleanComponent {
                brep: body.brep,
                face_sources: body.face_sources,
            });
        }
    }
    Ok(result)
}

fn coplanar_split(
    built: &arrangement::Arrangement<'_>,
    group: &[usize],
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
    let mut covered = BTreeMap::new();
    for cell in &built.cells {
        budget.spend(1)?;
        if group.contains(&cell.source[0]) && cell.source_covers_cell {
            covered
                .entry(canonical_ring(&cell.polygon.ring))
                .or_insert(cell);
        }
    }
    let mut primary = Vec::new();
    let mut outside = Vec::new();
    let mut shared = false;
    for cell in &built.cells {
        budget.spend(1)?;
        if cell.source[0] != 0 || !cell.source_covers_cell {
            continue;
        }
        if let Some(other) = covered.get(&canonical_ring(&cell.polygon.ring)) {
            shared = true;
            let mut polygon = cell.polygon.clone();
            polygon.source = other.polygon.source;
            polygon.reversed =
                other.polygon.reversed ^ dot(&polygon.normal, &other.polygon.normal).is_negative();
            primary.push((polygon, other.source));
        } else {
            primary.push((cell.polygon.clone(), cell.source));
            outside.push((cell.polygon.clone(), cell.source));
        }
    }
    if !shared || outside.is_empty() {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for pieces in [primary, outside] {
        let sources = pieces.iter().map(|(_, source)| *source).collect::<Vec<_>>();
        let brep = rebuild_open_boundary(
            pieces.into_iter().map(|(p, _)| p).collect(),
            tolerance,
            budget,
        )?;
        for faces in brep.edge_connected_face_components() {
            result.push(BrepPolyhedralBooleanComponent {
                brep: brep.duplicate_faces(&faces, tolerance)?,
                face_sources: faces.into_iter().map(|i| sources[i]).collect(),
            });
        }
    }
    Ok(result)
}
