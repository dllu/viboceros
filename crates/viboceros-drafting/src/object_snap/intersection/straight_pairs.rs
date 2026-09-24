//! Projected straight-wire crossings. Keep each pair's exact 3D point while
//! grouping coincident screen crossings to rank three or more source curves.
use super::{Segment, SourceChoice, crossing, prefer_first};
use crate::object_snap::{SnapMetric, projected_line};
use std::collections::{HashMap, HashSet};
use viboceros_document::ObjectId;
use viboceros_geometry::{Point3, Real};

#[derive(Clone, Copy)]
struct Crossing {
    first: usize,
    second: usize,
    image: [Real; 2],
    distance: Real,
    group: usize,
}

struct Group {
    image: [Real; 2],
    segments: Vec<usize>,
    segment_set: HashSet<usize>,
    crossings: Vec<usize>,
}

pub(super) fn visit(
    segments: &[Segment],
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let mut crossings = Vec::new();
    let mut groups: Vec<Group> = Vec::new();
    let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let cluster_radius = (metric.capture_radius() * 1e-9).max(Real::MIN_POSITIVE);
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            let a = segments[first];
            let b = segments[second];
            // Rhino captures a polyline's own corners, but not shared
            // vertices of wires belonging to one mesh.
            if a.owner == b.owner && (a.mesh || b.mesh) {
                continue;
            }
            let Some(image) = crossing(a.image_a, a.image_b, b.image_a, b.image_b) else {
                continue;
            };
            let Some(distance) = metric.captured_offset_distance(image) else {
                continue;
            };
            let cell = (
                (image[0] / cluster_radius).floor() as i64,
                (image[1] / cluster_radius).floor() as i64,
            );
            let mut matching = None;
            'nearby: for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(indices) = cells.get(&(cell.0 + dx, cell.1 + dy)) {
                        for &index in indices {
                            let target = groups[index].image;
                            if (target[0] - image[0])
                                .abs()
                                .max((target[1] - image[1]).abs())
                                <= cluster_radius
                            {
                                matching = Some(index);
                                break 'nearby;
                            }
                        }
                    }
                }
            }
            let group = matching.unwrap_or_else(|| {
                let index = groups.len();
                groups.push(Group {
                    image,
                    segments: Vec::new(),
                    segment_set: HashSet::new(),
                    crossings: Vec::new(),
                });
                cells.entry(cell).or_default().push(index);
                index
            });
            let event = crossings.len();
            crossings.push(Crossing {
                first,
                second,
                image,
                distance,
                group,
            });
            groups[group].crossings.push(event);
            for index in [first, second] {
                if groups[group].segment_set.insert(index) {
                    groups[group].segments.push(index);
                }
            }
        }
    }
    let multi_groups: Vec<_> = groups
        .iter()
        .map(|group| {
            let owners: HashSet<_> = group
                .segments
                .iter()
                .map(|&index| segments[index].owner)
                .collect();
            owners.len() >= 3 && group.segments.iter().all(|&index| !segments[index].mesh)
        })
        .collect();
    let mut handled = HashSet::new();
    for event in crossings.iter().copied() {
        let group = &groups[event.group];
        if multi_groups[event.group] {
            if handled.insert(event.group) {
                emit_multi(group, segments, &crossings, metric, emit);
            }
        } else {
            emit_pair(event, segments, metric, emit);
        }
    }
}

fn emit_multi(
    group: &Group,
    segments: &[Segment],
    crossings: &[Crossing],
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let Some(&selected) = group.segments.iter().min_by(|&&a, &&b| {
        segments[a]
            .hover_distance
            .total_cmp(&segments[b].hover_distance)
            .then(segments[a].order.cmp(&segments[b].order))
            .then(a.cmp(&b))
    }) else {
        return;
    };
    let segment = segments[selected];
    let Some(event) = group
        .crossings
        .iter()
        .map(|&index| crossings[index])
        .find(|event| event.first == selected || event.second == selected)
    else {
        return;
    };
    if let Some(point) = projected_line::point_at_image(segment.a, segment.b, event.image, metric) {
        emit(segment.owner, point, event.distance);
    }
}

fn emit_pair(
    event: Crossing,
    segments: &[Segment],
    metric: &impl SnapMetric,
    emit: &mut impl FnMut(ObjectId, Point3, Real),
) {
    let (a, b) = (segments[event.first], segments[event.second]);
    let (Some(point_a), Some(point_b)) = (
        projected_line::point_at_image(a.a, a.b, event.image, metric),
        projected_line::point_at_image(b.a, b.b, event.image, metric),
    ) else {
        return;
    };
    let prefer_a = prefer_first(
        SourceChoice {
            hover_distance: a.hover_distance,
            mesh: a.mesh,
            curve_priority: 0,
            order: a.order,
            point: point_a,
        },
        SourceChoice {
            hover_distance: b.hover_distance,
            mesh: b.mesh,
            curve_priority: 0,
            order: b.order,
            point: point_b,
        },
        metric,
    );
    let (owner, point) = if prefer_a {
        (a.owner, point_a)
    } else {
        (b.owner, point_b)
    };
    emit(owner, point, event.distance);
}
