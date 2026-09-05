//! Endpoint matching and representation-aware assembly of mixed curve chains.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::{
    Curve3, CurveRef, GeometryError, Point3, PolyCurve3, Polyline3, Real, Tolerance, UnitVector3,
};

const MAX_JOIN_INPUTS: usize = 100_000;
const MAX_JOIN_CANDIDATES: usize = 1_000_000;
const MAX_JOIN_SCANS: usize = 16_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveJoinStyle {
    /// Batch API: majority direction and chord-length linear outputs.
    Batch,
    /// Extend the earliest source, retaining its direction and interval.
    Seeded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveJoinOptions {
    pub tolerance: Real,
    pub preserve_direction: bool,
    pub style: CurveJoinStyle,
}

#[derive(Clone, Copy)]
struct AssemblyPolicy {
    all_linear: bool,
    style: CurveJoinStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JoinedCurve3 {
    curve: Curve3,
    source_indices: Vec<usize>,
}

impl JoinedCurve3 {
    pub fn curve(&self) -> &Curve3 {
        &self.curve
    }
    pub fn into_curve(self) -> Curve3 {
        self.curve
    }
    /// Source indices in original input order, independent of traversal direction.
    pub fn source_indices(&self) -> &[usize] {
        &self.source_indices
    }
}

#[derive(Clone, Copy)]
struct Endpoint {
    curve: usize,
    start: bool,
    point: Point3,
    outward_tangent: Option<UnitVector3>,
}

#[derive(Clone, Copy)]
struct Candidate {
    distance: Real,
    tangent_dot: Real,
    left: usize,
    right: usize,
}

/// Joins nearest compatible endpoints without merging entire tolerance
/// clusters. Each endpoint can have at most one partner, so branched input
/// yields multiple traversable chains. Input geometry is never mutated.
pub fn join_curves(
    curves: &[Curve3],
    options: CurveJoinOptions,
    validation: Tolerance,
) -> Result<Vec<JoinedCurve3>, GeometryError> {
    if !options.tolerance.is_finite() || options.tolerance < 0.0 {
        return Err(GeometryError::InvalidCurveJoinTolerance);
    }
    if curves.len() > MAX_JOIN_INPUTS {
        return Err(GeometryError::CurveJoinLimit {
            resource: "input curves",
            maximum: MAX_JOIN_INPUTS,
        });
    }
    let all_linear = curves.iter().all(|curve| match curve {
        Curve3::Line(_) | Curve3::Polyline(_) => true,
        Curve3::NurbsCurve(curve) => {
            curve.degree() == 1
                && curve.knots()[1..curve.knots().len() - 1]
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
        }
        _ => false,
    });
    let mut endpoints = Vec::with_capacity(curves.len() * 2);
    let mut ends = vec![None; curves.len()];
    for (index, curve) in curves.iter().enumerate() {
        if curve.as_ref().is_closed()? {
            continue;
        }
        ends[index] = Some([endpoints.len(), endpoints.len() + 1]);
        endpoints.push(Endpoint {
            curve: index,
            start: true,
            point: curve.as_ref().start_point()?,
            outward_tangent: endpoint_tangent(curve.as_ref(), true)?,
        });
        endpoints.push(Endpoint {
            curve: index,
            start: false,
            point: curve.as_ref().end_point()?,
            outward_tangent: endpoint_tangent(curve.as_ref(), false)?,
        });
    }
    let mut candidates = find_candidates(&endpoints, options)?;
    candidates.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.tangent_dot.total_cmp(&b.tangent_dot))
            .then_with(|| (a.left, a.right).cmp(&(b.left, b.right)))
    });
    let mut partners = vec![None; endpoints.len()];
    if options.style == CurveJoinStyle::Seeded {
        partners = seeded_partners(&endpoints, &ends, &candidates)?;
    } else {
        for candidate in candidates {
            if partners[candidate.left].is_none() && partners[candidate.right].is_none() {
                partners[candidate.left] = Some(candidate.right);
                partners[candidate.right] = Some(candidate.left);
            }
        }
    }
    let mut visited = vec![false; curves.len()];
    let mut results = Vec::new();
    for first in 0..curves.len() {
        if visited[first] {
            continue;
        }
        let Some(first_ends) = ends[first] else {
            visited[first] = true;
            results.push(JoinedCurve3 {
                curve: curves[first].clone(),
                source_indices: vec![first],
            });
            continue;
        };
        // Find a free end by walking backwards from this curve, or return to
        // the seed in a cycle. No recursive traversal or endpoint averaging is
        // needed to decide the connectivity.
        let mut entered = first_ends[0];
        let mut count = 0;
        while let Some(partner) = partners[entered] {
            let opposite = ends[endpoints[partner].curve].expect("open curve has endpoints");
            entered = opposite[usize::from(endpoints[partner].start)];
            count += 1;
            if entered == first_ends[0] || count > curves.len() {
                break;
            }
        }
        let mut chain = Vec::new();
        loop {
            let endpoint = endpoints[entered];
            if visited[endpoint.curve] {
                break;
            }
            visited[endpoint.curve] = true;
            chain.push((endpoint.curve, !endpoint.start));
            let opposite = ends[endpoint.curve].expect("open curve has endpoints")
                [usize::from(endpoint.start)];
            let Some(next) = partners[opposite] else {
                break;
            };
            entered = next;
        }
        if !options.preserve_direction {
            let reverse_count = chain.iter().filter(|(_, reversed)| *reversed).count();
            let last_source_reversed = chain
                .iter()
                .max_by_key(|(source, _)| source)
                .is_some_and(|(_, reversed)| *reversed);
            let reverse = match options.style {
                CurveJoinStyle::Batch => {
                    2 * reverse_count > chain.len()
                        || (2 * reverse_count == chain.len() && last_source_reversed)
                }
                CurveJoinStyle::Seeded => chain
                    .iter()
                    .min_by_key(|(index, _)| index)
                    .is_some_and(|(_, reversed)| *reversed),
            };
            if reverse {
                chain.reverse();
                for (_, reversed) in &mut chain {
                    *reversed = !*reversed;
                }
            }
        }
        let mut sources = chain.iter().map(|(index, _)| *index).collect::<Vec<_>>();
        sources.sort_unstable();
        let output = if chain.len() == 1 {
            if all_linear {
                Curve3::Polyline(
                    linear_form(&curves[first], validation)?.try_chord_length_parameterized()?,
                )
            } else {
                curves[first].clone()
            }
        } else {
            assemble(
                curves,
                &chain,
                &ends,
                &endpoints,
                &partners,
                AssemblyPolicy {
                    all_linear,
                    style: options.style,
                },
                validation,
            )?
        };
        results.push(JoinedCurve3 {
            curve: output,
            source_indices: sources,
        });
    }
    // Newly joined chains precede unchanged inputs; retain source order within
    // each category so an unrelated earlier input does not reorder the chain.
    results.sort_by_key(|result| (result.source_indices.len() == 1, result.source_indices[0]));
    Ok(results)
}

