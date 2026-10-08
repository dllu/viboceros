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
        .map(|g| {
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
    let target_cells = built
        .cells
        .iter()
        .filter(|c| c.source[0] == 0 && c.source_covers_cell)
        .map(|c| (c.polygon.clone(), c.source))
        .collect::<Vec<_>>();
    let covered = groups
        .iter()
        .map(|g| {
            let mut cells = BTreeMap::new();
            for c in &built.cells {
                if g.contains(&c.source[0]) && c.source_covers_cell {
                    cells
                        .entry(canonical_ring(&c.polygon.ring))
                        .or_insert((c.polygon.clone(), c.source));
                }
            }
            cells
        })
        .collect::<Vec<_>>();
    let planes = groups
        .iter()
        .map(|g| built.operands[g[0]][0].clone())
        .collect::<Vec<_>>();
    let mut plan = BrepPolyhedralBooleanPlan::from_arrangement(built, tolerance, budget);
    let inputs = (0..refs.len())
        .map(|i| plan.input(i))
        .collect::<Result<Vec<_>, _>>()?;
    let initial = plan
        .physical_patches(&inputs[0])?
        .into_iter()
        .filter(|(_, source)| source[0] == 0)
        .collect();
    let mut nodes = vec![BoundaryNode {
        region: inputs[0].clone(),
        patches: initial,
    }];
    let mut split = false;
    let mut logical_sources = BTreeSet::from([0usize]);
    let mut patch_budget = Budget(EXACT_WORK_LIMIT);
    for (stage, group) in groups.into_iter().enumerate() {
        let closed = refs[group[0]].is_solid();
        if closed && !plan.sheet_covers_region(&inputs[group[0]], &[0])? {
            continue;
        }
        let mut next = Vec::new();
        for node in nodes {
            if coplanar[stage] {
                let (children, changed) = coplanar_nodes(
                    node,
                    &covered[stage],
                    &target_cells,
                    &planes[stage],
                    &mut patch_budget,
                )?;
                split |= changed;
                next.extend(children);
                continue;
            }
            if !closed {
                if !plan.sheet_covers_boundary_section(&node.region, &group)? {
                    next.push(node);
                    continue;
                }
                let keys = node
                    .patches
                    .iter()
                    .map(|(p, _)| canonical_ring(&p.ring))
                    .collect::<BTreeSet<_>>();
                let missing = plan
                    .physical_patches(&node.region)?
                    .iter()
                    .any(|(p, source)| {
                        logical_sources.contains(&source[0])
                            && !keys.contains(&canonical_ring(&p.ring))
                            && (0..p.ring.len()).any(|i| {
                                planes[stage].plane_side(&p.ring[i]).is_zero()
                                    && planes[stage]
                                        .plane_side(&p.ring[(i + 1) % p.ring.len()])
                                        .is_zero()
                            })
                    });
                if missing {
                    next.push(node);
                    continue;
                }
            }
            let inside = plan.combine(
                BrepBooleanOperation::Intersection,
                &[&node.region, &inputs[group[0]]],
            )?;
            let outside = plan.combine(
                BrepBooleanOperation::Difference,
                &[&node.region, &inputs[group[0]]],
            )?;
            if plan.is_empty(&inside)? || plan.is_empty(&outside)? {
                next.push(node);
                continue;
            }
            let owned = node
                .patches
                .iter()
                .map(|(p, s)| (canonical_ring(&p.ring), (p.clone(), *s)))
                .collect::<BTreeMap<_, _>>();
            split = true;
            for region in [inside, outside] {
                let patches = plan
                    .physical_patches(&region)?
                    .into_iter()
                    .filter_map(|(p, s)| {
                        owned
                            .get(&canonical_ring(&p.ring))
                            .cloned()
                            .or_else(|| group.contains(&s[0]).then_some((p, s)))
                    })
                    .collect();
                next.push(BoundaryNode { region, patches });
            }
        }
        if !coplanar[stage] {
            logical_sources.extend(group);
        }
        nodes = next;
    }
    if !split {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for node in nodes {
        result.extend(plan.export_open_patches(node.patches)?);
    }
    Ok(result)
}

type Patch<'a> = (Polygon<'a>, [usize; 2]);
struct BoundaryNode<'a> {
    region: BrepPolyhedralRegion,
    patches: Vec<Patch<'a>>,
}

