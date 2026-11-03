use super::index::{HullIndex, NodeIndex};
use super::*;

pub(in crate::brep) fn build(
    surface: &NurbsSurface,
    bounds: [[Real; 2]; 2],
    cut_curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    network: bool,
    maximum: usize,
) -> Result<SurfaceCutGraph, GeometryError> {
    if cut_curves.len() > maximum || maximum < 4 {
        return invalid("a surface cutting arrangement exceeds its work limit");
    }
    let parameter_tolerance = [
        trim_parameter_epsilon(bounds[0], tolerance),
        trim_parameter_epsilon(bounds[1], tolerance),
    ];
    let mut cuts = Vec::<SurfaceCutArrangementCurve>::new();
    let mut hulls = Vec::new();
    let mut hull_index = HullIndex::default();
    let mut pair_work = 0usize;
    for curve in cut_curves {
        let candidate = try_surface_cut_arrangement_curve(
            surface,
            bounds,
            curve,
            parameter_tolerance,
            tolerance,
            network,
        )?;
        let hull = HullIndex::hull(&candidate.spatial);
        let mut duplicate = false;
        for index in hull_index.candidates(hull, tolerance) {
            let existing = &cuts[index];
            pair_work += 1;
            if pair_work > 16_000_000 {
                return invalid("a surface cutting arrangement exceeds its pair-work limit");
            }
            if !index::may_duplicate(&candidate, existing, tolerance).unwrap_or(true) {
                continue;
            }
            for event in candidate
                .spatial
                .intersection_events_with_curve(&existing.spatial, tolerance)?
            {
                let CurveCurveIntersectionEvent::Overlap(overlap) = event else {
                    continue;
                };
                if surface_cut_overlap_covers_curves(&candidate.spatial, &existing.spatial, overlap)
                {
                    duplicate = true;
                }
            }
            if duplicate {
                break;
            }
        }
        if !duplicate {
            cuts.push(candidate);
            hulls.push(hull);
            hull_index.insert(hull);
        }
    }
    if cuts.is_empty() {
        return invalid("a surface cutting arrangement requires a distinct curve");
    }
    let closed = cuts.iter().map(|cut| cut.closed).collect::<Vec<_>>();

    let corner_parameters = [
        Point2::try_new(bounds[0][0], bounds[1][0])?,
        Point2::try_new(bounds[0][1], bounds[1][0])?,
        Point2::try_new(bounds[0][1], bounds[1][1])?,
        Point2::try_new(bounds[0][0], bounds[1][1])?,
    ];
    let mut nodes = corner_parameters
        .into_iter()
        .map(|parameter| {
            Ok(SurfaceCutArrangementNode {
                parameter,
                point: surface.evaluate(parameter.x(), parameter.y())?,
            })
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut node_index = NodeIndex::new(&nodes);
    let mut break_count = 0usize;
    for cut in &mut cuts {
        let domain = cut.spatial.domain();
        let endpoint_parameters = [*domain.start(), *domain.end()];
        for (end, endpoint_parameter) in endpoint_parameters.into_iter().enumerate() {
            let parameter = cut.endpoints[end];
            let node = node_index.node(
                &mut nodes,
                parameter,
                surface.evaluate(parameter.x(), parameter.y())?,
                parameter_tolerance,
                maximum,
            )?;
            check_break_budget(&mut break_count, 1, maximum)?;
            cut.breaks.push(SurfaceCutArrangementBreak {
                parameter: endpoint_parameter,
                node,
            });
        }
        for knot in cut.spatial.full_multiplicity_kink_parameters(tolerance)? {
            let parameter = cut.parameter.evaluate(knot)?;
            let node = node_index.node(
                &mut nodes,
                parameter,
                cut.spatial.evaluate(knot)?,
                parameter_tolerance,
                maximum,
            )?;
            check_break_budget(&mut break_count, 1, maximum)?;
            cut.breaks.push(SurfaceCutArrangementBreak {
                parameter: knot,
                node,
            });
        }
    }

    for first_index in 0..cuts.len() {
        for second_index in hull_index.candidates(hulls[first_index], tolerance) {
            if second_index <= first_index {
                continue;
            }
            pair_work += 1;
            if pair_work > 16_000_000 {
                return invalid("a surface cutting arrangement exceeds its pair-work limit");
            }
            let (first_slice, second_slice) = cuts.split_at_mut(second_index);
            let first = &mut first_slice[first_index];
            let second = &mut second_slice[0];
            if network
                && index::only_common_endpoint(&first.spatial, &second.spatial, tolerance)
                    .unwrap_or(false)
            {
                continue;
            }
            let events = first
                .spatial
                .intersection_events_with_curve(&second.spatial, tolerance)?;
            for event in events {
                let contacts = match event {
                    CurveCurveIntersectionEvent::Point(intersection) => [Some(intersection), None],
                    CurveCurveIntersectionEvent::Overlap(overlap) => {
                        [Some(overlap.start()), Some(overlap.end())]
                    }
                };
                for intersection in contacts.into_iter().flatten() {
                    let first_parameter = intersection.first_parameter();
                    let second_parameter = intersection.second_parameter();
                    let first_uv = first.parameter.evaluate(first_parameter)?;
                    let second_uv = second.parameter.evaluate(second_parameter)?;
                    if !parameter_points_near(
                        first_uv,
                        second_uv,
                        parameter_tolerance.map(|value| value * 8.0),
                    ) {
                        return invalid(
                            "a model-space cutter intersection is not shared in parameter space",
                        );
                    }
                    let averaged = Point2::try_new(
                        first_uv.x() * 0.5 + second_uv.x() * 0.5,
                        first_uv.y() * 0.5 + second_uv.y() * 0.5,
                    )?;
                    let (parameter, _) = snap_surface_cut_arrangement_parameter(
                        averaged,
                        bounds,
                        parameter_tolerance,
                    )?;
                    let node = node_index.node(
                        &mut nodes,
                        parameter,
                        intersection.point(),
                        parameter_tolerance,
                        maximum,
                    )?;
                    // Bound contributor records even when many coincident
                    // cutters reuse a small number of geometric nodes.
                    let before = first.breaks.len() + second.breaks.len();
                    push_surface_cut_arrangement_break(first, first_parameter, node)?;
                    push_surface_cut_arrangement_break(second, second_parameter, node)?;
                    check_break_budget(
                        &mut break_count,
                        first.breaks.len() + second.breaks.len() - before,
                        maximum,
                    )?;
                }
            }
        }
    }

    let mut node_rank = vec![usize::MAX; nodes.len()];
    let mut next_node_rank = 0;
    for rank in node_rank.iter_mut().take(4) {
        *rank = next_node_rank;
        next_node_rank += 1;
    }
    for cut in &cuts {
        let mut breaks = cut.breaks.clone();
        breaks.sort_by(|left, right| left.parameter.total_cmp(&right.parameter));
        for split in breaks {
            if node_rank[split.node] == usize::MAX {
                node_rank[split.node] = next_node_rank;
                next_node_rank += 1;
            }
        }
    }
    if node_rank.contains(&usize::MAX) {
        return invalid("a surface cutting arrangement node is not on a cutter");
    }

    let mut edges = Vec::<SurfaceCutArrangementEdge>::new();
    let mut edge_index = BTreeMap::<(usize, usize), Vec<usize>>::new();
    let boundary_iso = rectangular_surface_boundary_iso(surface, bounds);
    for side in [
        RectangularBoundarySide::South,
        RectangularBoundarySide::East,
        RectangularBoundarySide::North,
        RectangularBoundarySide::West,
    ] {
        let mut side_nodes = nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                surface_cut_parameter_on_side(node.parameter, side, bounds, parameter_tolerance)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        side_nodes.sort_by(|&left, &right| {
            let coordinate = |node: usize| match side {
                RectangularBoundarySide::South | RectangularBoundarySide::North => {
                    nodes[node].parameter.x()
                }
                RectangularBoundarySide::East | RectangularBoundarySide::West => {
                    nodes[node].parameter.y()
                }
            };
            let ordering = coordinate(left).total_cmp(&coordinate(right));
            if matches!(
                side,
                RectangularBoundarySide::North | RectangularBoundarySide::West
            ) {
                ordering.reverse()
            } else {
                ordering
            }
        });
        side_nodes.dedup();
        for pair in side_nodes.windows(2) {
            if pair[0] == pair[1] {
                continue;
            }
            let start = nodes[pair[0]].parameter;
            let end = nodes[pair[1]].parameter;
            if edges.len() >= maximum {
                return invalid("a surface cutting arrangement exceeds its work limit");
            }
            push_surface_cut_arrangement_edge(
                &mut edges,
                SurfaceCutArrangementEdge {
                    nodes: [pair[0], pair[1]],
                    spatial: rectangular_surface_boundary_segment(surface, side, start, end)?,
                    parameter: NurbsCurve2::try_line(start, end)?,
                    iso: boundary_iso[side.index()],
                    kind: SurfaceCutArrangementEdgeKind::Boundary(side),
                    smooth_continuations: 0,
                    coincidences: Vec::new(),
                },
            )?;
        }
    }

    for (cut_index, cut) in cuts.iter_mut().enumerate() {
        cut.breaks
            .sort_by(|left, right| left.parameter.total_cmp(&right.parameter));
        let parameter_epsilon = surface_cut_curve_parameter_epsilon(&cut.spatial);
        let mut unique_breaks = Vec::<SurfaceCutArrangementBreak>::new();
        for split in cut.breaks.iter().copied() {
            if let Some(previous) = unique_breaks.last()
                && (previous.parameter - split.parameter).abs() <= parameter_epsilon
            {
                if previous.node != split.node {
                    return invalid("one cutter parameter maps to distinct arrangement nodes");
                }
                continue;
            }
            unique_breaks.push(split);
        }
        let domain = cut.spatial.domain();
        for (segment_index, pair) in unique_breaks.windows(2).enumerate() {
            let complete_closed_edge = cut.closed
                && unique_breaks.len() == 2
                && pair[0].parameter == *domain.start()
                && pair[1].parameter == *domain.end();
            if (pair[0].node == pair[1].node && !complete_closed_edge)
                || pair[1].parameter - pair[0].parameter <= parameter_epsilon
            {
                continue;
            }
            let spatial =
                if pair[0].parameter == *domain.start() && pair[1].parameter == *domain.end() {
                    cut.spatial.clone()
                } else {
                    cut.spatial.try_trimmed_with_normalized_end_weights(
                        pair[0].parameter..=pair[1].parameter,
                    )?
                };
            let start = nodes[pair[0].node].parameter;
            let end = nodes[pair[1].node].parameter;
            let parameter =
                surface_split_parameter_curve(surface, &spatial, start, end, tolerance)?;
            let iso = surface_cut_arrangement_iso(&parameter, bounds, parameter_tolerance)?;
            // Rhino retains the coincident contributor that continues
            // tangentially beyond the shared span rather than one that kinks.
            let smooth_continuations = [pair[0].parameter, pair[1].parameter]
                .into_iter()
                .filter(|parameter| {
                    *parameter > *domain.start() + parameter_epsilon
                        && *parameter < *domain.end() - parameter_epsilon
                })
                .map(|parameter| {
                    cut.spatial
                        .kink_angle_at(parameter)
                        .map(|angle| u8::from(angle <= tolerance.angular()))
                })
                .try_fold(0_u8, |total, continuation| {
                    continuation.map(|continuation| total + continuation)
                })?;
            edges::push_unique(
                &mut edges,
                &mut edge_index,
                maximum,
                SurfaceCutArrangementEdge {
                    nodes: [pair[0].node, pair[1].node],
                    spatial,
                    parameter,
                    iso,
                    kind: SurfaceCutArrangementEdgeKind::Cut {
                        source: cut_index,
                        segment: segment_index,
                    },
                    smooth_continuations,
                    coincidences: Vec::new(),
                },
                tolerance,
                &closed,
            )?;
            if edges.len() > maximum {
                return invalid("a surface cutting arrangement exceeds its work limit");
            }
        }
    }

    if edges.len() > maximum {
        return invalid("a surface cutting arrangement exceeds its work limit");
    }
    // Rhino uses the open curve as the representative of an open/closed
    // overlap. If the closed curve was supplied first, its local vertex order
    // advances from the node following the replaced edge rather than from the
    // original closed seam.
    if closed[0]
        && let Some(overlap_end) = edges.iter().find_map(|edge| {
            let SurfaceCutArrangementEdgeKind::Cut { source, .. } = edge.kind else {
                return None;
            };
            if closed[source] {
                return None;
            }
            edge.coincidences
                .iter()
                .find(|coincidence| coincidence.source == 0)
                .map(|coincidence| edge.nodes[usize::from(coincidence.same_direction)])
        })
    {
        let mut first_nodes = cuts[0]
            .breaks
            .iter()
            .map(|split| split.node)
            .collect::<Vec<_>>();
        first_nodes.sort_by_key(|node| node_rank[*node]);
        first_nodes.dedup();
        if let Some(start) = first_nodes.iter().position(|node| *node == overlap_end) {
            first_nodes.rotate_left(start);
            let mut ranks = first_nodes
                .iter()
                .map(|node| node_rank[*node])
                .collect::<Vec<_>>();
            ranks.sort_unstable();
            for (node, rank) in first_nodes.into_iter().zip(ranks) {
                node_rank[node] = rank;
            }
        }
    }

    let halfedge_count = edges
        .len()
        .checked_mul(2)
        .ok_or(GeometryError::TooManySurfaceWires)?;
    let halfedge_directions = (0..halfedge_count)
        .map(|halfedge| surface_cut_halfedge_direction(&edges, halfedge))
        .collect::<Result<Vec<_>, _>>()?;
    let mut outgoing = vec![Vec::<usize>::new(); nodes.len()];
    for (edge_index, edge) in edges.iter().enumerate() {
        outgoing[edge.nodes[0]].push(edge_index * 2);
        outgoing[edge.nodes[1]].push(edge_index * 2 + 1);
    }
    // A tangential contact has the same unoriented tangent line on the first
    // cutter and a later cutter. Rhino uses a different trim seam at these
    // pinched vertices than it does at a transverse crossing.
    let parallel_tolerance = tolerance.angular().clamp(Real::EPSILON * 256.0, 1.0e-7);
    let first_tangent_nodes = outgoing
        .iter()
        .map(|halfedges| {
            halfedges.iter().copied().any(|first_halfedge| {
                if !matches!(
                    edges[first_halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
                ) {
                    return false;
                }
                let first_tangent = halfedge_directions[first_halfedge].tangent;
                halfedges.iter().copied().any(|second_halfedge| {
                    if !matches!(
                        edges[second_halfedge / 2].kind,
                        SurfaceCutArrangementEdgeKind::Cut { source, .. } if source != 0
                    ) {
                        return false;
                    }
                    let second_tangent = halfedge_directions[second_halfedge].tangent;
                    first_tangent[0]
                        .mul_add(second_tangent[1], -first_tangent[1] * second_tangent[0])
                        .abs()
                        <= parallel_tolerance
                })
            })
        })
        .collect::<Vec<_>>();
    if network && outgoing.iter().any(|list| list.len() == 1) {
        return invalid("a surface cutting network contains a dangling endpoint");
    }
    let mut next = vec![usize::MAX; halfedge_count];
    for halfedges in &mut outgoing {
        sort_surface_cut_outgoing_halfedges(halfedges, &halfedge_directions, tolerance.angular());
        for (index, &outgoing_halfedge) in halfedges.iter().enumerate() {
            let clockwise = halfedges[(index + halfedges.len() - 1) % halfedges.len()];
            next[outgoing_halfedge ^ 1] = clockwise;
        }
    }
    if next.contains(&usize::MAX) {
        return invalid("a surface cutting arrangement contains a dangling edge");
    }

    let mut visited = vec![false; halfedge_count];
    let mut outer_cycles = Vec::<Vec<usize>>::new();
    let mut hole_cycles = Vec::<Vec<usize>>::new();
    for start in 0..halfedge_count {
        if visited[start] {
            continue;
        }
        let mut cycle = Vec::new();
        let mut current = start;
        loop {
            if visited[current] {
                if current != start {
                    return invalid("surface cutting arrangement face traversal crossed itself");
                }
                break;
            }
            visited[current] = true;
            cycle.push(current);
            current = next[current];
            if cycle.len() > halfedge_count {
                return invalid("surface cutting arrangement face traversal did not close");
            }
        }
        if cycle.is_empty() {
            continue;
        }
        if let Some(area) = sampled_surface_cut_cycle_signed_area(&edges, &cycle)? {
            if area > 0.0 {
                outer_cycles.push(cycle);
            } else {
                hole_cycles.push(cycle);
            }
        }
    }
    if outer_cycles.len() < 2 {
        return invalid("surface cutting arrangement did not divide the rectangular region");
    }
    // The halfedge walk emits the unbounded exterior clockwise as well as
    // clockwise inner boundaries. Only the exterior contains rectangle edges.
    hole_cycles.retain(|cycle| {
        !cycle.iter().any(|halfedge| {
            matches!(
                edges[*halfedge / 2].kind,
                SurfaceCutArrangementEdgeKind::Boundary(_)
            )
        })
    });
    if outer_cycles.len() + hole_cycles.len() > crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }
    let first_closed = cuts.iter().position(|cut| cut.closed);
    let mut first_source_nodes = vec![false; nodes.len()];
    let mut first_overlap_nodes = vec![false; nodes.len()];
    let mut first_opposite_overlap_nodes = vec![false; nodes.len()];
    let mut first_boundary_overlap_edges = vec![false; edges.len()];
    let mut first_maximum_segment = 0_usize;
    for (edge_index, edge) in edges.iter().enumerate() {
        if let Some(first_contributor) = surface_cut_arrangement_edge_contributor(edge, 0) {
            first_maximum_segment = first_maximum_segment.max(first_contributor.segment);
            first_boundary_overlap_edges[edge_index] = !edge.coincidences.is_empty()
                && edge.nodes.into_iter().any(|node| {
                    [
                        RectangularBoundarySide::South,
                        RectangularBoundarySide::East,
                        RectangularBoundarySide::North,
                        RectangularBoundarySide::West,
                    ]
                    .into_iter()
                    .any(|side| {
                        surface_cut_parameter_on_side(
                            nodes[node].parameter,
                            side,
                            bounds,
                            parameter_tolerance,
                        )
                    })
                });
            for node in edge.nodes {
                first_source_nodes[node] = true;
                if !edge.coincidences.is_empty() {
                    first_overlap_nodes[node] = true;
                }
                if surface_cut_arrangement_edge_contributors(edge).any(|contributor| {
                    contributor.source != 0
                        && contributor.same_direction != first_contributor.same_direction
                }) {
                    first_opposite_overlap_nodes[node] = true;
                }
            }
        }
    }
    let source_info = SurfaceCutArrangementSourceInfo {
        node_rank,
        first_closed,
        closed,
        first_nodes: first_source_nodes,
        first_tangent_nodes,
        first_overlap_nodes,
        first_opposite_overlap_nodes,
        first_boundary_overlap_edges,
        first_maximum_segment,
    };
    for cycle in &mut outer_cycles {
        rotate_surface_cut_outer_cycle(
            cycle,
            &edges,
            &nodes,
            bounds,
            parameter_tolerance,
            &source_info,
        )?;
    }
    let mut holes_by_outer = vec![Vec::<Vec<usize>>::new(); outer_cycles.len()];
    for mut cycle in hole_cycles {
        rotate_surface_cut_hole_cycle(
            &mut cycle,
            &edges,
            &nodes,
            bounds,
            parameter_tolerance,
            &source_info,
        )?;
        let parent =
            surface_cut_hole_parent(&edges, &outer_cycles, &cycle, bounds, parameter_tolerance)?
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "a surface cutting arrangement hole has no containing face",
                })?;
        holes_by_outer[parent].push(cycle);
    }
    for holes in &mut holes_by_outer {
        holes.sort_by(|left, right| {
            // Rhino places kinky closed loops before smooth one-edge loops.
            // Source order is stable within kinky loops and reversed within
            // the self-edge representation used for smooth closed curves.
            right
                .len()
                .cmp(&left.len())
                .then_with(|| {
                    let left_source = surface_cut_cycle_source_key(&edges, left);
                    let right_source = surface_cut_cycle_source_key(&edges, right);
                    if left.len() == 1 {
                        right_source.cmp(&left_source)
                    } else {
                        left_source.cmp(&right_source)
                    }
                })
                .then_with(|| {
                    surface_cut_cycle_bounds(&edges, &nodes, left)
                        .into_iter()
                        .zip(surface_cut_cycle_bounds(&edges, &nodes, right))
                        .find_map(|(left, right)| {
                            let ordering = left.total_cmp(&right);
                            (ordering != std::cmp::Ordering::Equal).then_some(ordering)
                        })
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
    }

    let mut regions = outer_cycles
        .into_iter()
        .zip(holes_by_outer)
        .map(|(outer, holes)| SurfaceCutArrangementRegion { outer, holes })
        .collect::<Vec<_>>();
    regions.sort_by(|left, right| {
        surface_cut_cycle_bounds(&edges, &nodes, &left.outer)
            .into_iter()
            .zip(surface_cut_cycle_bounds(&edges, &nodes, &right.outer))
            .find_map(|(left, right)| {
                let ordering = left.total_cmp(&right);
                (ordering != std::cmp::Ordering::Equal).then_some(ordering)
            })
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(SurfaceCutGraph {
        nodes,
        edges,
        regions,
        source_info,
    })
}

fn check_break_budget(
    count: &mut usize,
    extra: usize,
    maximum: usize,
) -> Result<(), GeometryError> {
    *count = count.saturating_add(extra);
    if *count > maximum.saturating_mul(4) {
        return invalid("a surface cutting arrangement exceeds its contributor-work limit");
    }
    Ok(())
}