fn assemble(
    curves: &[Curve3],
    chain: &[(usize, bool)],
    ends: &[Option<[usize; 2]>],
    endpoints: &[Endpoint],
    partners: &[Option<usize>],
    policy: AssemblyPolicy,
    validation: Tolerance,
) -> Result<Curve3, GeometryError> {
    let mut parts = Vec::with_capacity(chain.len());
    let mut linear_points = Vec::new();
    let mut linear_parameters: Vec<Real> = Vec::new();
    let seed = chain
        .iter()
        .map(|(index, _)| *index)
        .min()
        .expect("an assembly has sources");
    let mut seed_offset = 0.0;
    for &(source, reversed) in chain {
        let source_ends = ends[source].expect("assembled curves are open");
        let mut targets = [None; 2];
        for side in 0..2 {
            let endpoint = source_ends[side];
            if let Some(partner) = partners[endpoint] {
                let first = endpoints[endpoint];
                let second = endpoints[partner];
                let first_arc = endpoint_is_arc(&curves[first.curve], first.start);
                let second_arc = endpoint_is_arc(&curves[second.curve], second.start);
                let point = if first_arc && second_arc {
                    midpoint(first.point, second.point)?
                } else if first_arc {
                    first.point
                } else if second_arc {
                    second.point
                } else {
                    midpoint(first.point, second.point)?
                };
                targets[side] = Some(point);
            }
        }
        if policy.all_linear {
            let polyline = linear_form(&curves[source], validation)?;
            let mut points = polyline.vertices().to_vec();
            if let Some(point) = targets[0] {
                points[0] = point;
            }
            if let Some(point) = targets[1] {
                *points.last_mut().expect("a polyline has vertices") = point;
            }
            if reversed {
                points.reverse();
            }
            let parameters = if reversed {
                polyline
                    .parameters()
                    .iter()
                    .rev()
                    .map(|t| -t)
                    .collect::<Vec<_>>()
            } else {
                polyline.parameters().to_vec()
            };
            if linear_parameters.is_empty() {
                linear_parameters.push(parameters[0]);
            }
            if source == seed {
                seed_offset = parameters[0] - linear_parameters.last().unwrap();
            }
            for pair in parameters.windows(2) {
                linear_parameters.push(linear_parameters.last().unwrap() + (pair[1] - pair[0]));
            }
            let skip = usize::from(!linear_points.is_empty());
            linear_points.extend_from_slice(&points[skip..]);
        } else {
            let mut part = match &curves[source] {
                Curve3::Arc(arc) => {
                    Curve3::Arc(arc.try_with_endpoints(targets[0], targets[1], validation)?)
                        .to_polycurve()?
                }
                curve => curve
                    .to_polycurve()?
                    .try_with_endpoints(targets[0], targets[1])?,
            };
            if reversed {
                part = part.reversed()?;
            }
            if source == seed {
                let start = parts
                    .first()
                    .map_or(*part.domain().start(), |curve: &PolyCurve3| {
                        *curve.domain().start()
                    });
                let preceding = parts
                    .iter()
                    .map(|curve| curve.domain().end() - curve.domain().start())
                    .sum::<Real>();
                seed_offset = *part.domain().start() - (start + preceding);
            }
            parts.push(part);
        }
    }
    if policy.all_linear {
        let curve = match policy.style {
            CurveJoinStyle::Batch => {
                Polyline3::try_new(linear_points, validation)?.try_chord_length_parameterized()?
            }
            CurveJoinStyle::Seeded => Polyline3::try_with_parameters(
                linear_points,
                linear_parameters
                    .into_iter()
                    .map(|t| t + seed_offset)
                    .collect(),
                validation,
            )?,
        };
        Ok(Curve3::Polyline(curve))
    } else {
        let curve = PolyCurve3::concatenate(&parts)?;
        let curve = if policy.style == CurveJoinStyle::Seeded && seed_offset != 0.0 {
            PolyCurve3::try_with_segment_domains(
                curve.segments().to_vec(),
                curve.parameters().iter().map(|t| t + seed_offset).collect(),
            )?
        } else {
            curve
        };
        Ok(Curve3::PolyCurve(curve))
    }
}

