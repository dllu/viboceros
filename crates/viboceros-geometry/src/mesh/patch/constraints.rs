//! Ordered guide recovery and boundary metadata, without generated vertices.
use super::*;
use spade::handles::{FixedDirectedEdgeHandle, FixedVertexHandle};
use spade::{DelaunayTriangulation, Intersection, LineIntersectionIterator};
mod segments;
type Cdt = ConstrainedDelaunayTriangulation<Vertex>;
const MAX_WORK: usize = 4_000_000;
struct Recovered {
    edge: FixedDirectedEdgeHandle,
    original_ends: [usize; 2],
    chain: usize,
}
fn charge(work: &mut usize, amount: usize) -> Result<(), GeometryError> {
    *work = work
        .checked_add(amount)
        .filter(|n| *n <= MAX_WORK)
        .ok_or(GeometryError::MeshPatchLimit)?;
    Ok(())
}
#[derive(Default)]
pub(super) struct Constraints {
    pub(super) barriers: BTreeSet<usize>,
    pub(super) holes: Vec<(FixedDirectedEdgeHandle, bool)>,
    pub(super) thin_guides: BTreeSet<usize>,
    partial_holes: Vec<(FixedDirectedEdgeHandle, bool)>,
    guide_cuts: Vec<(FixedDirectedEdgeHandle, FixedVertexHandle)>,
    overlap_cuts: Vec<FixedDirectedEdgeHandle>,
    outer_candidates: Vec<(Vec<usize>, Vec<FixedDirectedEdgeHandle>, bool)>,
}
impl Constraints {
    pub(super) fn install(
        cdt: &mut Cdt,
        coordinates: &[[Real; 2]],
        remap: &[usize],
        chains: &[(Range<usize>, bool)],
        (guide_count, hole_count): (usize, usize),
        automatic: bool,
        point_nodes: &BTreeSet<usize>,
    ) -> Result<Self, GeometryError> {
        let mut result = Self::default();
        let mut work = 0usize;
        let mut recovered = BTreeMap::<usize, Recovered>::new();
        let mut handles = vec![None; coordinates.len()];
        for v in cdt.vertices() {
            handles[v.data().source] = Some(v.fix());
        }
        let prepared = segments::prepare(
            cdt,
            remap,
            chains,
            guide_count,
            point_nodes,
            automatic,
            &mut work,
        )?;
        for chain_index in prepared.order {
            let (range, barrier) = &chains[chain_index];
            let area = if *barrier {
                boundary_orientation(
                    &range
                        .clone()
                        .map(|i| coordinates[remap[i]])
                        .collect::<Vec<_>>(),
                )
            } else {
                Ordering::Equal
            };
            let is_guide = chain_index < guide_count;
            let is_hole = chain_index >= guide_count && chain_index < guide_count + hole_count;
            let mut accepted = Vec::new();
            let mut complete = true;
            for segment in &prepared.pairs[chain_index] {
                let pair = segment.nodes;
                if pair[0] == pair[1] {
                    continue;
                }
                let from = handles[pair[0]].unwrap();
                let to = handles[pair[1]].unwrap();
                let mut blocked = None;
                let mut overlap_cut = None;
                // Charge the actual traversal before Spade performs recovery.
                // A conflicting edge keeps its earlier source priority; there
                // is no spatial sample from which to create a crossing vertex.
                for intersection in LineIntersectionIterator::new_from_handles(cdt, from, to) {
                    charge(&mut work, 1)?;
                    if automatic
                        && is_guide
                        && let Intersection::EdgeOverlap(edge) = intersection
                        && edge.from().data().source == pair[0]
                        && let Some(prior) = recovered.get(&edge.as_undirected().index())
                        && prior.chain != chain_index
                        && prior.edge == edge.fix()
                        && !prior.original_ends.contains(&pair[0])
                    {
                        overlap_cut = Some(edge.fix());
                    }
                    if let Some(edge) = intersection
                        .as_edge_intersection()
                        .filter(|e| e.is_constraint_edge())
                    {
                        blocked = Some(edge.fix());
                        break;
                    }
                }
                if let Some(edge) = blocked {
                    complete = false;
                    if is_guide {
                        if automatic && segment.original {
                            let endpoint = if point_nodes.contains(&pair[1])
                                && !point_nodes.contains(&pair[0])
                            {
                                from
                            } else {
                                to
                            };
                            result.guide_cuts.push((edge, endpoint));
                        }
                        continue;
                    }
                    if is_hole {
                        continue;
                    }
                    return Err(GeometryError::MeshPatchCrossingConstraints);
                }
                let edges = cdt.try_add_constraint(from, to);
                if edges.is_empty() {
                    return Err(GeometryError::MeshPatchCrossingConstraints);
                }
                charge(&mut work, edges.len())?;
                if let Some(edge) = overlap_cut {
                    result.overlap_cuts.push(edge);
                }
                if is_guide {
                    for &edge in &edges {
                        recovered
                            .entry(edge.as_undirected().index())
                            .or_insert(Recovered {
                                edge,
                                original_ends: segment.original_ends,
                                chain: chain_index,
                            });
                    }
                }
                if *barrier {
                    for &edge in &edges {
                        result.barriers.insert(edge.as_undirected().index());
                        if is_guide && area == Ordering::Equal {
                            result.thin_guides.insert(edge.as_undirected().index());
                        }
                    }
                }
                accepted.extend(edges);
            }
            if *barrier && !is_guide && area == Ordering::Equal {
                return Err(GeometryError::Degenerate {
                    context: "mesh patch boundary area",
                });
            }
            let left = area == Ordering::Greater;
            if is_hole {
                if complete {
                    if let Some(&edge) = accepted.first() {
                        result.holes.push((edge, left));
                    }
                } else {
                    // A broken hole cannot seed a closed-region flood. The
                    // captured command trims only its recovered inside faces.
                    result
                        .partial_holes
                        .extend(accepted.into_iter().map(|e| (e, left)));
                }
            } else if is_guide
                && complete
                && remap[range.start] == remap[range.end - 1]
                && area != Ordering::Equal
            {
                result
                    .outer_candidates
                    .push((remap[range.clone()].to_vec(), accepted, left));
            }
        }
        Ok(result)
    }
    pub(super) fn outer_seeds(
        &self,
        cdt: &Cdt,
        remap: &[usize],
        source_count: usize,
    ) -> Result<Vec<usize>, GeometryError> {
        if self.guide_cuts.is_empty() {
            return Ok(vec![]);
        }
        let source = remap[..source_count]
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let hull = DelaunayTriangulation::<Vertex>::bulk_load(
            cdt.vertices()
                .filter(|vertex| source.contains(&vertex.data().source))
                .map(|vertex| vertex.data().clone())
                .collect(),
        )
        .map_err(|_| GeometryError::MeshPatchCrossingConstraints)?;
        let hull = hull
            .convex_hull()
            .map(|edge| edge.from().data().source)
            .collect::<BTreeSet<_>>();
        let mut seeds = Vec::new();
        // During failed guide recovery, captured hull-touching closed guides
        // retain their interiors even when explicit point seeds lie outside.
        for (nodes, edges, left) in &self.outer_candidates {
            if nodes.iter().any(|i| hull.contains(i)) {
                for edge in edges {
                    let edge = cdt.directed_edge(*edge);
                    seeds.push(if *left {
                        edge.face().index()
                    } else {
                        edge.rev().face().index()
                    });
                }
            }
        }
        Ok(seeds)
    }
    pub(super) fn mark_partial_faces(
        &self,
        cdt: &Cdt,
        points: &BTreeSet<usize>,
        removed: &mut [bool],
    ) {
        for edge in &self.overlap_cuts {
            removed[cdt.directed_edge(*edge).rev().face().index()] = true;
        }
        for (edge, left) in &self.partial_holes {
            let edge = cdt.directed_edge(*edge);
            removed[if *left {
                edge.face().index()
            } else {
                edge.rev().face().index()
            }] = true;
        }
        for (edge, to) in &self.guide_cuts {
            if points.contains(&cdt.vertex(*to).data().source) {
                continue;
            }
            let edge = cdt.directed_edge(*edge);
            for face in [edge.face(), edge.rev().face()] {
                if face
                    .as_inner()
                    .is_some_and(|f| f.vertices().iter().any(|v| v.fix() == *to))
                {
                    removed[face.index()] = true;
                }
            }
        }
    }
}
