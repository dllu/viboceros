//! Boundary evidence for command policy, independent of mathematical success.
use super::*;

/// Proper face crossings, partial coplanar overlap and opposing face-area
/// contacts of certified polyhedral inputs. Equal regions, strict nesting and
/// point/edge-only contacts are excluded. All original inputs are certified.
pub fn polyhedral_brep_boundary_interactions(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<[usize; 2]>, GeometryError> {
    interactions(breps, tolerance, false)
}

/// Boundary interactions including positive-length contacts for subtraction.
/// Equal regions and point-only contacts remain excluded.
pub fn polyhedral_brep_subtraction_interactions(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<[usize; 2]>, GeometryError> {
    interactions(breps, tolerance, true)
}

fn interactions(
    breps: &[&Brep],
    tolerance: Tolerance,
    include_edges: bool,
) -> Result<Vec<[usize; 2]>, GeometryError> {
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let built = arrangement::build(breps, tolerance, &mut budget)?;
    let mut result = Vec::new();
    for a in 0..breps.len() {
        for b in a + 1..breps.len() {
            let mut equal = true;
            for sample in &built.samples {
                budget.spend(1)?;
                if sample.inside[a] != sample.inside[b] {
                    equal = false;
                    break;
                }
            }
            if equal {
                continue;
            }
            let mut faces = BTreeMap::<[usize; 2], [bool; 2]>::new(); // covered, outside
            let mut contact = false;
            for cell in &built.cells {
                let other = if cell.source[0] == a {
                    b
                } else if cell.source[0] == b {
                    a
                } else {
                    continue;
                };
                budget.spend(1)?;
                let own = cell.sides.map(|s| built.samples[s].inside[cell.source[0]]);
                let state = cell.sides.map(|s| built.samples[s].inside[other]);
                if own[0] == own[1] {
                    return Err(unsupported("input face does not bound material"));
                }
                let flags = faces.entry(cell.source).or_default();
                flags[0] |= state[0] || state[1];
                flags[1] |= !state[0] && !state[1];
                contact |= state[0] != state[1] && own[0] != state[0];
            }
            let crossing = faces
                .values()
                .any(|[covered, outside]| *covered && *outside);
            if crossing
                || contact
                || (include_edges
                    && edge_contact(&built.operands[a], &built.operands[b], &mut budget)?)
            {
                result.push([a, b]);
            }
        }
    }
    Ok(result)
}

fn edge_contact(
    left: &[Polygon<'_>],
    right: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for a in left {
        for b in right {
            budget.spend(1)?;
            let direction = cross(&a.normal, &b.normal);
            if zero(&direction) {
                if !a.plane_side(&b.ring[0]).is_zero() {
                    continue;
                }
                for i in 0..a.ring.len() {
                    for j in 0..b.ring.len() {
                        if input::segment_contacts(
                            &a.ring[i],
                            &a.ring[(i + 1) % a.ring.len()],
                            &b.ring[j],
                            &b.ring[(j + 1) % b.ring.len()],
                            &a.normal,
                            budget,
                        )?
                        .len()
                            == 2
                        {
                            return Ok(true);
                        }
                    }
                }
            } else {
                let axis = direction.iter().position(|v| !v.is_zero()).unwrap();
                if let (Some([a0, a1]), Some([b0, b1])) = (
                    embedding::plane_section(a, b, axis, budget)?,
                    embedding::plane_section(b, a, axis, budget)?,
                ) {
                    let low = a0[axis].clone().max(b0[axis].clone());
                    let high = a1[axis].clone().min(b1[axis].clone());
                    if low < high {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}
