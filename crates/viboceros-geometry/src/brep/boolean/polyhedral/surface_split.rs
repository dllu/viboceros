//! Finite coplanar sheets and solid cutters over one exact original arrangement.
use super::*;
use arrangement::{Arrangement, Cell, Sample};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct BrepSurfaceSplitComponent {
    pub brep: Brep,
    pub face_sources: Vec<[usize; 2]>,
    /// One entry per input cutter; None means it did not separate this lineage.
    pub cut_sides: Vec<Option<bool>>,
    /// Connected children on the selected side at each ordered cutting stage.
    /// Coplanar sheets form one stage with their combined finite coverage.
    pub branch_component_counts: Vec<usize>,
}

/// Partition a closed certified polyhedron by closed solids and finite planar
/// sheets. A sheet must cover the entire section of the current connected piece.
/// Coplanar sheets supply joint coverage. Stages follow first input occurrence;
/// incomplete sheets do not silently become infinite planes. All region and
/// coverage queries use exact original cells; only final exports round geometry.
pub fn split_polyhedral_brep_with_surfaces(
    target: &Brep,
    cutters: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepSurfaceSplitComponent>, GeometryError> {
    if !target.is_solid() {
        return Err(unsupported("closed polyhedral target required"));
    }
    let refs = std::iter::once(target)
        .chain(cutters.iter().copied())
        .collect::<Vec<_>>();
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let (built, groups) = build(&refs, tolerance, &mut budget)?;
    let mut plan = BrepPolyhedralBooleanPlan::from_arrangement(built, tolerance, budget);
    let inputs = (0..refs.len())
        .map(|i| plan.input(i))
        .collect::<Result<Vec<_>, _>>()?;
    let mut regions = vec![(
        inputs[0].clone(),
        vec![None; cutters.len()],
        Vec::<usize>::new(),
    )];
    for group in groups {
        let sheet = !refs[group[0]].is_solid();
        let mut next = Vec::new();
        for (region, sides, path) in regions {
            if sheet && !plan.sheet_covers_region(&region, &group)? {
                let mut path = path;
                path.push(1);
                next.push((region, sides, path));
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
                let mut path = path;
                path.push(1);
                next.push((region, sides, path));
                continue;
            }
            for child in [outside, inside] {
                let children = plan.components(&child)?;
                let count = children.len();
                for child in children {
                    let mut sides = sides.clone();
                    for &i in &group {
                        sides[i - 1] = Some(plan.covered_by(&child, &inputs[i])?);
                    }
                    let mut path = path.clone();
                    path.push(count);
                    next.push((child, sides, path));
                }
            }
        }
        regions = next;
    }
    let mut result = Vec::new();
    for (region, cut_sides, branch_component_counts) in regions {
        for body in plan.export(&region)? {
            result.push(BrepSurfaceSplitComponent {
                brep: body.brep,
                face_sources: body.face_sources,
                cut_sides: cut_sides.clone(),
                branch_component_counts: branch_component_counts.clone(),
            });
        }
    }
    Ok(result)
}

pub(super) fn build<'a>(
    breps: &[&'a Brep],
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<(Arrangement<'a>, Vec<Vec<usize>>), GeometryError> {
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let mut operands = Vec::new();
    let mut sheets = Vec::new();
    for brep in breps {
        if brep.is_solid() {
            operands.push(input::extract(brep, tolerance, budget)?);
            sheets.push(false);
        } else {
            let polygons = input::extract_faces(brep, tolerance, budget)?;
            let first = polygons
                .first()
                .ok_or_else(|| unsupported("empty cutting sheet"))?;
            if polygons.iter().any(|p| {
                !zero(&cross(&p.normal, &first.normal)) || !first.plane_side(&p.ring[0]).is_zero()
            }) {
                return Err(unsupported("planar cutting sheets required"));
            }
            operands.push(polygons);
            sheets.push(true);
        }
    }
    let mut groups = Vec::<Vec<usize>>::new();
    for i in 1..breps.len() {
        if sheets[i]
            && let Some(group) = groups.iter_mut().find(|g| {
                sheets[g[0]]
                    && zero(&cross(&operands[g[0]][0].normal, &operands[i][0].normal))
                    && operands[g[0]][0]
                        .plane_side(&operands[i][0].ring[0])
                        .is_zero()
            })
        {
            group.push(i);
            continue;
        }
        groups.push(vec![i]);
    }
    let mut planning = operands.clone();
    let extent = if sheets[0] {
        operands.iter().flatten().cloned().collect::<Vec<_>>()
    } else {
        operands[0].clone()
    };
    for i in 0..breps.len() {
        if sheets[i] {
            planning[i] = vec![seed(&operands[i][0], &extent, budget)?];
        }
    }
    let all = planning
        .iter()
        .flatten()
        .cloned()
        .chain(
            operands
                .iter()
                .zip(&sheets)
                .filter(|(_, sheet)| **sheet)
                .flat_map(|(polygons, _)| polygons.iter().cloned()),
        )
        .collect::<Vec<_>>();
    if all.len() > MAX_OUTPUT_FACES {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let planes = supporting_planes(&all, budget)?;
    let indices = operands
        .iter()
        .map(|p| (0..p.len()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut cells = Vec::new();
    let mut samples = Vec::new();
    for (owner, polygons) in planning.iter().enumerate() {
        let face_ids = breps[owner]
            .faces
            .iter()
            .enumerate()
            .map(|(i, f)| (std::ptr::from_ref(f), i))
            .collect::<BTreeMap<_, _>>();
        for polygon in polygons {
            let mut cuts = planes.clone();
            for other in &all {
                budget.spend(1)?;
                if zero(&cross(&polygon.normal, &other.normal))
                    && other.plane_side(&polygon.ring[0]).is_zero()
                {
                    for i in 0..other.ring.len() {
                        add_plane(
                            &mut cuts,
                            Plane {
                                anchor: other.ring[i].clone(),
                                normal: cross(
                                    &polygon.normal,
                                    &sub(&other.ring[(i + 1) % other.ring.len()], &other.ring[i]),
                                ),
                            },
                            budget,
                        )?;
                    }
                }
            }
            for ring in arrange(polygon.ring.clone(), &cuts, budget)? {
                if cells.len() == MAX_OUTPUT_FACES {
                    return Err(GeometryError::BrepBooleanWorkLimit);
                }
                let mut piece = polygon.with_ring(ring);
                let center = mean(&piece.ring, budget)?;
                let covered = if sheets[owner] {
                    let mut cover = None;
                    for candidate in &operands[owner] {
                        if in_polygon(&center, candidate, budget)? {
                            cover = Some(candidate);
                            break;
                        }
                    }
                    if let Some(cover) = cover {
                        piece.source = cover.source;
                        piece.reversed =
                            cover.reversed ^ dot(&piece.normal, &cover.normal).is_negative();
                    }
                    cover.is_some()
                } else {
                    true
                };
                let points = side_points(&center, &piece.normal, &planes, budget)?;
                let sides = [samples.len(), samples.len() + 1];
                for point in points {
                    let mut inside = Vec::new();
                    for i in 0..operands.len() {
                        budget.spend(1)?;
                        inside.push(if sheets[i] {
                            operands[i][0].plane_side(&point).is_negative()
                        } else {
                            union::contains(&point, &indices[i], &operands[i], budget)?
                        });
                    }
                    samples.push(Sample { point, inside });
                }
                let face = face_ids[&std::ptr::from_ref(piece.source)];
                cells.push(Cell {
                    polygon: piece,
                    source: [owner, face],
                    sides,
                    source_covers_cell: covered,
                });
            }
        }
    }
    Ok((
        Arrangement {
            operands,
            cells,
            samples,
        },
        groups,
    ))
}

fn in_polygon(
    point: &ExactPoint,
    polygon: &Polygon<'_>,
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    for i in 0..polygon.ring.len() {
        budget.spend(1)?;
        let a = &polygon.ring[i];
        let b = &polygon.ring[(i + 1) % polygon.ring.len()];
        if dot(&polygon.normal, &cross(&sub(b, a), &sub(point, a))).is_negative() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn seed<'a>(
    sheet: &Polygon<'a>,
    target: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<Polygon<'a>, GeometryError> {
    let controls = sheet.source.surface.control_points();
    let origin = point(controls[0].point());
    let u = sub(&point(controls[1].point()), &origin);
    let v = sub(&point(controls[2].point()), &origin);
    let normal = cross(&u, &v);
    let denominator = dot(&normal, &normal);
    let mut bounds: [Option<[Rational; 2]>; 2] = [None, None];
    for p in target.iter().flat_map(|p| &p.ring) {
        budget.spend(1)?;
        let delta = sub(p, &origin);
        let uv = [
            dot(&cross(&delta, &v), &normal) / &denominator,
            dot(&cross(&u, &delta), &normal) / &denominator,
        ];
        for i in 0..2 {
            check_scalar(&uv[i])?;
            if let Some(b) = &mut bounds[i] {
                b[0] = b[0].clone().min(uv[i].clone());
                b[1] = b[1].clone().max(uv[i].clone());
            } else {
                bounds[i] = Some([uv[i].clone(), uv[i].clone()]);
            }
        }
    }
    let bounds = bounds.map(|b| b.unwrap());
    let lo = bounds.clone().map(|b| b[0].clone() - rational(1.));
    let hi = bounds.map(|b| b[1].clone() + rational(1.));
    let mut ring = Vec::new();
    for [s, t] in [
        [&lo[0], &lo[1]],
        [&hi[0], &lo[1]],
        [&hi[0], &hi[1]],
        [&lo[0], &hi[1]],
    ] {
        let p: ExactPoint = std::array::from_fn(|i| &origin[i] + s * &u[i] + t * &v[i]);
        check_point(&p)?;
        ring.push(p);
    }
    if sheet.reversed {
        ring.reverse();
    }
    Ok(sheet.with_ring(ring))
}