fn coplanar_nodes<'a>(
    node: BoundaryNode<'a>,
    covered: &BTreeMap<Vec<ExactPoint>, Patch<'a>>,
    originals: &[Patch<'a>],
    plane: &Polygon<'_>,
    budget: &mut Budget,
) -> Result<(Vec<BoundaryNode<'a>>, bool), GeometryError> {
    let planar = node
        .patches
        .iter()
        .filter(|(p, _)| {
            zero(&cross(&p.normal, &plane.normal)) && plane.plane_side(&p.ring[0]).is_zero()
        })
        .cloned()
        .collect::<Vec<_>>();
    if planar.is_empty() || covered.is_empty() {
        return Ok((vec![node], false));
    }
    let loops = patch_loops(&planar, budget)?;
    let normal = &planar[0].0.normal;
    let outer = loops
        .iter()
        .filter(|ring| loop_area(ring, normal).is_positive())
        .collect::<Vec<_>>();
    let holes = loops
        .iter()
        .filter(|ring| loop_area(ring, normal).is_negative())
        .collect::<Vec<_>>();
    for (p, _) in covered.values() {
        let center = mean(&p.ring, budget)?;
        let mut inside = false;
        for ring in &outer {
            inside |= input::inside_ring(&center, ring, normal, budget)? == Some(true);
        }
        if !inside {
            return Ok((vec![node], false));
        }
    }
    let keys = planar
        .iter()
        .map(|(p, _)| canonical_ring(&p.ring))
        .collect::<BTreeSet<_>>();
    let shared = keys.iter().filter(|k| covered.contains_key(*k)).count();
    if shared == keys.len() {
        return Ok((vec![node], false));
    }
    let mut hole_covered = false;
    for hole in holes {
        let mut all = true;
        let mut any = false;
        let mut found = false;
        for (p, _) in originals {
            let center = mean(&p.ring, budget)?;
            if input::inside_ring(&center, hole, normal, budget)? == Some(true) {
                found = true;
                let contains = covered.contains_key(&canonical_ring(&p.ring));
                all &= contains;
                any |= contains;
            }
        }
        if any && !all {
            return Ok((vec![node], false));
        }
        hole_covered |= found && all;
    }
    if shared == 0 {
        if !hole_covered {
            return Ok((vec![node], false));
        }
        let mut filled = Vec::new();
        for (p, s) in originals {
            let center = mean(&p.ring, budget)?;
            let mut inside = false;
            for ring in &outer {
                inside |= input::inside_ring(&center, ring, normal, budget)? == Some(true);
            }
            if inside {
                filled.push(reassign(p, s, covered));
            }
        }
        let extra = BoundaryNode {
            region: node.region.clone(),
            patches: filled,
        };
        return Ok((vec![node, extra], true));
    }
    let mut primary = Vec::new();
    let mut remainder = Vec::new();
    for (p, s) in &node.patches {
        let key = canonical_ring(&p.ring);
        if keys.contains(&key) {
            primary.push(reassign(p, s, covered));
            if !covered.contains_key(&key) {
                remainder.push((p.clone(), *s));
            }
        } else {
            primary.push((p.clone(), *s));
            if !hole_covered {
                remainder.push((p.clone(), *s));
            }
        }
    }
    let region = node.region;
    Ok((
        vec![
            BoundaryNode {
                region: region.clone(),
                patches: primary,
            },
            BoundaryNode {
                region,
                patches: remainder,
            },
        ],
        true,
    ))
}
fn reassign<'a>(
    polygon: &Polygon<'a>,
    source: &[usize; 2],
    covered: &BTreeMap<Vec<ExactPoint>, Patch<'a>>,
) -> Patch<'a> {
    if let Some((other, source)) = covered.get(&canonical_ring(&polygon.ring)) {
        let mut p = polygon.clone();
        p.source = other.source;
        p.reversed = other.reversed ^ dot(&p.normal, &other.normal).is_negative();
        (p, *source)
    } else {
        (polygon.clone(), *source)
    }
}
fn loop_area(ring: &[ExactPoint], normal: &ExactPoint) -> Rational {
    (0..ring.len())
        .map(|i| dot(normal, &cross(&ring[i], &ring[(i + 1) % ring.len()])))
        .sum()
}
fn patch_loops(
    patches: &[Patch<'_>],
    budget: &mut Budget,
) -> Result<Vec<Vec<ExactPoint>>, GeometryError> {
    let mut polygons = patches.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
    subdivide(&mut polygons, budget)?;
    let mut edges = BTreeMap::<[ExactPoint; 2], Vec<[ExactPoint; 2]>>::new();
    for p in polygons {
        for i in 0..p.ring.len() {
            budget.spend(1)?;
            let a = p.ring[i].clone();
            let b = p.ring[(i + 1) % p.ring.len()].clone();
            let key = if a < b {
                [a.clone(), b.clone()]
            } else {
                [b.clone(), a.clone()]
            };
            edges.entry(key).or_default().push([a, b]);
        }
    }
    let mut next = BTreeMap::new();
    for uses in edges.into_values() {
        if uses.len() == 1 {
            let [a, b] = uses.into_iter().next().unwrap();
            if next.insert(a, b).is_some() {
                return Err(unsupported("ambiguous coplanar patch junction"));
            }
        } else if uses.len() != 2 {
            return Err(unsupported("overlapping coplanar patches"));
        }
    }
    let mut loops = Vec::new();
    while let Some(first) = next.keys().next().cloned() {
        let mut current = first.clone();
        let mut ring = Vec::new();
        loop {
            budget.spend(1)?;
            ring.push(current.clone());
            current = next
                .remove(&current)
                .ok_or_else(|| unsupported("open coplanar patch boundary"))?;
            if current == first {
                break;
            }
        }
        loops.push(ring);
    }
    Ok(loops)
}
