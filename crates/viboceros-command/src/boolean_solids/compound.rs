//! Native compound two-set policy over one exact original-face arrangement.
use super::*;
use viboceros_geometry::{
    BrepBooleanOperation as Op, BrepPolyhedralBooleanPlan as Plan, BrepPolyhedralRegion as Region,
    BrepPolyhedralShell as Shell, BrepSolidOrientation,
};

struct StagedShell {
    shell: Shell,
    allowed_faces: Vec<[usize; 2]>,
    owner: usize,
    geometry_owner: Option<usize>,
}
type UnionShells = Vec<(Shell, Vec<[usize; 2]>)>;

fn root(parents: &[usize], mut i: usize) -> usize {
    while parents[i] != i {
        i = parents[i];
    }
    i
}

/// Prune boundary-enclosed objects, union each set, and retain participating
/// original shells of consumed objects. Independent objects remain separate.
/// Metadata belongs to the connected original-object union, even if one input
/// contains several disconnected material bodies or nested islands.
fn preprocess(
    plan: &mut Plan<'_>,
    breps: &[&Brep],
    inputs: &[Region],
    indices: std::ops::Range<usize>,
    tolerance: Tolerance,
) -> Result<Vec<StagedShell>, GeometryError> {
    let mut retained = Vec::new();
    for i in indices.clone() {
        let mut enclosed = false;
        for j in indices.clone() {
            if i == j {
                continue;
            }
            if plan.boundary_covered_by(&inputs[i], &inputs[j])?
                && !plan.boundary_interacts(&inputs[i], &inputs[j])?
                && (j < i || !plan.boundary_covered_by(&inputs[j], &inputs[i])?)
            {
                enclosed = true;
                break;
            }
        }
        if !enclosed {
            retained.push(i);
        }
    }
    let mut originals = Vec::new();
    for &i in &retained {
        let parts = plan.shells(&inputs[i])?;
        // A reversed single closed shell is normalized. Unmeasured compound
        // orientations must not silently acquire odd/even command semantics.
        let original_faces = breps[i].edge_connected_face_components();
        if original_faces.len() > 1 {
            for shell in &parts {
                let aliases = plan.boundary_faces(&shell.region)?;
                let faces = original_faces
                    .iter()
                    .find(|faces| aliases.iter().any(|s| s[0] == i && faces.contains(&s[1])))
                    .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
                let inward = breps[i]
                    .duplicate_faces(faces, tolerance)?
                    .solid_orientation()?
                    == BrepSolidOrientation::Inward;
                if inward != shell.inward {
                    return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
                        context: "compound orientations inconsistent with material nesting require a native command certificate",
                    });
                }
            }
        }
        originals.push(parts);
    }
    let mut parents = (0..breps.len()).collect::<Vec<_>>();
    let mut interacting = vec![false; breps.len()];
    let mut active = breps
        .iter()
        .map(|b| vec![false; b.faces().len()])
        .collect::<Vec<_>>();
    for a in 0..retained.len() {
        for b in a + 1..retained.len() {
            let (i, j) = (retained[a], retained[b]);
            if !plan.boundary_interacts(&inputs[i], &inputs[j])? {
                continue;
            }
            let first = root(&parents, i);
            let second = root(&parents, j);
            parents[second] = first;
            interacting[i] = true;
            interacting[j] = true;
            for left in &originals[a] {
                for right in &originals[b] {
                    if plan.boundary_interacts(&left.region, &right.region)? {
                        for (owner, shell) in [(i, left), (j, right)] {
                            for [source, face] in plan.boundary_faces(&shell.region)? {
                                if source == owner {
                                    active[source][face] = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let refs = retained.iter().map(|&i| &inputs[i]).collect::<Vec<_>>();
    let region = plan.combine(Op::Union, &refs)?;
    let mut groups = BTreeMap::<usize, UnionShells>::new();
    let shells = if retained.len() == 1 {
        originals.remove(0)
    } else {
        plan.shells(&region)?
    };
    for shell in shells {
        let aliases = plan.boundary_faces(&shell.region)?;
        let sources = aliases
            .iter()
            .map(|s| s[0])
            .filter(|i| retained.contains(i))
            .collect::<BTreeSet<_>>();
        let &first = sources
            .first()
            .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
        let group = root(&parents, first);
        if sources.iter().any(|&i| root(&parents, i) != group) {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        let consumed = retained
            .iter()
            .any(|&i| root(&parents, i) == group && interacting[i]);
        if consumed
            && !aliases
                .iter()
                .any(|s| retained.contains(&s[0]) && active[s[0]][s[1]])
        {
            continue;
        }
        let allowed_faces = aliases
            .into_iter()
            .filter(|s| retained.contains(&s[0]))
            .collect();
        groups
            .entry(group)
            .or_default()
            .push((shell, allowed_faces));
    }
    let mut result = Vec::new();
    for pieces in groups.into_values() {
        let sources = pieces
            .iter()
            .flat_map(|(_, s)| s.iter().map(|s| s[0]))
            .collect::<BTreeSet<_>>();
        let owner = *sources
            .first()
            .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
        let geometry_owner = if pieces.len() == 1 {
            sources.last().copied()
        } else {
            None
        };
        for (shell, allowed_faces) in pieces {
            result.push(StagedShell {
                shell,
                allowed_faces,
                owner,
                geometry_owner,
            });
        }
    }
    result.sort_by_key(|s| s.owner);
    Ok(result)
}

pub(super) fn sets(
    breps: &[&Brep],
    first_count: usize,
    tolerance: Tolerance,
) -> Result<Vec<ShellIntersection>, GeometryError> {
    let mut plan = Plan::try_new(breps, tolerance)?;
    let inputs = (0..breps.len())
        .map(|i| plan.input(i))
        .collect::<Result<Vec<_>, _>>()?;
    let first = preprocess(&mut plan, breps, &inputs, 0..first_count, tolerance)?;
    let second = preprocess(
        &mut plan,
        breps,
        &inputs,
        first_count..breps.len(),
        tolerance,
    )?;
    if first.len().saturating_mul(second.len()) > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let mut interacts = false;
    for a in &first {
        for b in &second {
            interacts |= plan.boundary_interacts(&a.shell.region, &b.shell.region)?;
        }
    }
    if !interacts {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for a in &first {
        for b in &second {
            let (left, right) = (&a.shell.region, &b.shell.region);
            let crossing = plan.boundary_interacts(left, right)?;
            let a_inside = plan.covered_by(left, right)?;
            let b_inside = plan.covered_by(right, left)?;
            let (region, metadata) = match (a.shell.inward, b.shell.inward) {
                (false, false) => {
                    if !crossing && a_inside {
                        (left.clone(), a)
                    } else if !crossing && b_inside {
                        (right.clone(), b)
                    } else {
                        (plan.combine(Op::Intersection, &[left, right])?, a)
                    }
                }
                (true, true) => {
                    let overlap = plan.combine(Op::Intersection, &[left, right])?;
                    if !crossing && plan.is_empty(&overlap)? {
                        continue;
                    }
                    return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
                        context: "interacting compound inward shell pairs require a native command certificate",
                    });
                }
                _ => {
                    if (a_inside || b_inside) && plan.boundaries_overlap(left, right)? {
                        return Err(GeometryError::UnrepresentableBrepBoolean);
                    }
                    if a_inside || b_inside {
                        continue;
                    }
                    let (positive, negative) = if a.shell.inward { (b, a) } else { (a, b) };
                    if crossing {
                        (
                            plan.combine(
                                Op::Difference,
                                &[&positive.shell.region, &negative.shell.region],
                            )?,
                            a,
                        )
                    } else {
                        (positive.shell.region.clone(), positive)
                    }
                }
            };
            let faces = a
                .allowed_faces
                .iter()
                .chain(&b.allowed_faces)
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let outputs = plan.export_with_boundary_faces(&region, &faces)?;
            let geometry_owner = if outputs.len() == 1 {
                metadata.geometry_owner
            } else {
                None
            };
            for component in outputs {
                result.push(ShellIntersection {
                    component,
                    owner: metadata.owner,
                    geometry_owner,
                });
            }
        }
    }
    Ok(result)
}
