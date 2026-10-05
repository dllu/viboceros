//! Plane arrangements and exact two-sided membership for closed polyhedra.
use super::*;

mod embedding;
mod input;
#[cfg(test)]
mod tests;

/// One material component of a polyhedral Boolean, including its cavity shells.
#[derive(Clone, Debug)]
pub struct BrepPolyhedralBooleanComponent {
    pub brep: Brep,
    /// Original operand (0 or 1) and face index for each unmerged output face.
    pub face_sources: Vec<[usize; 2]>,
}

/// Boolean of two closed, embedded polyhedral B-reps.
///
/// Faces may be concave and have holes; operands may have disjoint shells,
/// cavities, and nested islands. Material is defined by odd/even shell
/// containment, independently of input orientation. Surface control nets must
/// be exactly affine bilinear, edges and trims certified clamped straight
/// segments, and model vertices exactly coplanar. UV-to-model correspondence is
/// certified over each whole straight segment within the absolute tolerance.
/// Rounded UV coordinates in earlier Boolean results are therefore accepted.
/// Curves, open shells, self intersections and singular contacts are rejected.
///
/// Exact rational plane arrangements split all boundary patches. Exact ray
/// parity on both sides determines which patches bound the result and their
/// outward orientation. Equivalent coplanar patches belong to the earliest
/// operand/face. Rounding occurs once during validated topology construction.
/// Output components retain their cavities and original supporting surfaces;
/// coplanar fragments are not merged. Inputs are unchanged. Empty mathematical
/// regions return an empty vector, independently of interactive command policy.
/// Input faces, arrangement fragments, work and rational sizes are bounded.
pub fn boolean_polyhedral_breps(
    first: &Brep,
    second: &Brep,
    operation: BrepBooleanOperation,
    tolerance: Tolerance,
) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let operands = [
        input::extract(first, tolerance, &mut budget)?,
        input::extract(second, tolerance, &mut budget)?,
    ];
    let all = operands.iter().flatten().cloned().collect::<Vec<_>>();
    let planes = supporting_planes(&all, &mut budget)?;
    let indices = operands
        .each_ref()
        .map(|p| (0..p.len()).collect::<Vec<_>>());
    let mut output = Vec::new();
    let mut unique = BTreeSet::new();
    for polygon in &all {
        let mut cuts = planes.clone();
        // Coplanar overlap needs subdivision at bounded patch edges, not just
        // at supporting face planes. Include our own coplanar fragments so all
        // owners construct the same minimal arrangement cells.
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
                        &mut budget,
                    )?;
                }
            }
        }
        for ring in arrange(polygon.ring.clone(), &cuts, &mut budget)? {
            let piece = polygon.with_ring(ring);
            let center = mean(&piece.ring, &mut budget)?;
            let [negative, positive] = side_points(&center, &piece.normal, &planes, &mut budget)?;
            let mut values = [false; 2];
            for (side, p) in [negative, positive].iter().enumerate() {
                let a = union::contains(p, &indices[0], &operands[0], &mut budget)?;
                let b = union::contains(p, &indices[1], &operands[1], &mut budget)?;
                values[side] = match operation {
                    BrepBooleanOperation::Union => a || b,
                    BrepBooleanOperation::Intersection => a && b,
                    BrepBooleanOperation::Difference => a && !b,
                };
            }
            if values[0] == values[1] || !unique.insert(canonical_ring(&piece.ring)) {
                continue;
            }
            output.push(if values[0] { piece } else { piece.reverse() });
            if output.len() > MAX_OUTPUT_FACES {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
        }
    }
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let sources = source_faces(&[first, second], &output, &mut budget)?;
    union::material_components(&output, tolerance, &mut budget)?
        .into_iter()
        .map(|body| {
            embedding::certify_vertex_links(&body.brep, &mut budget).map_err(
                |error| match error {
                    GeometryError::UnsupportedPolyhedralBrepBoolean { .. } => {
                        GeometryError::UnrepresentableBrepBoolean
                    }
                    other => other,
                },
            )?;
            Ok(BrepPolyhedralBooleanComponent {
                brep: body.brep,
                face_sources: body.faces.into_iter().map(|i| sources[i]).collect(),
            })
        })
        .collect()
}