fn endpoint_is_arc(curve: &Curve3, start: bool) -> bool {
    match curve {
        Curve3::Arc(_) => true,
        Curve3::PolyCurve(curve) => matches!(
            if start {
                curve.segments().first()
            } else {
                curve.segments().last()
            },
            Some(crate::CurveSegment3::Arc(_))
        ),
        _ => false,
    }
}

fn linear_form(curve: &Curve3, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
    match curve {
        Curve3::Line(line) => Polyline3::try_with_parameters(
            vec![line.start(), line.end()],
            vec![*line.domain().start(), *line.domain().end()],
            tolerance,
        ),
        Curve3::Polyline(line) => Ok(line.clone()),
        Curve3::NurbsCurve(curve) => Polyline3::try_with_parameters(
            curve
                .control_points()
                .iter()
                .map(|point| point.point())
                .collect(),
            curve.knots()[1..curve.knots().len() - 1].to_vec(),
            tolerance,
        ),
        _ => unreachable!("linear form is only used for line/polyline inputs"),
    }
}

fn midpoint(left: Point3, right: Point3) -> Result<Point3, GeometryError> {
    Point3::try_from(std::array::from_fn(|i| {
        let a = left.to_array()[i];
        let b = right.to_array()[i];
        let difference = b - a;
        if difference.is_finite() {
            a + 0.5 * difference
        } else {
            0.5 * a + 0.5 * b
        }
    }))
}

