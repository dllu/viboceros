use super::*;

pub(super) fn push_unique(
    edges: &mut Vec<SurfaceCutArrangementEdge>,
    index: &mut BTreeMap<(usize, usize), Vec<usize>>,
    maximum: usize,
    edge: SurfaceCutArrangementEdge,
    tolerance: Tolerance,
    closed_sources: &[bool],
) -> Result<(), GeometryError> {
    let SurfaceCutArrangementEdgeKind::Cut { source, segment } = edge.kind else {
        return push_surface_cut_arrangement_edge(edges, edge);
    };
    let key = (
        edge.nodes[0].min(edge.nodes[1]),
        edge.nodes[0].max(edge.nodes[1]),
    );
    for &candidate in index.get(&key).into_iter().flatten() {
        let existing = &mut edges[candidate];
        let SurfaceCutArrangementEdgeKind::Cut {
            source: existing_source,
            ..
        } = existing.kind
        else {
            continue;
        };
        if !(existing.nodes == edge.nodes || existing.nodes == [edge.nodes[1], edge.nodes[0]]) {
            continue;
        }
        for event in edge
            .spatial
            .intersection_events_with_curve(&existing.spatial, tolerance)?
        {
            let CurveCurveIntersectionEvent::Overlap(overlap) = event else {
                continue;
            };
            if surface_cut_overlap_covers_curves(&edge.spatial, &existing.spatial, overlap) {
                let same_direction = existing.nodes == edge.nodes;
                // An open contributor wins over a closed loop. Within the
                // same topology class, continuity is Rhino's tie-breaker;
                // exact ties remain stable in cutter order.
                let prefer_incoming = (closed_sources[existing_source] && !closed_sources[source])
                    || (closed_sources[existing_source] == closed_sources[source]
                        && edge.smooth_continuations > existing.smooth_continuations);
                if prefer_incoming {
                    let SurfaceCutArrangementEdgeKind::Cut {
                        source: previous_source,
                        segment: previous_segment,
                    } = existing.kind
                    else {
                        unreachable!("a coincident arrangement edge is a cutter")
                    };
                    let mut coincidences = std::mem::take(&mut existing.coincidences);
                    if !same_direction {
                        for coincidence in &mut coincidences {
                            coincidence.same_direction = !coincidence.same_direction;
                        }
                    }
                    coincidences.push(SurfaceCutArrangementCoincidence {
                        source: previous_source,
                        segment: previous_segment,
                        same_direction,
                    });
                    *existing = edge;
                    existing.coincidences = coincidences;
                } else {
                    existing
                        .coincidences
                        .push(SurfaceCutArrangementCoincidence {
                            source,
                            segment,
                            same_direction,
                        });
                }
                return Ok(());
            }
        }
    }
    if edges.len() >= maximum {
        return invalid("a surface cutting arrangement exceeds its work limit");
    }
    index.entry(key).or_default().push(edges.len());
    push_surface_cut_arrangement_edge(edges, edge)
}
