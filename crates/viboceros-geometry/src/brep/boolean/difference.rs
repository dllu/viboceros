//! Difference from the union of original cutters, before any rounding.
use super::*;

/// A connected material remainder with any enclosed cavity shells.
#[derive(Clone, Debug)]
pub struct BrepDifferenceComponent {
    pub brep: Brep,
    /// Combined original input/face indices in result-face order. Index zero
    /// is the target; cutter indices start at one. Cutter boundaries reverse
    /// their original outward orientation to face the removed region.
    pub face_sources: Vec<[usize; 2]>,
}

/// Subtracts the union of up to 127 certified convex polyhedral cutters.
///
/// Each target face is clipped outside every original cutter. Cutter union
/// boundary patches inside the target are reversed into new material boundary.
/// Supporting surfaces and exact subdivisions are retained; no intermediate
/// nonconvex result is rounded or recertified. Separate material components
/// retain their inward cavities. Fully removed material yields no components;
/// empty cutters yield a rebuilt certified copy of the target.
///
/// Certificates and work/output limits match `try_boolean_convex`. Rational
/// coordinates and shell-volume accumulators are bounded. Inputs remain
/// unchanged; unsupported or unresolved output returns an error. This API
/// implements mathematical set behavior, including strict interior cavities.
/// Native command policies belong in the document adapter.
pub fn subtract_convex_breps(
    target: &Brep,
    cutters: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepDifferenceComponent>, GeometryError> {
    if cutters.len() > 127 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let target_polygons = extract(target, &mut budget)?;
    let operands = cutters
        .iter()
        .map(|b| extract(b, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    // Redundant cutter intersections introduce artificial coplanar partitions.
    // Compare exact target-clipped convex regions before constructing boundary.
    let mut regions = Vec::new();
    let mut count = 0;
    for cutter in &operands {
        let region = intersection::common_polygons(&[&target_polygons, cutter], &mut budget)?;
        count += region.len();
        bound(count)?;
        regions.push(region);
    }
    let maximal = intersection::maximal_regions(&regions, &mut budget)?;
    let operands = operands
        .into_iter()
        .zip(maximal)
        .filter_map(|(p, keep)| keep.then_some(p))
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for polygon in &target_polygons {
        let mut pieces = vec![polygon.clone()];
        for cutter in &operands {
            // Opposing contact faces are exterior to the cutter's interior.
            if coplanar_sense(polygon, cutter, &mut budget)? == Some(false) {
                continue;
            }
            let mut next = Vec::new();
            for piece in pieces {
                next.extend(partition(&piece, cutter, &mut budget)?.0);
                bound(next.len() + output.len())?;
            }
            pieces = next;
            if pieces.is_empty() {
                break;
            }
        }
        output.extend(pieces);
        bound(output.len())?;
    }
    for (owner, polygons) in operands.iter().enumerate() {
        for polygon in polygons {
            if coplanar_sense(polygon, &target_polygons, &mut budget)?.is_some() {
                continue;
            }
            let Some(inside) = partition(polygon, &target_polygons, &mut budget)?.1 else {
                continue;
            };
            let mut pieces = vec![inside];
            for (other, cutter) in operands.iter().enumerate() {
                if owner == other
                    || (owner < other
                        && coplanar_sense(polygon, cutter, &mut budget)? == Some(true))
                {
                    continue;
                }
                let mut next = Vec::new();
                for piece in pieces {
                    next.extend(partition(&piece, cutter, &mut budget)?.0);
                    bound(next.len() + output.len())?;
                }
                pieces = next;
                if pieces.is_empty() {
                    break;
                }
            }
            output.extend(pieces.into_iter().map(Polygon::reverse));
            bound(output.len())?;
        }
    }
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let originals = std::iter::once(target)
        .chain(cutters.iter().copied())
        .collect::<Vec<_>>();
    let sources = source_faces(&originals, &output, &mut budget)?;
    Ok(union::material_components(&output, tolerance, &mut budget)?
        .into_iter()
        .map(|body| BrepDifferenceComponent {
            brep: body.brep,
            face_sources: body.faces.iter().map(|&i| sources[i]).collect(),
        })
        .collect())
}

fn bound(faces: usize) -> Result<(), GeometryError> {
    if faces > MAX_OUTPUT_FACES {
        Err(GeometryError::BrepBooleanWorkLimit)
    } else {
        Ok(())
    }
}
