//! Trim the miter rail to the boundary of the inward offset regions.
use super::*;

type Point2 = [Real; 2];

struct Rail {
    segment: LineSegment,
    start: Point2,
    direction: Point2,
    length: Real,
    min: Point2,
    max: Point2,
    forward: bool,
}

impl Rail {
    fn between(&self, fraction: Real) -> Point2 {
        std::array::from_fn(|axis| self.direction[axis].mul_add(fraction, self.start[axis]))
    }
}

pub(super) struct Span {
    pub index: usize,
    pub range: [Real; 2],
    pub points: [Point3; 2],
    ends: [Point2; 2],
}

pub(super) struct Trimmed {
    pub spans: Vec<Span>,
    pub single_loop: bool,
}

pub(super) fn trim(
    source: &Polyline3,
    layout: &OffsetPolylineLayout,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Option<Trimmed>, GeometryError> {
    // Dropping the dominant normal component keeps the plane well
    // conditioned and preserves exactly axis-aligned contacts.
    let normal = layout.normal.as_vector().to_array();
    let axis = (0..3)
        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
        .unwrap();
    let origin = source.vertices()[0];
    let project = |p| {
        let delta = origin.vector_to(p)?.to_array();
        Ok::<_, GeometryError>([
            delta[(axis + 1) % 3],
            delta[(axis + 2) % 3] * normal[axis].signum(),
        ])
    };
    let projected = layout
        .corners
        .iter()
        .map(|corner| project(corner.0))
        .collect::<Result<Vec<_>, _>>()?;
    let scale = projected
        .iter()
        .flatten()
        .fold(1.0_f64, |max, x| max.max(x.abs()));
    let epsilon = (tolerance.absolute() / scale).max(64. * Real::EPSILON);
    let count = projected.len();
    let rails = source
        .segments()
        .enumerate()
        .map(|(i, original)| {
            let next = (i + 1) % count;
            let start = projected[i].map(|x| x / scale);
            let end = projected[next].map(|x| x / scale);
            let direction = sub(end, start);
            let [a, b] = layout.segment_points(i);
            Ok::<_, GeometryError>(Rail {
                segment: LineSegment::from_validated(a, b, [0., 1.]),
                start,
                direction,
                length: direction[0].hypot(direction[1]),
                min: std::array::from_fn(|axis| start[axis].min(end[axis])),
                max: std::array::from_fn(|axis| start[axis].max(end[axis])),
                forward: layout.segment_advances(i, original)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut order = (0..count).collect::<Vec<_>>();
    order.sort_unstable_by(|&a, &b| rails[a].min[0].total_cmp(&rails[b].min[0]));
    let mut cuts = vec![vec![0., 1.]; count];
    let mut crossing = rails.iter().any(|rail| !rail.forward);
    let mut contact_count = 0;
    for (position, &i) in order.iter().enumerate() {
        let a = &rails[i];
        for &j in &order[position + 1..] {
            let b = &rails[j];
            if b.min[0] > a.max[0] + epsilon {
                break;
            }
            if a.min[1] > b.max[1] + epsilon || b.min[1] > a.max[1] + epsilon {
                continue;
            }
            for [u, v] in contacts(a, b, epsilon).into_iter().flatten() {
                let adjacent = i.abs_diff(j) == 1 || i.abs_diff(j) + 1 == count;
                if adjacent && (u == 0. || u == 1.) && (v == 0. || v == 1.) {
                    continue;
                }
                crossing = true;
                cuts[i].push(u);
                cuts[j].push(v);
                contact_count += 1;
                if contact_count > 4 * MAX_PATCHES {
                    return Err(GeometryError::RibbonResourceLimit {
                        maximum: MAX_PATCHES,
                    });
                }
            }
        }
    }
    if !crossing {
        return Ok(None);
    }
    let inward = match Curve3::Polyline(source.clone())
        .offset_region_inward_sign(layout.normal, tolerance)
    {
        Ok(Some(sign)) => sign,
        Ok(None)
        | Err(
            GeometryError::SelfIntersectingOffsetRegion | GeometryError::DegenerateOffsetRegion,
        ) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if distance.signum() != inward {
        return Ok(None);
    }

    let mut spans = Vec::<Span>::new();
    for (i, (rail, mut cuts)) in rails.iter().zip(cuts).enumerate() {
        if !rail.forward || rail.length <= epsilon {
            continue;
        }
        cuts.sort_unstable_by(Real::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() * rail.length <= epsilon);
        let axis = rail.direction.map(|x| x / rail.length);
        for pair in cuts.windows(2) {
            let [lo, hi] = [pair[0], pair[1]];
            let mid = rail.between(lo + (hi - lo) * 0.5);
            if !on_boundary(i, &rails, mid, axis, inward, epsilon) {
                continue;
            }
            let end = rail.segment.point_at(hi)?;
            if let Some(last) = spans
                .last_mut()
                .filter(|last| last.index == i && last.range[1] == lo)
            {
                last.range[1] = hi;
                last.points[1] = end;
                last.ends[1] = rail.between(hi);
            } else {
                spans.push(Span {
                    index: i,
                    range: [lo, hi],
                    points: [rail.segment.point_at(lo)?, end],
                    ends: [rail.between(lo), rail.between(hi)],
                });
            }
        }
    }
    if spans.len() > MAX_PATCHES {
        return Err(GeometryError::RibbonResourceLimit {
            maximum: MAX_PATCHES,
        });
    }
    let single_loop = single_loop(&spans, epsilon)?;
    Ok(Some(Trimmed { spans, single_loop }))
}

fn sub(a: Point2, b: Point2) -> Point2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: Point2, b: Point2) -> Real {
    a[0].mul_add(b[0], a[1] * b[1])
}

fn cross(a: Point2, b: Point2) -> Real {
    a[0].mul_add(b[1], -(a[1] * b[0]))
}

fn fraction(value: Real, epsilon: Real) -> Option<Real> {
    if value < -epsilon || value > 1. + epsilon {
        None
    } else if value <= epsilon {
        Some(0.)
    } else if value >= 1. - epsilon {
        Some(1.)
    } else {
        Some(value)
    }
}

fn contacts(a: &Rail, b: &Rail, epsilon: Real) -> [Option<[Real; 2]>; 2] {
    if a.length <= epsilon || b.length <= epsilon {
        return [None, None];
    }
    let u = a.direction.map(|x| x / a.length);
    let v = b.direction.map(|x| x / b.length);
    let delta = sub(b.start, a.start);
    let determinant = cross(u, v);
    let a_epsilon = epsilon / a.length;
    let b_epsilon = epsilon / b.length;
    if determinant.abs() > 64. * Real::EPSILON {
        let on_a = cross(delta, v) / (determinant * a.length);
        let on_b = cross(delta, u) / (determinant * b.length);
        return [
            fraction(on_a, a_epsilon)
                .zip(fraction(on_b, b_epsilon))
                .map(|(a, b)| [a, b]),
            None,
        ];
    }
    if cross(u, delta).abs() > epsilon {
        return [None, None];
    }
    let ends = [0., 1.].map(|t| dot(sub(b.between(t), a.start), u) / a.length);
    let lo = ends[0].min(ends[1]).max(0.);
    let hi = ends[0].max(ends[1]).min(1.);
    if hi < lo - a_epsilon {
        return [None, None];
    }
    [lo, hi].map(|t| {
        let on_a = fraction(t, a_epsilon)?;
        let on_b = dot(sub(a.between(on_a), b.start), v) / b.length;
        Some([on_a, fraction(on_b, b_epsilon)?])
    })
}

fn on_boundary(
    i: usize,
    rails: &[Rail],
    point: Point2,
    axis: Point2,
    inward: Real,
    epsilon: Real,
) -> bool {
    let mut left = 0_i32;
    let mut jump = 0_i32;
    for (j, rail) in rails.iter().enumerate() {
        if rail.length <= epsilon {
            continue;
        }
        let ends = [rail.start, rail.between(1.)].map(|p| sub(p, point));
        let [a, b] = ends.map(|p| dot(axis, p));
        let [ya, yb] = ends.map(|p| cross(axis, p));
        if ya.abs() <= epsilon && yb.abs() <= epsilon {
            if a.min(b) <= 0. && a.max(b) >= 0. {
                let forward = a < b;
                if j < i && forward && rail.forward {
                    // Coincident rails retain the first source interval.
                    return false;
                }
                jump += if forward { 1 } else { -1 };
            }
        } else if (a > 0.) != (b > 0.) && (yb - ya).mul_add(-a / (b - a), ya) > epsilon {
            // Crossings of the ray from the left side give its winding.
            left += if a > b { 1 } else { -1 };
        }
    }
    let right = left - jump;
    if inward > 0. {
        left > 0 && right <= 0
    } else {
        right < 0 && left >= 0
    }
}

fn single_loop(spans: &[Span], epsilon: Real) -> Result<bool, GeometryError> {
    let mut nodes: [Vec<_>; 2] = [0, 1].map(|end| {
        spans
            .iter()
            .enumerate()
            .map(|(i, span)| (span.ends[end], i))
            .collect()
    });
    for nodes in &mut nodes {
        nodes.sort_unstable_by(|a, b| a.0[0].total_cmp(&b.0[0]));
    }
    let mut successors = vec![None; spans.len()];
    for (set, nodes_at_end) in nodes.iter().enumerate() {
        for &(point, index) in nodes_at_end {
            let neighbors = nodes.each_ref().map(|nodes| {
                let start = nodes.partition_point(|(a, _)| a[0] < point[0] - epsilon);
                let matches = nodes[start..]
                    .iter()
                    .take_while(|(a, _)| a[0] <= point[0] + epsilon)
                    .filter(|(a, _)| (a[1] - point[1]).abs() <= epsilon);
                matches.fold((0, None), |(count, _), &(_, i)| (count + 1, Some(i)))
            });
            if neighbors[0].0 != neighbors[1].0 {
                return Err(GeometryError::Degenerate {
                    context: "inconsistent trimmed ribbon rails",
                });
            }
            if set == 1 && neighbors[0].0 == 1 {
                successors[index] = neighbors[0].1;
            }
        }
    }
    let mut next = 0;
    for step in 1..=spans.len() {
        let Some(index) = successors[next] else {
            return Ok(false);
        };
        if index == 0 {
            return Ok(step == spans.len());
        }
        next = index;
    }
    Ok(false)
}
