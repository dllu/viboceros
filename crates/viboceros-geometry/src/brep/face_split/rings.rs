use super::*;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Node {
    vertex: usize,
    uv: [u64; 2],
}
impl Node {
    fn new(vertex: usize, p: Point2) -> Self {
        Self {
            vertex,
            uv: p.to_array().map(canonical_brep_coordinate_bits),
        }
    }
    fn point(self) -> Point2 {
        Point2::try_new(Real::from_bits(self.uv[0]), Real::from_bits(self.uv[1])).unwrap()
    }
}

struct Arc {
    trim: BrepTrim,
    kind: BrepLoopType,
    connector: bool,
}
impl Arc {
    fn nodes(&self) -> Result<[Node; 2], GeometryError> {
        Ok([
            Node::new(self.trim.vertices[0], self.trim.curve.start_point()?),
            Node::new(self.trim.vertices[1], self.trim.curve.end_point()?),
        ])
    }
}

/// The positive rational basis and its entire control hull authorize the side.
/// Searches/samples never decide whether a curve was missed by the splitter.
pub(super) fn side(
    curve: &NurbsCurve2,
    axis: usize,
    cut: Real,
) -> Result<Option<usize>, GeometryError> {
    let sign = curve.control_points()[0].weight().is_sign_negative();
    let mut less = false;
    let mut greater = false;
    for cp in curve.control_points() {
        if cp.weight().is_sign_negative() != sign {
            return invalid("face partition needs a sign-coherent UV hull");
        }
        let x = parameter_coordinate(cp.point(), axis);
        less |= x < cut;
        greater |= x > cut;
    }
    Ok(match (less, greater) {
        (true, true) => None,
        (true, false) => Some(0),
        (false, true) => Some(1),
        (false, false) => {
            let Some([a, b]) = certificate::linear_endpoints(&curves::lift(curve)?) else {
                return invalid("face partition has an ambiguous trim on its cut");
            };
            let increasing = a.to_array()[1 - axis] < b.to_array()[1 - axis];
            // Oriented trim loops keep the face on their left, including holes.
            Some(usize::from(increasing == (axis == 1)))
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn partition(
    result: &mut Brep,
    face_index: usize,
    axis: usize,
    cut: Real,
    patches: [NurbsSurface; 2],
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Option<Vec<BrepFace>>, GeometryError> {
    let source = &result.faces[face_index];
    let reversed = source.reversed;
    let mut sides: [Vec<Arc>; 2] = std::array::from_fn(|_| Vec::new());
    for ring in &source.loops {
        for trim in &ring.trims {
            budget.charge(trim.curve.control_points().len())?;
            let Some(which) = side(&trim.curve, axis, cut)? else {
                return invalid("face partition did not isolate every UV crossing");
            };
            sides[which].push(Arc {
                trim: trim.clone(),
                kind: ring.loop_type,
                connector: false,
            });
        }
    }
    if sides.iter().any(Vec::is_empty) {
        return Ok(None);
    }
    let ends = [
        open_ends(&sides[0], axis, cut)?,
        open_ends(&sides[1], axis, cut)?,
    ];
    if ends[0].len() != ends[1].len()
        || ends[0]
            .iter()
            .zip(&ends[1])
            .any(|(a, b)| a.0 != b.0 || a.1 == b.1)
    {
        return invalid("face partition cut incidences disagree");
    }
    if ends[0].len() % 2 != 0 {
        return invalid("face partition has an odd crossing count");
    }
    let Some(image) =
        certificate::SurfaceCurve::new(&source.surface, 1 - axis, cut, false, &mut |n| {
            budget.charge(n)
        })?
    else {
        return invalid("face partition cannot certify its new isocurve");
    };
    let isocurve = if axis == 0 {
        source.surface.isocurve_v(cut)?
    } else {
        source.surface.isocurve_u(cut)?
    };
    for pair in ends[0].chunks_exact(2) {
        let [(a, a_out), (b, b_out)] = pair else {
            unreachable!()
        };
        let interval = [
            parameter_coordinate(a.point(), 1 - axis),
            parameter_coordinate(b.point(), 1 - axis),
        ];
        if interval[0] >= interval[1] || *a_out != (axis == 1) || *b_out != (axis == 0) {
            return invalid("face partition has touching or ambiguous cut intervals");
        }
        budget.charge(isocurve.control_points().len().saturating_mul(4))?;
        let curve = isocurve.try_trimmed(interval[0]..=interval[1])?;
        let Some(bound) = image.bound(&curve, interval, false, &mut |n| budget.charge(n))? else {
            return invalid("face partition cannot bound its new spatial edge");
        };
        if bound > tolerance.absolute() {
            return invalid("face partition isocurve exceeds model tolerance");
        }
        if a.vertex == b.vertex
            && curve
                .control_points()
                .iter()
                .all(|p| p.point() == curve.control_points()[0].point())
        {
            return invalid("face partition cannot introduce a collapsed interior seam");
        }
        let edge = result.edges.len();
        result
            .edges
            .push(BrepEdge::try_new([a.vertex, b.vertex], curve, bound)?);
        for (which, arcs) in sides.iter_mut().enumerate() {
            let reverse = (which == 1) ^ (axis == 1);
            let [a, b] = if reverse { [*b, *a] } else { [*a, *b] };
            let [start, end] = if reverse {
                [-interval[1], -interval[0]]
            } else {
                interval
            };
            arcs.push(Arc {
                trim: BrepTrim::try_new(
                    [a.vertex, b.vertex],
                    Some(edge),
                    reverse,
                    NurbsCurve2::try_new(
                        1,
                        vec![a.point(), b.point()],
                        vec![start, start, end, end],
                    )?,
                    BrepTrimType::Mated,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )?,
                kind: BrepLoopType::Outer,
                connector: true,
            });
        }
    }
    let mut faces = Vec::new();
    let mut halves = sides.into_iter().zip(patches).collect::<Vec<_>>();
    // Keep the region on the left of the positive-transverse cut first:
    // low U for an upward cut, high V for a rightward cut. This also matches
    // the retained U/V, reversed-face and shifted-domain Rhino observations.
    if axis == 1 {
        halves.reverse();
    }
    for (arcs, surface) in halves {
        let loops = cycles(arcs, budget)?;
        let mut outers = Vec::new();
        let mut inners = Vec::new();
        for ring in loops {
            if ring.loop_type == BrepLoopType::Outer {
                // Concave clipping can create many components. Charge each
                // separate tensor-net copy before allocating its face.
                budget.charge(
                    surface
                        .control_points()
                        .len()
                        .saturating_add(surface.knots_u().len())
                        .saturating_add(surface.knots_v().len()),
                )?;
                outers.push(BrepFace::try_new(surface.clone(), reversed, vec![ring])?);
            } else {
                inners.push(ring);
            }
        }
        if outers.is_empty() {
            return invalid("face partition lost its outer loop");
        }
        // Original valid holes do not intersect the new outer rings. Determine
        // their unique owner using the existing trim containment predicate.
        for hole in inners {
            let p = hole.trims[0].curve.start_point()?;
            let mut owners = Vec::new();
            for (i, f) in outers.iter().enumerate() {
                for trim in f.loops.iter().flat_map(|l| &l.trims) {
                    budget.charge(
                        trim.curve.control_points().len().saturating_mul(
                            trim.curve.degree().saturating_add(1).saturating_pow(3),
                        ),
                    )?;
                }
                if f.contains_parameters(p.x(), p.y(), tolerance)? {
                    owners.push(i);
                }
            }
            let [owner] = owners.as_slice() else {
                return invalid("face partition hole ownership is ambiguous");
            };
            outers[*owner].loops.push(hole);
        }
        for face in &mut outers {
            for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                trim.iso = iso(&trim.curve, &surface);
            }
        }
        faces.extend(outers);
    }
    Ok(Some(faces))
}

/// `true` means the boundary already has an outgoing arc, so its connector
/// must arrive here. UV identity is separate from spatial vertex identity:
/// opposite sides of a closed surface seam must not be welded in parameter space.
fn open_ends(arcs: &[Arc], axis: usize, cut: Real) -> Result<Vec<(Node, bool)>, GeometryError> {
    let mut counts = BTreeMap::<Node, [usize; 2]>::new();
    for arc in arcs {
        let [a, b] = arc.nodes()?;
        counts.entry(a).or_default()[1] += 1;
        counts.entry(b).or_default()[0] += 1;
    }
    let mut ends = Vec::new();
    for (node, count) in counts {
        match count {
            [1, 1] => (),
            [1, 0] | [0, 1] if parameter_coordinate(node.point(), axis) == cut => {
                ends.push((node, count[1] == 1))
            }
            _ => return invalid("face partition boundary incidence is ambiguous"),
        }
    }
    ends.sort_by(|(a, _), (b, _)| {
        parameter_coordinate(a.point(), 1 - axis)
            .total_cmp(&parameter_coordinate(b.point(), 1 - axis))
    });
    Ok(ends)
}

fn cycles(arcs: Vec<Arc>, budget: &mut Budget) -> Result<Vec<BrepLoop>, GeometryError> {
    budget.charge(arcs.len())?;
    if arcs.len() > MAX_PARTS {
        return invalid("too many face partition trims");
    }
    let mut next = BTreeMap::new();
    for (i, arc) in arcs.iter().enumerate() {
        if next.insert(arc.nodes()?[0], i).is_some() {
            return invalid("face partition has multiple outgoing trims");
        }
    }
    // New-cut cycles precede untouched loops. The first connector anchors its
    // cycle: forward connector first, reversed connector last. Existing holes
    // and boundary cycles with no connector keep their original start.
    let order = arcs
        .iter()
        .enumerate()
        .filter_map(|(i, a)| a.connector.then_some(i))
        .chain(
            arcs.iter()
                .enumerate()
                .filter_map(|(i, a)| (!a.connector).then_some(i)),
        )
        .collect::<Vec<_>>();
    let mut arcs = arcs.into_iter().map(Some).collect::<Vec<_>>();
    let mut rings = Vec::new();
    for index in order {
        let Some(first) = &arcs[index] else { continue };
        let start = first.nodes()?[0];
        let original_kind = first.kind;
        let reverse_anchor = first.connector && first.trim.reversed_3d;
        let mut connector = false;
        let mut trims = Vec::new();
        let mut current = index;
        loop {
            let arc = arcs[current]
                .take()
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "face partition revisits an open trim",
                })?;
            let end = arc.nodes()?[1];
            connector |= arc.connector;
            trims.push(arc.trim);
            if end == start {
                break;
            }
            current = *next.get(&end).ok_or(GeometryError::InvalidBrepTopology {
                context: "face partition has an open loop",
            })?;
        }
        if reverse_anchor {
            trims.rotate_left(1);
        }
        rings.push(BrepLoop::try_new(
            if connector {
                BrepLoopType::Outer
            } else {
                original_kind
            },
            trims,
        )?);
    }
    Ok(rings)
}

fn iso(curve: &NurbsCurve2, surface: &NurbsSurface) -> SurfaceIso {
    let first = curve.control_points()[0].point();
    let u = surface.domain_u();
    let v = surface.domain_v();
    if curve
        .control_points()
        .iter()
        .all(|p| p.point().x() == first.x())
    {
        if first.x() == *u.start() {
            SurfaceIso::West
        } else if first.x() == *u.end() {
            SurfaceIso::East
        } else {
            SurfaceIso::InteriorUConstant
        }
    } else if curve
        .control_points()
        .iter()
        .all(|p| p.point().y() == first.y())
    {
        if first.y() == *v.start() {
            SurfaceIso::South
        } else if first.y() == *v.end() {
            SurfaceIso::North
        } else {
            SurfaceIso::InteriorVConstant
        }
    } else {
        SurfaceIso::NotIso
    }
}
