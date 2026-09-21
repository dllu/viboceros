//! Endpoint matching and representation-aware assembly of mixed curve chains.

mod assembly;
mod search;
mod seeded;
#[cfg(test)]
mod tests;

use assembly::{assemble, endpoint_is_linear, is_linear, linear_form};
use search::find_candidates;

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

impl Candidate {
    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.tangent_dot.total_cmp(&other.tangent_dot))
            .then_with(|| (self.left, self.right).cmp(&(other.left, other.right)))
    }
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
    let (partners, closing_edge) = if options.style == CurveJoinStyle::Seeded {
        let seeded = seeded::connect(curves, &endpoints, &ends, options)?;
        (seeded.partners, seeded.closing_edge)
    } else {
        let mut candidates = find_candidates(&endpoints, options)?;
        candidates.sort_by(Candidate::compare);
        let mut partners = vec![None; endpoints.len()];
        for candidate in candidates {
            if partners[candidate.left].is_none() && partners[candidate.right].is_none() {
                partners[candidate.left] = Some(candidate.right);
                partners[candidate.right] = Some(candidate.left);
            }
        }
        (partners, None)
    };
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
