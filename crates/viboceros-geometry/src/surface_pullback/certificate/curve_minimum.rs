//! Global distance brackets from exact rational curve hulls, with numeric seeds.
use super::*;
use crate::{CurveMinimumDistance, ParameterSide};
use std::{cell::OnceCell, cmp::Ordering, collections::BinaryHeap, ops::Range, rc::Rc};

mod hull;
mod witness;
use hull::{Hull, axis_lower, box_lower, tree};
use witness::Search;

type P = [Rational; 3];
const MAX_CONTROLS: usize = 4096;
const MAX_PAIRS: usize = 65_536;

impl NurbsCurve {
    /// Find a near-minimum between complete active curve images, including both
    /// limits of discontinuities. Exact extracted hulls and separation planes
    /// prove the distance bracket; closest-point searches only propose witnesses.
    /// No rounded span extraction, sampling-only acceptance or source edits occur.
    ///
    /// Supports common-sign weights, degrees <=16 and <=4096 controls per curve.
    /// The absolute tolerance bounds the value error, not parameter error,
    /// uniqueness or topology. Native parameter quantization can prevent a
    /// sufficiently accurate witness. `None` means unsupported or unproved under
    /// the shared exact-arithmetic budgets, depth48 and 65536 queued pairs.
    pub fn minimum_distance_with_curve(
        &self,
        other: &Self,
        tolerance: Tolerance,
    ) -> Result<Option<CurveMinimumDistance>, GeometryError> {
        self.minimum_distance_seeded(other, tolerance, None)
    }

    /// Same complete-image bracket, starting from an independently proposed
    /// native-parameter pair (for example from a native kernel). The seed is
    /// exactly reevaluated and never substitutes for global lower bounds.
    pub fn minimum_distance_with_curve_seeded(
        &self,
        other: &Self,
        tolerance: Tolerance,
        parameters: [Real; 2],
    ) -> Result<Option<CurveMinimumDistance>, GeometryError> {
        for (curve, t) in [self, other].into_iter().zip(parameters) {
            if !t.is_finite() || !curve.domain().contains(&t) {
                return Err(GeometryError::ParameterOutOfDomain {
                    parameter: t,
                    domain_start: *curve.domain().start(),
                    domain_end: *curve.domain().end(),
                });
            }
        }
        self.minimum_distance_seeded(other, tolerance, Some(parameters))
    }

    fn minimum_distance_seeded(
        &self,
        other: &Self,
        tolerance: Tolerance,
        seed: Option<[Real; 2]>,
    ) -> Result<Option<CurveMinimumDistance>, GeometryError> {
        if [self, other]
            .iter()
            .any(|c| c.degree() > MAX_DEGREE || c.control_points().len() > MAX_CONTROLS)
        {
            return Ok(None);
        }
        match minimum([self, other], tolerance, seed) {
            Err(GeometryError::SurfaceCurveCertificateWorkLimit) => Ok(None),
            result => result,
        }
    }
}

struct Pair {
    lower: Rational,
    sequence: usize,
    hulls: [Rc<Hull>; 2],
}
impl PartialEq for Pair {
    fn eq(&self, b: &Self) -> bool {
        self.sequence == b.sequence && self.lower == b.lower
    }
}
impl Eq for Pair {}
impl Ord for Pair {
    fn cmp(&self, b: &Self) -> Ordering {
        b.lower
            .cmp(&self.lower)
            .then_with(|| b.sequence.cmp(&self.sequence))
    }
}
impl PartialOrd for Pair {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}

// sqrt(upper^2)-sqrt(lower) <= epsilon, compared exactly without square roots.
fn within_gap(
    upper: Real,
    lower: &Rational,
    epsilon: Real,
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    let upper = rational(upper);
    let e = rational(epsilon);
    let e2 = &e * &e;
    let d = &upper * &upper - lower - &e2;
    budget.check(&d)?;
    if d <= Rational::zero() {
        return Ok(true);
    }
    let a = &d * &d;
    let b = rational(4.) * e2 * lower;
    budget.check(&a)?;
    budget.check(&b)?;
    Ok(a <= b)
}

fn minimum(
    curves: [&NurbsCurve; 2],
    tolerance: Tolerance,
    seed: Option<[Real; 2]>,
) -> Result<Option<CurveMinimumDistance>, GeometryError> {
    let mut budget = Budget(MAX_WORK);
    let (Some(a), Some(b)) = (
        curve::Spline::spatial(curves[0], &mut budget)?,
        curve::Spline::spatial(curves[1], &mut budget)?,
    ) else {
        return Ok(None);
    };
    let roots = [
        tree(curves[0], &a, &mut budget)?,
        tree(curves[1], &b, &mut budget)?,
    ];
    let mut search = Search {
        curves,
        splines: [a, b],
        best: None,
        budget,
    };
    if let Some(parameters) = seed {
        search.candidate(parameters, [ParameterSide::Right; 2], tolerance.absolute())?;
    } else {
        search.proposals(tolerance)?;
    }
    let mut sequence = 0;
    let mut queue = BinaryHeap::from([Pair {
        lower: box_lower(&roots[0], &roots[1]),
        sequence,
        hulls: roots,
    }]);
    while let Some(mut pair) = queue.pop() {
        search.budget.charge(1)?;
        if let Some(best) = &search.best {
            if within_gap(
                best.result.upper_distance,
                &pair.lower,
                tolerance.absolute(),
                &mut search.budget,
            )? {
                return Ok(Some(best.result.clone()));
            }
            let lower = pair.lower.clone().max(axis_lower(
                &pair.hulls[0],
                &pair.hulls[1],
                &best.direction,
                &mut search.budget,
            )?);
            if lower >= best.square {
                continue;
            }
            if lower > pair.lower {
                pair.lower = lower;
                queue.push(pair);
                continue;
            }
        }
        let locations: Vec<_> = (0..2)
            .map(|i| {
                let r = &pair.hulls[i].range;
                Ok([
                    search.native(i, &r[0])?,
                    search.native(i, &((&r[0] + &r[1]) / rational(2.)))?,
                    search.native(i, &r[1])?,
                ])
            })
            .collect::<Result<_, GeometryError>>()?;
        for (i, j) in [(0, 0), (0, 2), (1, 1), (2, 0), (2, 2)] {
            let sides = [
                if i == 2 {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                },
                if j == 2 {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                },
            ];
            search.candidate(
                [locations[0][i], locations[1][j]],
                sides,
                tolerance.absolute(),
            )?;
        }
        if let Some(best) = &search.best
            && pair.lower >= best.square
        {
            continue;
        }
        let split = usize::from(pair.hulls[1].width() > pair.hulls[0].width());
        for child in pair.hulls[split].children(&mut search.budget)? {
            let mut hulls = pair.hulls.clone();
            hulls[split] = child;
            let lower = box_lower(&hulls[0], &hulls[1]);
            search.budget.check(&lower)?;
            sequence += 1;
            queue.push(Pair {
                lower,
                sequence,
                hulls,
            });
        }
        if queue.len() > MAX_PAIRS {
            return Ok(None);
        }
    }
    if let Some(best) = search.best
        && within_gap(
            best.result.upper_distance,
            &best.square,
            tolerance.absolute(),
            &mut search.budget,
        )?
    {
        return Ok(Some(best.result));
    }
    Ok(None)
}
