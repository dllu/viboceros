//! Exact cells and membership states shared by all polyhedral set operations.
use super::*;

pub(super) struct Sample {
    pub(super) point: ExactPoint,
    pub(super) inside: Vec<bool>,
}

pub(super) struct Cell<'a> {
    pub(super) polygon: Polygon<'a>,
    pub(super) source: [usize; 2],
    pub(super) sides: [usize; 2],
}

pub(super) struct Arrangement<'a> {
    pub(super) operands: Vec<Vec<Polygon<'a>>>,
    pub(super) cells: Vec<Cell<'a>>,
    pub(super) samples: Vec<Sample>,
}

#[derive(Clone, Copy)]
pub(super) enum Expression<'a> {
    Union,
    Common,
    Sets(usize),
    Difference,
    Region(&'a [bool]),
}

impl Expression<'_> {
    pub(super) fn includes(self, inside: &[bool]) -> bool {
        match self {
            Self::Union => inside.iter().any(|v| *v),
            Self::Common => !inside.is_empty() && inside.iter().all(|v| *v),
            Self::Sets(first) => {
                inside[..first].iter().any(|v| *v) && inside[first..].iter().any(|v| *v)
            }
            Self::Difference => inside[0] && !inside[1..].iter().any(|v| *v),
            Self::Region(membership) => inside[0] && inside[1..].starts_with(membership),
        }
    }
}

pub(super) fn build<'a>(
    breps: &[&'a Brep],
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Arrangement<'a>, GeometryError> {
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let operands = breps
        .iter()
        .map(|b| input::extract(b, tolerance, budget))
        .collect::<Result<Vec<_>, _>>()?;
    let all = operands.iter().flatten().cloned().collect::<Vec<_>>();
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
    for (owner, polygons) in operands.iter().enumerate() {
        let face_ids = breps[owner]
            .faces
            .iter()
            .enumerate()
            .map(|(i, f)| (std::ptr::from_ref(f), i))
            .collect::<BTreeMap<_, _>>();
        for polygon in polygons {
            let mut cuts = planes.clone();
            // Include all coplanar fragment edges, including our own, so each
            // owner constructs exactly the same cells on a shared plane.
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
                let piece = polygon.with_ring(ring);
                let center = mean(&piece.ring, budget)?;
                let points = side_points(&center, &piece.normal, &planes, budget)?;
                let sides = [samples.len(), samples.len() + 1];
                for point in points {
                    let inside = operands
                        .iter()
                        .zip(&indices)
                        .map(|(p, i)| union::contains(&point, i, p, budget))
                        .collect::<Result<Vec<_>, _>>()?;
                    samples.push(Sample { point, inside });
                }
                cells.push(Cell {
                    polygon: piece,
                    source: [owner, face_ids[&std::ptr::from_ref(polygon.source)]],
                    sides,
                });
            }
        }
    }
    Ok(Arrangement {
        operands,
        cells,
        samples,
    })
}

pub(super) struct Assembled {
    pub(super) bodies: Vec<union::MaterialBody>,
    pub(super) face_sources: Vec<[usize; 2]>,
    pub(super) sample_bodies: Vec<Option<usize>>,
}

pub(super) fn assemble(
    arrangement: &Arrangement<'_>,
    expression: Expression<'_>,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Assembled, GeometryError> {
    let mut output = Vec::new();
    let mut face_sources = Vec::new();
    let mut unique = BTreeSet::new();
    for cell in &arrangement.cells {
        budget.spend(1)?;
        let included = cell
            .sides
            .map(|i| expression.includes(&arrangement.samples[i].inside));
        if included[0] == included[1] || !unique.insert(canonical_ring(&cell.polygon.ring)) {
            continue;
        }
        output.push(if included[0] {
            cell.polygon.clone()
        } else {
            cell.polygon.clone().reverse()
        });
        face_sources.push(cell.source);
    }
    let bodies = if output.is_empty() {
        Vec::new()
    } else {
        union::material_components(&output, tolerance, budget)?
    };
    for body in &bodies {
        embedding::certify_vertex_links(&body.brep, budget).map_err(|error| match error {
            GeometryError::UnsupportedPolyhedralBrepBoolean { .. } => {
                GeometryError::UnrepresentableBrepBoolean
            }
            other => other,
        })?;
    }
    let mut sample_bodies = Vec::with_capacity(arrangement.samples.len());
    for sample in &arrangement.samples {
        if !expression.includes(&sample.inside) {
            sample_bodies.push(None);
            continue;
        }
        let mut owner = None;
        for (index, body) in bodies.iter().enumerate() {
            if union::contains(&sample.point, &body.faces, &output, budget)?
                && owner.replace(index).is_some()
            {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
        sample_bodies.push(Some(
            owner.ok_or(GeometryError::UnrepresentableBrepBoolean)?,
        ));
    }
    Ok(Assembled {
        bodies,
        face_sources,
        sample_bodies,
    })
}
