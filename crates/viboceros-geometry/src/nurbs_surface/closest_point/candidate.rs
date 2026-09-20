//! Surface candidates share the kernel's immutable point-distance keys.
use super::*;
use crate::point::PointDistance;
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct Candidate {
    pub(super) parameters: (Real, Real),
    distance: PointDistance,
}

impl Candidate {
    pub(super) fn new(target: Point3, point: Point3, parameters: (Real, Real)) -> Self {
        Self {
            parameters,
            distance: PointDistance::new(target, point),
        }
    }

    pub(super) fn point(&self) -> Point3 {
        self.distance.point()
    }

    pub(super) fn evaluate(
        query: &mut SurfaceQuery<'_>,
        target: Point3,
        parameters: (Real, Real),
    ) -> Result<Self, GeometryError> {
        Ok(Self::new(
            target,
            query.evaluate(parameters.0, parameters.1)?,
            parameters,
        ))
    }

    pub(super) fn compare(&self, other: &Self, target: Point3) -> Ordering {
        self.distance.compare(&other.distance, target)
    }
}

/// Retain the same sixteen starts as a full exact stable grid sort, without
/// sorting every discarded cell. Grid parameters are unique and ascending in
/// each axis, so V then U explicitly reproduces their original tie order.
pub(super) fn retain_closest_seeds(seeds: &mut Vec<Candidate>, target: Point3) {
    let order = |a: &Candidate, b: &Candidate| {
        a.compare(b, target)
            .then_with(|| a.parameters.1.total_cmp(&b.parameters.1))
            .then_with(|| a.parameters.0.total_cmp(&b.parameters.0))
    };
    const STARTS: usize = 16;
    if seeds.len() > STARTS {
        seeds.select_nth_unstable_by(STARTS, order);
        seeds.truncate(STARTS);
    }
    seeds.sort_unstable_by(order);
}