fn seeded_partners(
    endpoints: &[Endpoint],
    ends: &[Option<[usize; 2]>],
    candidates: &[Candidate],
) -> Result<Vec<Option<usize>>, GeometryError> {
    let mut adjacent = vec![Vec::new(); endpoints.len()];
    for (index, candidate) in candidates.iter().enumerate() {
        adjacent[candidate.left].push(index);
        adjacent[candidate.right].push(index);
    }
    let mut partners = vec![None; endpoints.len()];
    let mut assigned = vec![false; ends.len()];
    let mut scans = 0;
    for (seed, seed_ends) in ends.iter().enumerate() {
        let Some(mut free) = *seed_ends else {
            continue;
        };
        if assigned[seed] {
            continue;
        }
        assigned[seed] = true;
        let mut last_source = seed;
        loop {
            let mut best: Option<(usize, usize, usize, usize)> = None;
            for side in 0..2 {
                for &index in &adjacent[free[side]] {
                    scans += 1;
                    if scans > MAX_JOIN_SCANS {
                        return Err(GeometryError::CurveJoinLimit {
                            resource: "seeded candidate scans",
                            maximum: MAX_JOIN_SCANS,
                        });
                    }
                    let candidate = candidates[index];
                    let other = if candidate.left == free[side] {
                        candidate.right
                    } else {
                        candidate.left
                    };
                    let source = endpoints[other].curve;
                    if source <= last_source || assigned[source] {
                        continue;
                    }
                    // Candidates already have distance/tangent order. Source
                    // order takes precedence in a seeded one-pass extension.
                    let key = (source, index, side, other);
                    if best.is_none_or(|previous| key < previous) {
                        best = Some(key);
                    }
                }
            }
            let Some((source, _, side, other)) = best else {
                break;
            };
            partners[free[side]] = Some(other);
            partners[other] = Some(free[side]);
            assigned[source] = true;
            last_source = source;
            free[side] = ends[source].expect("open source")[usize::from(endpoints[other].start)];
            if adjacent[free[0]].iter().any(|&index| {
                let candidate = candidates[index];
                candidate.left == free[1] || candidate.right == free[1]
            }) {
                partners[free[0]] = Some(free[1]);
                partners[free[1]] = Some(free[0]);
                break;
            }
        }
    }
    Ok(partners)
}

