//! Closest paths of source chords through triangle Voronoi regions.
use super::*;
use crate::Polyline3;
use num_traits::One;

mod feature;
mod linear;
mod partition;
#[cfg(test)]
mod tests;

type E = [Rational; 3];
const MAX_WORK: usize = 4_000_000;
const MAX_FEATURES: usize = 131_072;
const MAX_DEPTH: usize = 64;
const MAX_BITS: u64 = 8192;

struct Budget(usize);
impl Budget {
    fn charge(&mut self, n: usize) -> Result<(), GeometryError> {
        self.0 = self.0.checked_sub(n).ok_or(GeometryError::MeshPullLimit)?;
        Ok(())
    }
    fn check<'b>(
        &mut self,
        values: impl IntoIterator<Item = &'b Rational>,
    ) -> Result<(), GeometryError> {
        for r in values {
            self.charge(1)?;
            if r.numer().bits() > MAX_BITS || r.denom().bits() > MAX_BITS {
                return Err(GeometryError::MeshPullLimit);
            }
        }
        Ok(())
    }
}

impl MeshProjectionIndex<'_> {
    /// Pulls a curve's bounded polyline approximation to the indexed mesh.
    /// Source approximation uses one quarter of absolute tolerance. The nearest
    /// image of each source chord is resolved by exact triangle Voronoi regions
    /// and quadratic distance comparisons. Accepted paths stay within absolute
    /// tolerance of the mesh, including rounding and small transition bridges.
    ///
    /// Discontinuous closest branches, exhausted arithmetic/depth/output bounds
    /// and unrepresentable output decline atomically. This does not certify
    /// closest-branch stability under perturbation of the original curved source.
    pub fn pull_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<Option<Polyline3>, GeometryError> {
        if curve.degree() == 1 {
            return match linear::source(curve)? {
                Some(polyline) => self.pull_polyline(&polyline, tolerance),
                None => Ok(None),
            };
        }
        let source_tolerance = Tolerance::try_new(
            tolerance.absolute() / 4.,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let polyline = curve.try_approximate_polyline(source_tolerance)?;
        self.pull_polyline(&polyline, tolerance)
    }

    /// Resolves closest paths of the supplied polyline's exact stored chords.
    /// Output vertices follow source order; a fully collapsed image is None.
    pub fn pull_polyline(
        &self,
        source: &Polyline3,
        tolerance: Tolerance,
    ) -> Result<Option<Polyline3>, GeometryError> {
        let mut budget = Budget(MAX_WORK);
        let mut output = Vec::new();
        for pair in source.vertices().windows(2) {
            let Some(first) = self.closest_location(pair[0])? else {
                return Ok(None);
            };
            // A stored vertex is exactly on the mesh. A rounded closest point
            // may lie just off its face and cannot supply a strict upper bound.
            let foot = self
                .triangle_points(first.triangle_index)
                .unwrap()
                .into_iter()
                .min_by(|&a, &b| pair[0].compare_distances(a, b))
                .unwrap();
            let candidates = self.pull_candidates([pair[0], pair[1]], foot, &mut budget)?;
            let origin = exact(pair[0]);
            let delta = sub(&exact(pair[1]), &origin);
            partition::trace(
                self,
                [&origin, &delta],
                candidates,
                foot,
                tolerance.absolute(),
                &mut output,
                &mut budget,
            )?;
        }
        if output.len() < 2 {
            return Ok(None);
        }
        Ok(Some(Polyline3::try_new(
            output,
            Tolerance::NUMERICAL_VALIDATION,
        )?))
    }

    fn pull_candidates(
        &self,
        source: [Point3; 2],
        foot: Point3,
        budget: &mut Budget,
    ) -> Result<Vec<usize>, GeometryError> {
        let upper = if foot.compare_distances(source[0], source[1]).is_gt() {
            [foot, source[0]]
        } else {
            [foot, source[1]]
        };
        self.pull_candidates_in_box(BoundingBox3::from_points(source)?, upper, budget)
    }

    fn pull_candidates_in_box(
        &self,
        source: BoundingBox3,
        upper: [Point3; 2],
        budget: &mut Budget,
    ) -> Result<Vec<usize>, GeometryError> {
        let mut pending = if self.nodes.is_empty() {
            vec![]
        } else {
            vec![0]
        };
        let mut result = Vec::new();
        while let Some(key) = pending.pop() {
            budget.charge(1)?;
            let node = &self.nodes[key];
            let mut a = [0.; 3];
            let mut b = a;
            for i in 0..3 {
                a[i] = source.min().to_array()[i].clamp(
                    node.bounds.min().to_array()[i],
                    node.bounds.max().to_array()[i],
                );
                b[i] = a[i].clamp(source.min().to_array()[i], source.max().to_array()[i]);
            }
            let lower = [Point3::try_from(a)?, Point3::try_from(b)?];
            if crate::point::compare_chords(lower, upper).is_gt() {
                continue;
            }
            if let Some(children) = node.children {
                pending.extend(children);
            } else {
                result.extend_from_slice(&self.order[node.range.clone()]);
            }
        }
        result.sort_unstable();
        Ok(result)
    }
}