impl Brep {
    /// Polyhedral set operation including concave faces, holes and multiple
    /// shells. See [`boolean_polyhedral_breps`] for certificates and limits.
    pub fn try_boolean_polyhedral(
        &self,
        other: &Self,
        operation: BrepBooleanOperation,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let mut bodies = boolean_polyhedral_breps(self, other, operation, tolerance)?;
        match bodies.len() {
            0 => Ok(None),
            1 => Ok(Some(bodies.remove(0).brep)),
            _ => Self::try_disjoint_union(bodies.into_iter().map(|b| b.brep).collect(), tolerance)
                .map(Some),
        }
    }
}

fn unsupported(context: &'static str) -> GeometryError {
    GeometryError::UnsupportedPolyhedralBrepBoolean { context }
}

#[derive(Clone)]
struct Plane {
    anchor: ExactPoint,
    normal: ExactPoint,
}

fn add_plane(
    planes: &mut Vec<Plane>,
    plane: Plane,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    check_point(&plane.normal)?;
    if zero(&plane.normal) {
        return Err(unsupported("degenerate supporting line or plane"));
    }
    for existing in planes.iter() {
        budget.spend(1)?;
        if zero(&cross(&plane.normal, &existing.normal))
            && dot(&existing.normal, &sub(&plane.anchor, &existing.anchor)).is_zero()
        {
            return Ok(());
        }
    }
    planes.push(plane);
    Ok(())
}

fn supporting_planes(
    polygons: &[Polygon<'_>],
    budget: &mut Budget,
) -> Result<Vec<Plane>, GeometryError> {
    let mut planes = Vec::new();
    for p in polygons {
        add_plane(
            &mut planes,
            Plane {
                anchor: p.ring[0].clone(),
                normal: p.normal.clone(),
            },
            budget,
        )?;
    }
    Ok(planes)
}

fn arrange(
    ring: Vec<ExactPoint>,
    planes: &[Plane],
    budget: &mut Budget,
) -> Result<Vec<Vec<ExactPoint>>, GeometryError> {
    let mut cells = vec![ring];
    for plane in planes {
        let mut next = Vec::new();
        for cell in cells {
            let (a, b) = split_plane(&cell, &plane.anchor, &plane.normal, budget)?;
            next.extend(a);
            next.extend(b);
            if next.len() > MAX_OUTPUT_FACES {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
        }
        cells = next;
    }
    Ok(cells)
}

fn mean(points: &[ExactPoint], budget: &mut Budget) -> Result<ExactPoint, GeometryError> {
    budget.spend(points.len())?;
    let mut sum: ExactPoint = std::array::from_fn(|_| Rational::zero());
    for p in points {
        for i in 0..3 {
            sum[i] += &p[i];
        }
        check_point(&sum)?;
    }
    for value in &mut sum {
        *value /= Rational::from_integer(points.len().into());
    }
    check_point(&sum)?;
    Ok(sum)
}

/// Stay strictly within the current arrangement cell on either side of its
/// supporting plane. This is an exact distance bound, not a numerical jitter.
fn side_points(
    center: &ExactPoint,
    normal: &ExactPoint,
    planes: &[Plane],
    budget: &mut Budget,
) -> Result<[ExactPoint; 2], GeometryError> {
    let mut step = rational(1.);
    for plane in planes {
        budget.spend(1)?;
        let distance = dot(&plane.normal, &sub(center, &plane.anchor)).abs();
        let speed = dot(&plane.normal, normal).abs();
        if !distance.is_zero() && !speed.is_zero() {
            let bound = distance / (rational(2.) * speed);
            check_scalar(&bound)?;
            step = step.min(bound);
        }
    }
    let result = [
        std::array::from_fn(|i| &center[i] - &step * &normal[i]),
        std::array::from_fn(|i| &center[i] + &step * &normal[i]),
    ];
    for p in &result {
        check_point(p)?;
    }
    Ok(result)
}

fn canonical_ring(ring: &[ExactPoint]) -> Vec<ExactPoint> {
    let first = (0..ring.len()).min_by_key(|&i| &ring[i]).unwrap();
    let forward = (0..ring.len())
        .map(|i| ring[(first + i) % ring.len()].clone())
        .collect::<Vec<_>>();
    let backward = (0..ring.len())
        .map(|i| ring[(first + ring.len() - i) % ring.len()].clone())
        .collect::<Vec<_>>();
    forward.min(backward)
}
