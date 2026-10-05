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

fn original_shells(
    plan: &mut Plan<'_>,
    breps: &[&Brep],
    inputs: &[Region],
    indices: &[usize],
    tolerance: Tolerance,
) -> Result<Vec<Vec<Shell>>, GeometryError> {
    let mut originals = Vec::new();
    for &i in indices {
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
    Ok(originals)
}

fn contact(plan: &mut Plan<'_>, a: &Region, b: &Region) -> Result<bool, GeometryError> {
    if plan.covered_by(a, b)? && plan.covered_by(b, a)? {
        return Ok(false);
    }
    Ok(plan.boundary_interacts(a, b)? || plan.boundaries_share_line(a, b)?)
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
    let mut originals = original_shells(plan, breps, inputs, &retained, tolerance)?;
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
            if a.shell.inward && b.shell.inward {
                interacts |= plan.boundaries_share_line(&a.shell.region, &b.shell.region)?;
            }
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
                    if a_inside && b_inside {
                        return Err(GeometryError::UnrepresentableBrepBoolean);
                    }
                    if !crossing && a_inside {
                        (left.clone(), a)
                    } else if !crossing && b_inside {
                        (right.clone(), b)
                    } else {
                        (plan.combine(Op::Intersection, &[left, right])?, a)
                    }
                }
                (true, true) => {
                    if a_inside && b_inside {
                        return Err(GeometryError::UnrepresentableBrepBoolean);
                    }
                    if a_inside {
                        (right.clone(), b)
                    } else if b_inside {
                        (left.clone(), a)
                    } else if crossing || plan.boundaries_share_line(left, right)? {
                        (plan.combine(Op::Union, &[left, right])?, a)
                    } else {
                        continue;
                    }
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
            let outputs = plan.export_boundary_with_faces(&region, &faces)?;
            let geometry_owner = if outputs.len() == 1 {
                metadata.geometry_owner
            } else {
                None
            };
            for output in outputs {
                // Native inward/inward intersection leaves non-solid SDK
                // boundaries inward; only solid outputs are turned outward.
                let brep = if a.shell.inward && b.shell.inward && !output.brep.is_solid() {
                    output.brep.reversed()
                } else {
                    output.brep
                };
                let component = BrepPolyhedralBooleanComponent {
                    brep,
                    face_sources: output.face_sources,
                };
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

/// Common intersection constructs material once, exports participating
/// connected boundaries, and turns inward solid cavities outward. Enclosing
/// inactive inputs do not supply result metadata. Nonmanifold boundaries retain
/// their winding and have no declared solid orientation.
pub(super) fn common(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<ShellIntersection>, GeometryError> {
    let mut plan = Plan::try_new(breps, tolerance)?;
    let inputs = (0..breps.len())
        .map(|i| plan.input(i))
        .collect::<Result<Vec<_>, _>>()?;
    let indices = (0..breps.len()).collect::<Vec<_>>();
    let originals = original_shells(&mut plan, breps, &inputs, &indices, tolerance)?;
    let mut active = breps
        .iter()
        .map(|b| vec![false; b.faces().len()])
        .collect::<Vec<_>>();
    let mut owners = BTreeSet::new();
    for a in 0..breps.len() {
        for b in a + 1..breps.len() {
            for left in &originals[a] {
                for right in &originals[b] {
                    if contact(&mut plan, &left.region, &right.region)? {
                        owners.extend([a, b]);
                        for (owner, shell) in [(a, left), (b, right)] {
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
    let Some(&first) = owners.first() else {
        return Ok(Vec::new());
    };
    let region = plan.combine(Op::Intersection, &inputs.iter().collect::<Vec<_>>())?;
    let mut outputs = plan.export_boundary(&region)?;
    outputs.retain(|o| o.boundary_faces.iter().any(|s| active[s[0]][s[1]]));
    let geometry_owner = if outputs.len() == 1 {
        owners.last().copied()
    } else {
        None
    };
    outputs
        .into_iter()
        .map(|output| {
            let owner = output
                .boundary_equal_inputs
                .into_iter()
                .find(|i| owners.contains(i))
                .unwrap_or(first);
            let brep = if output.brep.solid_orientation()? == BrepSolidOrientation::Inward {
                output.brep.reversed()
            } else {
                output.brep
            };
            let face_sources = output.face_sources;
            Ok(ShellIntersection {
                component: BrepPolyhedralBooleanComponent { brep, face_sources },
                owner,
                geometry_owner,
            })
        })
        .collect()
}
