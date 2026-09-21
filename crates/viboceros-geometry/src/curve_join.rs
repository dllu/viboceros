//! Endpoint matching and representation-aware assembly of mixed curve chains.

mod assembly;
#[cfg(test)]
mod tests;

use assembly::{assemble, endpoint_is_linear, is_linear, linear_form};

use std::collections::HashMap;

use crate::{
    Curve3, CurveRef, GeometryError, Point3, PolyCurve3, Polyline3, Real, Tolerance, UnitVector3,
};

const MAX_JOIN_INPUTS: usize = 100_000;
const MAX_JOIN_CANDIDATES: usize = 1_000_000;
const MAX_JOIN_SCANS: usize = 16_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveJoinStyle {
    /// Batch API: majority direction; wholly linear batches use chord lengths.
    Batch,
    /// Extend only the first open source in one pass, retaining its direction.
    /// Native Join assembly retains seed intervals except when mixed linear
    /// runs are consolidated. Unconnected sources remain singleton components.
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
    linear_batch: bool,
    style: CurveJoinStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JoinedCurve3 {
    curve: Curve3,
    source_indices: Vec<usize>,
    seed_start_parameter: Option<Real>,
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
    /// Start of the earliest source in a newly assembled seeded result.
    /// Rebuilt mixed composites can move it away from the source's old domain.
    /// Batch results and unchanged singletons do not expose this mapping.
    pub fn seed_start_parameter(&self) -> Option<Real> {
        self.seed_start_parameter
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

struct SeededConnections {
    partners: Vec<Option<usize>>,
    closing_edge: Option<[usize; 2]>,
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
    let linear_batch = options.style == CurveJoinStyle::Batch && curves.iter().all(is_linear);
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
    let mut closing_edge = None;
    if options.style == CurveJoinStyle::Seeded {
        let seeded = seeded_partners(curves, &endpoints, &ends, &candidates)?;
        partners = seeded.partners;
        closing_edge = seeded.closing_edge;
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
                seed_start_parameter: None,
            });
            continue;
        };
        // Find a free end by walking backwards from this curve, or return to
        // the seed in a cycle. No recursive traversal or endpoint averaging is
        // needed to decide the connectivity.
        let mut entered = first_ends[0];
        let mut count = 0;
        let mut batch_cut = (0, 0, 0, entered);
        while let Some(partner) = partners[entered] {
            if options.style == CurveJoinStyle::Seeded
                && closing_edge.is_some_and(|edge| edge.contains(&entered))
            {
                break;
            }
            let a = endpoints[entered].curve;
            let b = endpoints[partner].curve;
            let tie = if linear_batch {
                entered
            } else {
                endpoints.len() - entered
            };
            let key = (a.max(b), a.min(b), tie, entered);
            if key > batch_cut {
                batch_cut = key;
            }
            let opposite = ends[endpoints[partner].curve].expect("open curve has endpoints");
            entered = opposite[usize::from(endpoints[partner].start)];
            count += 1;
            if entered == first_ends[0] {
                // Accepted endpoint pairs are disjoint. Their source-order
                // final connection is the batch loop's original seam.
                entered = batch_cut.3;
                break;
            }
            if count > curves.len() {
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
                        || (linear_batch
                            && 2 * reverse_count == chain.len()
                            && last_source_reversed)
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
        // Representation is a property of this chain, not unrelated inputs.
        let all_linear = chain.iter().all(|&(index, _)| is_linear(&curves[index]));
        let (output, seed_start_parameter) = if chain.len() == 1 {
            let curve = if all_linear {
                Curve3::Polyline(
                    linear_form(&curves[first], validation)?.try_chord_length_parameterized()?,
                )
            } else {
                curves[first].clone()
            };
            (curve, None)
        } else {
            assemble(
                curves,
                &chain,
                &ends,
                &endpoints,
                &partners,
                AssemblyPolicy {
                    all_linear,
                    linear_batch,
                    style: options.style,
                },
                validation,
            )?
        };
        results.push(JoinedCurve3 {
            curve: output,
            source_indices: sources,
            seed_start_parameter,
        });
    }
    // Newly joined chains precede unchanged inputs; retain source order within
    // each category so an unrelated earlier input does not reorder the chain.
    results.sort_by_key(|result| (result.source_indices.len() == 1, result.source_indices[0]));
    Ok(results)
}

fn seeded_partners(
    curves: &[Curve3],
    endpoints: &[Endpoint],
    ends: &[Option<[usize; 2]>],
    candidates: &[Candidate],
) -> Result<SeededConnections, GeometryError> {
    let mut adjacent = vec![Vec::new(); endpoints.len()];
    for (index, candidate) in candidates.iter().enumerate() {
        adjacent[candidate.left].push(index);
        adjacent[candidate.right].push(index);
    }
    let mut partners = vec![None; endpoints.len()];
    let mut assigned = vec![false; ends.len()];
    let mut scans = 0;
    let mut closing_edge = None;
    if let Some((seed, Some(mut free))) = ends
        .iter()
        .copied()
        .enumerate()
        .find(|(_, ends)| ends.is_some())
    {
        assigned[seed] = true;
        let mut last_source = seed;
        let mut linear_vertices = assembly::linear_vertex_count(curves[seed].as_ref());
        loop {
            let mut sides: [Option<(usize, usize, usize, usize)>; 2] = [None, None];
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
                    if sides[side].is_none_or(|previous| key < previous) {
                        sides[side] = Some(key);
                    }
                }
            }
            let mut best = sides.into_iter().flatten().min();
            if let [Some(left), Some(right)] = sides
                && left.0 == right.0
                && left.3 != right.3
            {
                // A curve closing both free ends is prepended by individual
                // Join picking. Copy commands may restore the seed seam later.
                best = Some(
                    if (is_linear(&curves[left.0])
                        && !endpoint_is_linear(
                            &curves[endpoints[free[0]].curve],
                            endpoints[free[0]].start,
                        )
                        && endpoint_is_linear(
                            &curves[endpoints[free[1]].curve],
                            endpoints[free[1]].start,
                        ))
                        || (matches!(curves[left.0], Curve3::Arc(_))
                            && linear_vertices.is_some_and(|n| n > 2))
                    {
                        right
                    } else {
                        left
                    },
                );
            }
            let Some((source, _, side, other)) = best else {
                break;
            };
            partners[free[side]] = Some(other);
            partners[other] = Some(free[side]);
            assigned[source] = true;
            linear_vertices = linear_vertices
                .zip(assembly::linear_vertex_count(curves[source].as_ref()))
                .and_then(|(a, b)| a.checked_add(b - 1));
            last_source = source;
            free[side] = ends[source].expect("open source")[usize::from(endpoints[other].start)];
            if adjacent[free[0]].iter().any(|&index| {
                let candidate = candidates[index];
                candidate.left == free[1] || candidate.right == free[1]
            }) {
                partners[free[0]] = Some(free[1]);
                partners[free[1]] = Some(free[0]);
                closing_edge = Some(free);
                break;
            }
        }
    }
    Ok(SeededConnections {
        partners,
        closing_edge,
    })
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
