//! Command shell and metadata policies; every geometric operation uses SMLib.
use super::*;

struct Shell {
    solid: Solid,
    inward: bool,
    owner: usize,
    geometry_owner: Option<usize>,
}

fn split_boundaries(
    brep: &Brep,
    solid: &Solid,
    owner: usize,
    tolerance: Tolerance,
) -> Result<Vec<Shell>, CommandError> {
    shells(brep, solid, tolerance)?
        .into_iter()
        .map(|(faces, solid)| {
            let inward = brep.sub_brep(&faces, tolerance)?.signed_volume(tolerance)? < 0.;
            Ok(Shell {
                solid,
                inward,
                owner,
                geometry_owner: None,
            })
        })
        .collect()
}

fn contact(a: &Solid, b: &Solid, tolerance: Tolerance) -> Result<bool, CommandError> {
    if covered_by(a, b)? && covered_by(b, a)? {
        return Ok(false);
    }
    Ok(a.boundary_contact(b, tolerance)?)
}

fn preprocess(
    breps: &[&Brep],
    inputs: &[Solid],
    indices: std::ops::Range<usize>,
    tolerance: Tolerance,
) -> Result<Vec<Shell>, CommandError> {
    let mut retained = Vec::new();
    for i in indices.clone() {
        let mut enclosed = false;
        for j in indices.clone() {
            if i != j
                && inputs[i].boundary_is_covered_by(&inputs[j])?
                && !contact(&inputs[i], &inputs[j], tolerance)?
                && (j < i || !inputs[j].boundary_is_covered_by(&inputs[i])?)
            {
                enclosed = true;
                break;
            }
        }
        if !enclosed {
            retained.push(i);
        }
    }
    let mut parents = (0..inputs.len()).collect::<Vec<_>>();
    for (a, &i) in retained.iter().enumerate() {
        for &j in &retained[a + 1..] {
            if contact(&inputs[i], &inputs[j], tolerance)? {
                let first = root(&parents, i);
                let second = root(&parents, j);
                parents[second] = first;
            }
        }
    }
    let mut groups = BTreeMap::<usize, Vec<usize>>::new();
    for &i in &retained {
        groups.entry(root(&parents, i)).or_default().push(i);
    }
    let mut result = Vec::new();
    for group in groups.into_values() {
        let mut boundaries;
        let owner;
        let last;
        if group.len() == 1 {
            owner = group[0];
            last = owner;
            boundaries = split_boundaries(breps[owner], &inputs[owner], owner, tolerance)?;
        } else {
            let mut union = inputs[group[0]].try_clone()?;
            for &i in &group[1..] {
                union = union.boolean(&inputs[i], BooleanOperation::Union)?;
            }
            let contributors = group
                .iter()
                .copied()
                .filter_map(|i| match inputs[i].boundary_contact(&union, tolerance) {
                    Ok(true) => Some(Ok(i)),
                    Ok(false) => None,
                    Err(e) => Some(Err(CommandError::from(e))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            owner = *contributors
                .first()
                .ok_or_else(|| unsupported("Ambiguous set-union boundary owner"))?;
            last = *contributors.last().unwrap();
            let mut active = Vec::new();
            for &i in &group {
                for (_, shell) in shells(breps[i], &inputs[i], tolerance)? {
                    for &j in &group {
                        if i != j && contact(&shell, &inputs[j], tolerance)? {
                            active.push(shell);
                            break;
                        }
                    }
                }
            }
            let shape = union.to_brep(tolerance)?;
            boundaries = split_boundaries(&shape, &union, owner, tolerance)?;
            let mut kept = Vec::new();
            for boundary in boundaries {
                for original in &active {
                    if boundary.solid.boundary_contact(original, tolerance)? {
                        kept.push(boundary);
                        break;
                    }
                }
            }
            boundaries = kept;
        }
        let geometry_owner = (boundaries.len() == 1).then_some(last);
        for mut boundary in boundaries {
            boundary.owner = owner;
            boundary.geometry_owner = geometry_owner;
            result.push(boundary);
        }
    }
    result.sort_by_key(|s| s.owner);
    Ok(result)
}

pub(super) fn sets(
    breps: &[&Brep],
    inputs: &[Solid],
    objects: &[&viboceros_document::Object],
    first: usize,
    tolerance: Tolerance,
) -> Result<Vec<Output>, CommandError> {
    let left = preprocess(breps, inputs, 0..first, tolerance)?;
    let right = preprocess(breps, inputs, first..inputs.len(), tolerance)?;
    if left.len().saturating_mul(right.len()) > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit.into());
    }
    let mut intersects = false;
    for a in &left {
        for b in &right {
            intersects |= a.solid.boundary_contact(&b.solid, tolerance)?;
        }
    }
    if !intersects {
        return Err(CommandError::NothingIntersected);
    }
    let mut outputs = Vec::new();
    for a in &left {
        for b in &right {
            let crossing = a.solid.boundary_contact(&b.solid, tolerance)?;
            let a_inside = covered_by(&a.solid, &b.solid)?;
            let b_inside = covered_by(&b.solid, &a.solid)?;
            let (result, metadata) = match (a.inward, b.inward) {
                (false, false) => {
                    if a_inside && b_inside {
                        return Err(unsupported("Equal oriented intersection shells"));
                    }
                    if !crossing && a_inside {
                        (a.solid.try_clone()?, a)
                    } else if !crossing && b_inside {
                        (b.solid.try_clone()?, b)
                    } else {
                        (
                            a.solid.boolean(&b.solid, BooleanOperation::Intersection)?,
                            a,
                        )
                    }
                }
                (true, true) => {
                    if a_inside && b_inside {
                        return Err(unsupported("Equal inward intersection shells"));
                    }
                    if a_inside {
                        (b.solid.try_clone()?, b)
                    } else if b_inside {
                        (a.solid.try_clone()?, a)
                    } else if crossing {
                        (a.solid.boolean(&b.solid, BooleanOperation::Union)?, a)
                    } else {
                        continue;
                    }
                }
                _ => {
                    if a_inside || b_inside {
                        if crossing {
                            return Err(unsupported("Coincident nested oriented shell boundaries"));
                        }
                        continue;
                    }
                    let (positive, negative) = if a.inward { (b, a) } else { (a, b) };
                    if crossing {
                        (
                            positive
                                .solid
                                .boolean(&negative.solid, BooleanOperation::Difference)?,
                            a,
                        )
                    } else {
                        (positive.solid.try_clone()?, positive)
                    }
                }
            };
            if result.is_empty()? {
                continue;
            }
            let shape = result.to_brep(tolerance)?;
            let mut pieces = Vec::new();
            for component in shape.edge_connected_face_components() {
                let mut piece = shape.sub_brep(&component, tolerance)?;
                if piece.is_solid() {
                    if piece.signed_volume(tolerance)? < 0. {
                        piece.reverse_orientation();
                    }
                } else if a.inward && b.inward {
                    piece.reverse_orientation();
                }
                pieces.push(piece);
            }
            let text = if pieces.len() == 1 {
                metadata
                    .geometry_owner
                    .map(|i| objects[i].geometry_user_text().clone())
                    .unwrap_or_default()
            } else {
                BTreeMap::new()
            };
            for piece in pieces {
                outputs.push((
                    objects[metadata.owner].id(),
                    Geometry::Brep(piece),
                    text.clone(),
                ));
            }
        }
    }
    if outputs.is_empty() {
        return Err(CommandError::NothingIntersected);
    }
    Ok(outputs)
}
