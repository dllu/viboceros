//! Exact tensor hulls bound the complete surface; numeric queries only seed witnesses.
use super::*;
use crate::SurfaceMinimumDistance;
use std::{cmp::Ordering, collections::BinaryHeap};

mod patch;
use patch::Patch;

type P = [Rational; 3];
const MAX_CONTROLS: usize = 4096;
const MAX_PENDING: usize = 65_536;

impl NurbsSurface {
    /// Bound the minimum distance from a point to the complete active tensor
    /// image, including its boundaries. Exact original-source evaluation and
    /// rational Bernstein hulls certify the bracket; numerical closest-point
    /// queries only propose witnesses. The source is never edited.
    ///
    /// Supports degrees <=16, <=4096 controls, common-sign weights and interior
    /// knot multiplicity <=degree. Absolute tolerance bounds distance error,
    /// not UV error or uniqueness. This covers the untrimmed tensor surface.
    /// `None` means unsupported or unproved within the shared arithmetic/work
    /// limits, subdivision depth48, 65536 pending patches, or native UV precision.
    pub fn minimum_distance_to_point(
        &self,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<Option<SurfaceMinimumDistance>, GeometryError> {
        self.point_minimum(point, tolerance, None)
    }

    /// Same certificate with independently proposed native UV parameters,
    /// for example from a native kernel. The seed is exactly reevaluated and
    /// never replaces the global lower bound.
    pub fn minimum_distance_to_point_seeded(
        &self,
        point: Point3,
        tolerance: Tolerance,
        parameters: [Real; 2],
    ) -> Result<Option<SurfaceMinimumDistance>, GeometryError> {
        for (t, d) in parameters
            .into_iter()
            .zip([self.domain_u(), self.domain_v()])
        {
            if !t.is_finite() || !d.contains(&t) {
                return Err(GeometryError::ParameterOutOfDomain {
                    parameter: t,
                    domain_start: *d.start(),
                    domain_end: *d.end(),
                });
            }
        }
        self.point_minimum(point, tolerance, Some(parameters))
    }

    fn point_minimum(
        &self,
        point: Point3,
        tolerance: Tolerance,
        seed: Option<[Real; 2]>,
    ) -> Result<Option<SurfaceMinimumDistance>, GeometryError> {
        if self.degree_u() > MAX_DEGREE
            || self.degree_v() > MAX_DEGREE
            || self.control_points().len() > MAX_CONTROLS
        {
            return Ok(None);
        }
        match minimum(self, point, tolerance, seed) {
            Err(GeometryError::SurfaceCurveCertificateWorkLimit) => Ok(None),
            result => result,
        }
    }
}

struct Best {
    result: SurfaceMinimumDistance,
    square: Rational,
    direction: P,
}

struct Search {
    surface: surface::Surface,
    target: P,
    best: Option<Best>,
    budget: Budget,
    epsilon: Real,
}

impl Search {
    fn candidate(&mut self, parameters: [Real; 2]) -> Result<(), GeometryError> {
        if parameters.iter().any(|t| !t.is_finite()) {
            return Ok(());
        }
        let uv: Uv = [
            rational(parameters[0]),
            rational(parameters[1]),
            Rational::one(),
        ];
        let bounds = [0, 1].map(|i| [uv[i].clone(), uv[i].clone()]);
        let Some(span) = self.surface.containing_patch(&bounds, &mut self.budget)? else {
            return Ok(());
        };
        let image = self.surface.point_value(span, uv, &mut self.budget)?;
        let p: P = std::array::from_fn(|i| &image[i] / &image[3]);
        let direction: P = std::array::from_fn(|i| &p[i] - &self.target[i]);
        for value in p.iter().chain(&direction) {
            self.budget.check(value)?;
        }
        let square: Rational = direction.iter().map(|x| x * x).sum();
        self.budget.check(&square)?;
        if self.best.as_ref().is_some_and(|b| square >= b.square) {
            return Ok(());
        }
        let h = [
            direction[0].clone(),
            direction[1].clone(),
            direction[2].clone(),
            Rational::one(),
        ];
        let Some(upper_distance) = algebra::norm_bound(&h, Real::MAX) else {
            return Ok(());
        };
        self.best = Some(Best {
            result: SurfaceMinimumDistance {
                parameters,
                point: Point3::try_new(scalar(&p[0])?, scalar(&p[1])?, scalar(&p[2])?)?,
                upper_distance,
                maximum_error: self.epsilon,
            },
            square,
            direction,
        });
        Ok(())
    }
    fn finished(&mut self, lower: &Rational) -> Result<bool, GeometryError> {
        let Some(best) = &self.best else {
            return Ok(false);
        };
        // Regions containing the current witness may already be pruned.
        // Its exact distance still caps the global lower bound, including
        // when epsilon is smaller than the witness upper bound's rounding.
        algebra::within_gap(
            best.result.upper_distance,
            &lower.clone().min(best.square.clone()),
            self.epsilon,
            &mut self.budget,
        )
    }
}

struct Pending {
    lower: Rational,
    sequence: usize,
    patch: Patch,
}
impl PartialEq for Pending {
    fn eq(&self, b: &Self) -> bool {
        self.sequence == b.sequence && self.lower == b.lower
    }
}
impl Eq for Pending {}
impl Ord for Pending {
    fn cmp(&self, b: &Self) -> Ordering {
        b.lower
            .cmp(&self.lower)
            .then_with(|| b.sequence.cmp(&self.sequence))
    }
}
impl PartialOrd for Pending {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}

fn minimum(
    source: &NurbsSurface,
    target: Point3,
    tolerance: Tolerance,
    seed: Option<[Real; 2]>,
) -> Result<Option<SurfaceMinimumDistance>, GeometryError> {
    let mut budget = Budget(MAX_WORK);
    let Some(surface) = surface::Surface::new(source, &mut budget)? else {
        return Ok(None);
    };
    let mut search = Search {
        surface,
        target: target.to_array().map(rational),
        best: None,
        budget,
        epsilon: tolerance.absolute(),
    };
    let proposal = seed.or_else(|| {
        source
            .closest_parameters(target, tolerance)
            .ok()
            .map(|(u, v)| [u, v])
    });
    if let Some(uv) = proposal {
        search.candidate(uv)?;
    }
    if search.finished(&Rational::zero())? {
        return Ok(search.best.map(|b| b.result));
    }
    let degree = [source.degree_u(), source.degree_v()];
    let mut queue = BinaryHeap::new();
    let mut sequence = 0;
    for (domain, net) in search.surface.distance_patches(&mut search.budget)? {
        let patch = Patch::new(net, degree, domain, 0, &mut search.budget)?;
        let lower = patch.lower(&search.target, None, &mut search.budget)?;
        queue.push(Pending {
            patch,
            lower,
            sequence,
        });
        sequence += 1;
    }
    while let Some(mut pending) = queue.pop() {
        search.budget.charge(1)?;
        if search.finished(&pending.lower)? {
            return Ok(search.best.map(|b| b.result));
        }
        if let Some(best) = &search.best {
            let lower =
                pending
                    .patch
                    .lower(&search.target, Some(&best.direction), &mut search.budget)?;
            if lower >= best.square {
                continue;
            }
            if lower > pending.lower {
                pending.lower = lower;
                queue.push(pending);
                continue;
            }
        }
        for fractions in [[0., 0.], [0., 1.], [1., 0.], [1., 1.], [0.5, 0.5]] {
            let mut uv = [0.; 2];
            for axis in 0..2 {
                let d = &pending.patch.domain[axis];
                uv[axis] = scalar(&(&d[0] + rational(fractions[axis]) * (&d[1] - &d[0])))?;
            }
            search.candidate(uv)?;
        }
        if search
            .best
            .as_ref()
            .is_some_and(|b| pending.lower >= b.square)
        {
            continue;
        }
        if pending.patch.constant() {
            continue;
        }
        for patch in pending.patch.children(&mut search.budget)? {
            let lower = patch.lower(
                &search.target,
                search.best.as_ref().map(|b| &b.direction),
                &mut search.budget,
            )?;
            queue.push(Pending {
                patch,
                lower,
                sequence,
            });
            sequence += 1;
        }
        if queue.len() > MAX_PENDING {
            return Ok(None);
        }
    }
    let Some(best) = &search.best else {
        return Ok(None);
    };
    let square = best.square.clone();
    if search.finished(&square)? {
        Ok(search.best.map(|b| b.result))
    } else {
        Ok(None)
    }
}