fn find_candidates(
    endpoints: &[Endpoint],
    options: CurveJoinOptions,
) -> Result<Vec<Candidate>, GeometryError> {
    let mut candidates = Vec::new();
    let mut scans = 0;
    let mut consider = |left: usize, right: usize| -> Result<(), GeometryError> {
        scans += 1;
        if scans > MAX_JOIN_SCANS {
            return Err(GeometryError::CurveJoinLimit {
                resource: "endpoint comparisons",
                maximum: MAX_JOIN_SCANS,
            });
        }
        let a = endpoints[left];
        let b = endpoints[right];
        if a.curve == b.curve || (options.preserve_direction && a.start == b.start) {
            return Ok(());
        }
        let delta = [
            a.point.x() - b.point.x(),
            a.point.y() - b.point.y(),
            a.point.z() - b.point.z(),
        ];
        let distance = delta[0].hypot(delta[1]).hypot(delta[2]);
        if distance <= options.tolerance {
            if candidates.len() == MAX_JOIN_CANDIDATES {
                return Err(GeometryError::CurveJoinLimit {
                    resource: "endpoint candidates",
                    maximum: MAX_JOIN_CANDIDATES,
                });
            }
            let tangent_dot = match (a.outward_tangent, b.outward_tangent) {
                (Some(a), Some(b)) => a.as_vector().dot(b.as_vector())?,
                _ => 1.0,
            };
            candidates.push(Candidate {
                distance,
                tangent_dot,
                left: left.min(right),
                right: left.max(right),
            });
        }
        Ok(())
    };
    if options.tolerance == 0.0 {
        let mut exact = HashMap::<[u64; 3], Vec<usize>>::new();
        for (index, endpoint) in endpoints.iter().enumerate() {
            let key = endpoint
                .point
                .to_array()
                .map(|value| if value == 0.0 { 0 } else { value.to_bits() });
            let bucket = exact.entry(key).or_default();
            for &other in bucket.iter() {
                consider(other, index)?;
            }
            bucket.push(index);
        }
        return Ok(candidates);
    }
    let origin = endpoints
        .first()
        .map(|endpoint| endpoint.point.to_array())
        .unwrap_or([0.0; 3]);
    let cells = endpoints
        .iter()
        .map(|endpoint| {
            let mut cell = [0_i64; 3];
            for axis in 0..3 {
                let value =
                    ((endpoint.point.to_array()[axis] - origin[axis]) / options.tolerance).floor();
                if !value.is_finite() || value <= i64::MIN as Real || value >= i64::MAX as Real {
                    return None;
                }
                cell[axis] = value as i64;
            }
            Some(cell)
        })
        .collect::<Option<Vec<_>>>();
    if let Some(cells) = cells {
        let mut grid = HashMap::<[i64; 3], Vec<usize>>::new();
        for (index, cell) in cells.into_iter().enumerate() {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(x) = cell[0].checked_add(dx) else {
                            continue;
                        };
                        let Some(y) = cell[1].checked_add(dy) else {
                            continue;
                        };
                        let Some(z) = cell[2].checked_add(dz) else {
                            continue;
                        };
                        if let Some(nearby) = grid.get(&[x, y, z]) {
                            for &other in nearby {
                                consider(other, index)?;
                            }
                        }
                    }
                }
            }
            grid.entry(cell).or_default().push(index);
        }
    } else {
        // Extreme coordinates use a sweep along the widest axis, avoiding
        // all-pairs work for data lying in a plane perpendicular to x.
        let mut minimum = [Real::INFINITY; 3];
        let mut maximum = [Real::NEG_INFINITY; 3];
        for endpoint in endpoints {
            for axis in 0..3 {
                let value = endpoint.point.to_array()[axis];
                minimum[axis] = minimum[axis].min(value);
                maximum[axis] = maximum[axis].max(value);
            }
        }
        let axis = (0..3)
            .max_by(|&a, &b| (maximum[a] - minimum[a]).total_cmp(&(maximum[b] - minimum[b])))
            .unwrap();
        let mut sorted = (0..endpoints.len()).collect::<Vec<_>>();
        sorted.sort_by(|&a, &b| {
            endpoints[a].point.to_array()[axis].total_cmp(&endpoints[b].point.to_array()[axis])
        });
        for (position, &right) in sorted.iter().enumerate() {
            for &left in sorted[..position].iter().rev() {
                if endpoints[right].point.to_array()[axis] - endpoints[left].point.to_array()[axis]
                    > options.tolerance
                {
                    break;
                }
                consider(left, right)?;
            }
        }
    }
    Ok(candidates)
}

fn endpoint_tangent(
    curve: CurveRef<'_>,
    start: bool,
) -> Result<Option<UnitVector3>, GeometryError> {
    let domain = curve.domain();
    match curve.evaluate_with_tangent(if start {
        *domain.start()
    } else {
        *domain.end()
    }) {
        Ok(sample) => Ok(Some(if start {
            sample.tangent().opposite()
        } else {
            sample.tangent()
        })),
        // A stationary endpoint remains eligible for joining; only its tangent
        // tie-break is unavailable. Do not hide numerical/evaluation failures.
        Err(GeometryError::Degenerate { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}