fn exact(p: Point3) -> E {
    p.to_array().map(rational)
}
fn sub(a: &E, b: &E) -> E {
    std::array::from_fn(|i| &a[i] - &b[i])
}
fn dot(a: &E, b: &E) -> Rational {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

struct Feature {
    range: [Rational; 2],
    origin: E,
    delta: E,
    distance: [Rational; 3],
}
impl Feature {
    fn point(&self, t: &Rational) -> E {
        std::array::from_fn(|i| &self.origin[i] + t * &self.delta[i])
    }
    fn distance(&self, t: &Rational) -> Rational {
        &self.distance[0] + t * (&self.distance[1] + t * &self.distance[2])
    }
    fn no_closer_than(
        &self,
        best: &Self,
        a: &Rational,
        b: &Rational,
        budget: &mut Budget,
    ) -> Result<bool, GeometryError> {
        let q: [Rational; 3] = std::array::from_fn(|i| &self.distance[i] - &best.distance[i]);
        let first = &q[0] + a * (&q[1] + a * &q[2]);
        let last = &q[0] + b * (&q[1] + b * &q[2]);
        let control = &first + (b - a) * (&q[1] + rational(2.) * a * &q[2]) / rational(2.);
        budget.check([&first, &last, &control])?;
        Ok([first, last, control]
            .iter()
            .all(|v| *v >= Rational::zero()))
    }
}

fn trace(
    active: &[&Feature],
    a: Rational,
    b: Rational,
    tolerance: f64,
    output: &mut Vec<Point3>,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let mut pending = vec![(a, b, 0)];
    while let Some((a, b, depth)) = pending.pop() {
        budget.charge(active.len())?;
        let mid = (&a + &b) / rational(2.);
        let mut best = active[0];
        let mut distance = best.distance(&mid);
        for &f in &active[1..] {
            let d = f.distance(&mid);
            budget.check([&d])?;
            if d < distance {
                best = f;
                distance = d;
            }
        }
        let mut unresolved = Vec::new();
        for &f in active {
            if std::ptr::eq(f, best) {
                continue;
            }
            if !f.no_closer_than(best, &a, &b, budget)? {
                unresolved.push(f);
            }
        }
        let localized = || {
            let anchor = best.point(&mid);
            unresolved.iter().chain(std::iter::once(&best)).all(|f| {
                [f.point(&a), f.point(&b)]
                    .iter()
                    .all(|p| within(&sub(p, &anchor), tolerance / 16.))
            })
        };
        if unresolved.is_empty() || localized() {
            let first = rounded(best.point(&a), tolerance / 16.)?;
            let last = rounded(best.point(&b), tolerance / 16.)?;
            if let Some(&previous) = output.last()
                && previous.distance_bound_to(first, tolerance / 2.)?.is_none()
            {
                return Err(GeometryError::AmbiguousMeshPull);
            }
            for p in [first, last] {
                if output.last() != Some(&p) {
                    if output.len() >= crate::MAX_CURVE_DIVISION_POINTS {
                        return Err(GeometryError::MeshPullLimit);
                    }
                    output.push(p);
                }
            }
        } else {
            if depth == MAX_DEPTH {
                return Err(GeometryError::AmbiguousMeshPull);
            }
            pending.push((mid.clone(), b, depth + 1));
            pending.push((a, mid, depth + 1));
        }
    }
    Ok(())
}
fn within(delta: &E, limit: f64) -> bool {
    let l = rational(limit);
    dot(delta, delta) <= &l * &l
}
fn rounded(point: E, limit: f64) -> Result<Point3, GeometryError> {
    let p = Point3::try_new(scalar(&point[0])?, scalar(&point[1])?, scalar(&point[2])?)?;
    if !within(&sub(&point, &exact(p)), limit) {
        return Err(GeometryError::UnrepresentableMeshProjection);
    }
    Ok(p)
}
