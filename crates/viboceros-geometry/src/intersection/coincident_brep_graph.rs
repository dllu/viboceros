//! Edge-disjoint curve paths for linear coincident B-rep face boundaries.

use crate::{GeometryError, NurbsCurve, Point3, Polyline3, Real, Tolerance};

#[derive(Clone, Copy)]
struct Edge {
    nodes: [usize; 2],
    virtual_edge: bool,
}

/// Covers each unique segment exactly once. A temporary vertex joins odd
/// junctions, giving each connected graph an Euler circuit. Removing its
/// temporary edges yields the minimum number of open paths; even components
/// remain closed circuits.
pub(super) fn cover_unique_segments(
    segments: &[Polyline3],
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let mut points = Vec::new();
    let mut edges = Vec::with_capacity(segments.len());
    for segment in segments {
        let vertices = segment.vertices();
        let start = vertex_index(vertices[0], &mut points, distance_tolerance)?;
        let end = vertex_index(vertices[1], &mut points, distance_tolerance)?;
        if start != end {
            edges.push(Edge {
                nodes: [start, end],
                virtual_edge: false,
            });
        }
    }
    if edges.is_empty() {
        return Ok(Vec::new());
    }

    let virtual_node = points.len();
    let mut adjacency = vec![Vec::new(); virtual_node + 1];
    for (edge_index, edge) in edges.iter().enumerate() {
        adjacency[edge.nodes[0]].push(edge_index);
        adjacency[edge.nodes[1]].push(edge_index);
    }
    let odd_nodes = (0..virtual_node)
        .filter(|&node| adjacency[node].len() % 2 == 1)
        .collect::<Vec<_>>();
    for node in &odd_nodes {
        let edge_index = edges.len();
        edges.push(Edge {
            nodes: [virtual_node, *node],
            virtual_edge: true,
        });
        adjacency[virtual_node].push(edge_index);
        adjacency[*node].push(edge_index);
    }

    let mut used = vec![false; edges.len()];
    let mut cursor = vec![0; adjacency.len()];
    let mut curves = Vec::new();
    if !odd_nodes.is_empty() {
        let (route_nodes, route_edges) =
            euler_circuit(virtual_node, &edges, &adjacency, &mut used, &mut cursor);
        append_paths(
            &route_nodes,
            &route_edges,
            Some(virtual_node),
            &points,
            &edges,
            tolerance,
            &mut curves,
        )?;
    }
    for node in 0..virtual_node {
        if adjacency[node].iter().all(|edge| used[*edge]) {
            continue;
        }
        let (route_nodes, route_edges) =
            euler_circuit(node, &edges, &adjacency, &mut used, &mut cursor);
        append_paths(
            &route_nodes,
            &route_edges,
            None,
            &points,
            &edges,
            tolerance,
            &mut curves,
        )?;
    }
    Ok(curves)
}

fn vertex_index(
    point: Point3,
    points: &mut Vec<Point3>,
    tolerance: Real,
) -> Result<usize, GeometryError> {
    for (index, existing) in points.iter().enumerate() {
        if existing.distance_to(point)? <= tolerance * 2.0 {
            return Ok(index);
        }
    }
    let index = points.len();
    points.push(point);
    Ok(index)
}

fn euler_circuit(
    start: usize,
    edges: &[Edge],
    adjacency: &[Vec<usize>],
    used: &mut [bool],
    cursor: &mut [usize],
) -> (Vec<usize>, Vec<usize>) {
    let mut node_stack = vec![start];
    let mut edge_stack = Vec::new();
    let mut route_nodes = Vec::new();
    let mut route_edges = Vec::new();
    while let Some(&node) = node_stack.last() {
        while cursor[node] < adjacency[node].len() && used[adjacency[node][cursor[node]]] {
            cursor[node] += 1;
        }
        if cursor[node] == adjacency[node].len() {
            route_nodes.push(node_stack.pop().expect("the traversal has a node"));
            if let Some(edge) = edge_stack.pop() {
                route_edges.push(edge);
            }
            continue;
        }
        let edge_index = adjacency[node][cursor[node]];
        cursor[node] += 1;
        used[edge_index] = true;
        let [first, second] = edges[edge_index].nodes;
        node_stack.push(if first == node { second } else { first });
        edge_stack.push(edge_index);
    }
    route_nodes.reverse();
    route_edges.reverse();
    (route_nodes, route_edges)
}

#[allow(clippy::too_many_arguments)]
fn append_paths(
    route_nodes: &[usize],
    route_edges: &[usize],
    virtual_node: Option<usize>,
    points: &[Point3],
    edges: &[Edge],
    tolerance: Tolerance,
    curves: &mut Vec<NurbsCurve>,
) -> Result<(), GeometryError> {
    if route_edges.is_empty() {
        return Ok(());
    }
    if let Some(virtual_node) = virtual_node {
        let first_departure = route_edges
            .iter()
            .enumerate()
            .find(|(index, edge)| edges[**edge].virtual_edge && route_nodes[*index] == virtual_node)
            .map(|(index, _)| index)
            .expect("an augmented circuit departs the virtual node");
        let edge_count = route_edges.len();
        let mut path = vec![points[route_nodes[(first_departure + 1) % edge_count]]];
        for offset in 0..edge_count - 1 {
            let index = (first_departure + 1 + offset) % edge_count;
            let next = route_nodes[(index + 1) % edge_count];
            if edges[route_edges[index]].virtual_edge {
                if next == virtual_node {
                    append_polyline(&path, tolerance, curves)?;
                    path.clear();
                } else {
                    path.push(points[next]);
                }
            } else {
                path.push(points[next]);
            }
        }
        append_polyline(&path, tolerance, curves)?;
    } else {
        let path = route_nodes
            .iter()
            .map(|&node| points[node])
            .collect::<Vec<_>>();
        append_polyline(&path, tolerance, curves)?;
    }
    Ok(())
}

fn append_polyline(
    points: &[Point3],
    tolerance: Tolerance,
    curves: &mut Vec<NurbsCurve>,
) -> Result<(), GeometryError> {
    if points.len() >= 2 {
        curves.push(Polyline3::try_new(points.to_vec(), tolerance)?.to_nurbs()?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    #[test]
    fn covers_branched_and_disconnected_edge_components_once() {
        let edges = [
            [point(0.0, 0.0), point(1.0, 0.0)],
            [point(0.0, 0.0), point(0.0, 1.0)],
            [point(0.0, 0.0), point(-1.0, 0.0)],
            [point(10.0, 0.0), point(11.0, 0.0)],
            [point(11.0, 0.0), point(10.0, 1.0)],
            [point(10.0, 1.0), point(10.0, 0.0)],
        ];
        let polylines = edges
            .iter()
            .map(|edge| Polyline3::try_new(edge.to_vec(), Tolerance::DEFAULT).unwrap())
            .collect::<Vec<_>>();
        let curves = cover_unique_segments(&polylines, Tolerance::DEFAULT, 1e-9).unwrap();
        assert_eq!(
            curves.len(),
            3,
            "a T has two trails and the triangle has one loop"
        );
        assert_eq!(
            curves
                .iter()
                .map(|curve| curve.control_points().len() - 1)
                .sum::<usize>(),
            edges.len()
        );
        assert_eq!(
            curves
                .iter()
                .filter(|curve| curve.is_closed().unwrap())
                .count(),
            1
        );
    }
}
