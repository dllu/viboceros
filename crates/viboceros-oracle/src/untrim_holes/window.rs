//! Independent Top-view rectangle adapter for planar polygon probe sources.
//! This probe rejects nonlinear or tilted sources; production viewport picking
//! uses its display mesh, depth clipping, and real projected edge segments.
use super::*;
use viboceros_command::UntrimHolesComponent;

type Point = [f64; 2];

pub(crate) fn picks(
    document: &Document,
    ids: &[ObjectId],
    corners: [[f64; 3]; 2],
    all: bool,
) -> Result<Vec<(ObjectId, UntrimHolesComponent)>, ProbeError> {
    let crossing = corners[1][0] < corners[0][0];
    let low = [
        corners[0][0].min(corners[1][0]),
        corners[0][1].min(corners[1][1]),
    ];
    let high = [
        corners[0][0].max(corners[1][0]),
        corners[0][1].max(corners[1][1]),
    ];
    if low[0] == high[0] || low[1] == high[1] {
        return Ok(Vec::new());
    }
    let contains =
        |point: Point| (0..2).all(|axis| point[axis] >= low[axis] && point[axis] <= high[axis]);
    let crosses =
        |segments: &[[Point; 2]]| segments.iter().any(|&[a, b]| intersects(a, b, low, high));
    let enclosed = |segments: &[[Point; 2]]| {
        !segments.is_empty() && segments.iter().flatten().all(|&p| contains(p))
    };
    let mut result = Vec::new();
    for &id in ids {
        let Geometry::Brep(brep) = document.object(id).unwrap().geometry() else {
            unreachable!()
        };
        if brep.faces().iter().any(|face| {
            let surface = face.surface();
            surface.degree_u() != 1
                || surface.degree_v() != 1
                || surface.control_point_count_u() != 2
                || surface.control_point_count_v() != 2
                || surface
                    .control_points()
                    .iter()
                    .any(|control| control.point().z() != 0. || control.weight() != 1.)
        }) {
            return Err(ProbeError::FixtureInvariant(
                "hole window probe requires Top-plane polygon faces",
            ));
        }
        let edges = brep
            .edges()
            .iter()
            .map(|edge| {
                let curve = edge.curve();
                if curve.degree() != 1
                    || curve
                        .control_points()
                        .iter()
                        .any(|c| c.point().z() != 0. || c.weight() != 1.)
                {
                    return Err(ProbeError::FixtureInvariant(
                        "hole window probe requires nonrational polygon edges",
                    ));
                }
                Ok(curve
                    .control_points()
                    .windows(2)
                    .map(|pair| std::array::from_fn(|i| [pair[i].point().x(), pair[i].point().y()]))
                    .collect::<Vec<_>>())
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        if !all {
            for (edge, segments) in edges.iter().enumerate() {
                if if crossing {
                    crosses(segments)
                } else {
                    enclosed(segments)
                } {
                    result.push((id, UntrimHolesComponent::Edge(edge)));
                }
            }
        } else {
            for (face, source) in brep.faces().iter().enumerate() {
                let segments = source
                    .loops()
                    .iter()
                    .flat_map(|boundary| boundary.trims())
                    .filter_map(|trim| trim.edge())
                    .flat_map(|index| edges[index].iter().copied())
                    .collect::<Vec<_>>();
                let selected = if crossing {
                    crosses(&segments)
                        || [
                            [low[0], low[1]],
                            [high[0], low[1]],
                            [low[0], high[1]],
                            [high[0], high[1]],
                        ]
                        .into_iter()
                        .any(|p| inside(p, &segments))
                } else {
                    enclosed(&segments)
                };
                if selected {
                    result.push((id, UntrimHolesComponent::Face(face)));
                }
            }
        }
    }
    Ok(result)
}

fn intersects(a: Point, b: Point, low: Point, high: Point) -> bool {
    let (mut start, mut end) = (0f64, 1f64);
    for axis in 0..2 {
        let delta = b[axis] - a[axis];
        if delta == 0. {
            if a[axis] < low[axis] || a[axis] > high[axis] {
                return false;
            }
        } else {
            let t = [
                (low[axis] - a[axis]) / delta,
                (high[axis] - a[axis]) / delta,
            ];
            start = start.max(t[0].min(t[1]));
            end = end.min(t[0].max(t[1]));
            if start > end {
                return false;
            }
        }
    }
    true
}
fn inside(p: Point, segments: &[[Point; 2]]) -> bool {
    segments
        .iter()
        .filter(|&&[a, b]| {
            (a[1] > p[1]) != (b[1] > p[1])
                && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        })
        .count()
        % 2
        == 1
}
