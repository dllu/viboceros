//! Continuous Bernstein bounds using finite intervals, followed by exact fallback.
use super::*;
mod crossing;
mod number;
pub(super) use crossing::Boxes;
use number::I;
#[cfg(test)]
mod tests;

pub(super) enum Outcome {
    Within(Real),
    Outside,
    Inconclusive,
}
pub(super) enum Crossing {
    Within(Real),
    /// The exact box bound exceeds the limit; this is not geometric rejection.
    Refine,
    Inconclusive,
}
type F = [I; 4];
pub(super) struct Patch {
    controls: Vec<F>,
}
impl Patch {
    pub fn new(net: &[H]) -> Option<Self> {
        let controls = net
            .iter()
            .map(|p| {
                Some([
                    I::exact(&p[0])?,
                    I::exact(&p[1])?,
                    I::exact(&p[2])?,
                    I::exact(&p[3])?,
                ])
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { controls })
    }
}
const FLOAT_WORK: usize = 65_536;
const FLOAT_DEPTH: usize = 8;

struct Work {
    remaining: usize,
    coefficients: BTreeMap<(usize, usize), Vec<I>>,
}
#[derive(Default)]
pub(super) struct Coefficients(BTreeMap<(usize, usize), Vec<I>>);
impl Work {
    fn charge(&mut self, n: usize) -> Option<()> {
        self.remaining = self.remaining.checked_sub(n)?;
        Some(())
    }
    fn coefficients(&mut self, p: usize, q: usize) -> Option<Vec<I>> {
        if let Some(values) = self.coefficients.get(&(p, q)) {
            return Some(values.clone());
        }
        self.charge((p + 1) * (q + 1))?;
        let mut values = Vec::new();
        for i in 0..=p {
            for j in 0..=q {
                values.push(I::exact(&Rational::new(
                    algebra::binomial(p, i) * algebra::binomial(q, j),
                    algebra::binomial(p + q, i + j),
                ))?);
            }
        }
        self.coefficients.insert((p, q), values.clone());
        Some(values)
    }
}
fn zero() -> I {
    I::point(0.).unwrap()
}
fn one() -> I {
    I::point(1.).unwrap()
}
fn multiply(a: &[I], b: &[I], work: &mut Work) -> Option<Vec<I>> {
    work.charge(a.len() * b.len())?;
    let c = work.coefficients(a.len() - 1, b.len() - 1)?;
    let mut result = vec![zero(); a.len() + b.len() - 1];
    let columns = b.len();
    for (i, &a) in a.iter().enumerate() {
        for (j, &b) in b.iter().enumerate() {
            result[i + j] = result[i + j].add(c[i * columns + j].mul(a)?.mul(b)?)?;
        }
    }
    Some(result)
}
fn basis(a: &[I], b: &[I], degree: usize, work: &mut Work) -> Option<Vec<Vec<I>>> {
    let mut ap = vec![vec![one()]];
    let mut bp = ap.clone();
    for i in 1..=degree {
        ap.push(multiply(&ap[i - 1], a, work)?);
        bp.push(multiply(&bp[i - 1], b, work)?);
    }
    (0..=degree)
        .map(|i| {
            let mut values = multiply(&ap[i], &bp[degree - i], work)?;
            let scale = I::exact(&Rational::from_integer(algebra::binomial(degree, i)))?;
            for v in &mut values {
                *v = v.mul(scale)?;
            }
            Some(values)
        })
        .collect()
}

#[cfg(test)]
fn bound(
    net: &[H],
    degree: [usize; 2],
    domains: &[[Rational; 2]; 2],
    uv: &[Uv],
    spatial: &[H],
    limit: Real,
) -> Outcome {
    let Some(net) = Patch::new(net) else {
        return Outcome::Inconclusive;
    };
    bound_with_cache(
        &net,
        degree,
        domains,
        uv,
        spatial,
        limit,
        &mut Coefficients::default(),
    )
}
pub(super) fn bound_with_cache(
    net: &Patch,
    degree: [usize; 2],
    domains: &[[Rational; 2]; 2],
    uv: &[Uv],
    spatial: &[H],
    limit: Real,
    coefficients: &mut Coefficients,
) -> Outcome {
    let mut work = Work {
        remaining: FLOAT_WORK,
        coefficients: std::mem::take(&mut coefficients.0),
    };
    let result = calculate(net, degree, domains, uv, spatial, limit, &mut work);
    coefficients.0 = work.coefficients;
    result.unwrap_or(Outcome::Inconclusive)
}
fn calculate(
    net: &Patch,
    degree: [usize; 2],
    domains: &[[Rational; 2]; 2],
    uv: &[Uv],
    spatial: &[H],
    limit: Real,
    work: &mut Work,
) -> Option<Outcome> {
    if !limit.is_finite() || limit < Real::MIN_POSITIVE {
        return None;
    }
    let uv = uv
        .iter()
        .map(|p| Some([I::exact(&p[0])?, I::exact(&p[1])?, I::exact(&p[2])?]))
        .collect::<Option<Vec<_>>>()?;
    let convert = |p: &H| {
        Some([
            I::exact(&p[0])?,
            I::exact(&p[1])?,
            I::exact(&p[2])?,
            I::exact(&p[3])?,
        ])
    };
    let net = &net.controls;
    let spatial = spatial.iter().map(convert).collect::<Option<Vec<_>>>()?;
    let mut bases = Vec::new();
    for axis in 0..2 {
        let low = I::exact(&domains[axis][0])?;
        let width = I::exact(&domains[axis][1])?;
        let a = uv
            .iter()
            .map(|p| p[axis].sub(low.mul(p[2])?)?.div(width))
            .collect::<Option<Vec<_>>>()?;
        let b = uv
            .iter()
            .zip(&a)
            .map(|(p, &a)| p[2].sub(a))
            .collect::<Option<Vec<_>>>()?;
        bases.push(basis(&a, &b, degree[axis], work)?);
    }
    let count = (uv.len() - 1) * (degree[0] + degree[1]) + 1;
    let mut image = vec![[zero(); 4]; count];
    for (j, v) in bases[1].iter().enumerate() {
        for (i, u) in bases[0].iter().enumerate() {
            let values = multiply(u, v, work)?;
            for (k, &value) in values.iter().enumerate() {
                work.charge(4)?;
                for axis in 0..4 {
                    image[k][axis] =
                        image[k][axis].add(net[j * (degree[0] + 1) + i][axis].mul(value)?)?;
                }
            }
        }
    }
    let c = work.coefficients(image.len() - 1, spatial.len() - 1)?;
    let mut difference = vec![[zero(); 4]; image.len() + spatial.len() - 1];
    for (i, a) in image.iter().enumerate() {
        for (j, b) in spatial.iter().enumerate() {
            work.charge(4)?;
            let c = c[i * spatial.len() + j];
            for axis in 0..3 {
                let d = a[axis].mul(b[3])?.sub(b[axis].mul(a[3])?)?;
                difference[i + j][axis] = difference[i + j][axis].add(c.mul(d)?)?;
            }
            difference[i + j][3] = difference[i + j][3].add(c.mul(a[3])?.mul(b[3])?)?;
        }
    }
    hull(difference, limit, work)
}
fn norm_squared(p: &F, limit: Real) -> Option<I> {
    if p[3].lo <= 0. {
        return None;
    }
    let scale = I::point(limit)?;
    let weight = p[3];
    let mut result = zero();
    for &p in &p[..3] {
        let q = p.abs()?.div(weight)?.div(scale)?;
        result = result.add(q.mul(q)?)?;
    }
    Some(result)
}
fn upper(square: Real, limit: Real) -> Real {
    if square == 0. {
        return 0.;
    }
    let exact = rational(square) * rational(limit) * rational(limit);
    let mut value = (square.sqrt() * limit).min(limit);
    for _ in 0..4 {
        let r = rational(value);
        if &r * &r >= exact {
            return value;
        }
        value = value.next_up().min(limit);
    }
    limit
}
fn hull(net: Vec<F>, limit: Real, work: &mut Work) -> Option<Outcome> {
    let mut pending = vec![(net, 0)];
    let mut result = 0_f64;
    while let Some((net, depth)) = pending.pop() {
        work.charge(4 * net.len())?;
        for p in [&net[0], net.last()?] {
            if norm_squared(p, limit)?.lo > 1. {
                return Some(Outcome::Outside);
            }
        }
        let mut bound = 0_f64;
        let mut within = true;
        for p in &net {
            let square = norm_squared(p, limit)?;
            if square.hi > 1. {
                within = false;
                break;
            }
            bound = bound.max(upper(square.hi, limit));
        }
        if within {
            result = result.max(bound);
            continue;
        }
        if depth == FLOAT_DEPTH {
            return None;
        }
        work.charge(4 * net.len().pow(2))?;
        let mut values = net;
        let mut left = vec![values[0]];
        let mut right = vec![*values.last()?];
        let half = I::point(0.5)?;
        for remaining in (1..values.len()).rev() {
            for i in 0..remaining {
                let next = values[i + 1];
                for (value, next) in values[i].iter_mut().zip(next) {
                    *value = value.add(next)?.mul(half)?;
                }
            }
            left.push(values[0]);
            right.push(values[remaining - 1]);
        }
        right.reverse();
        pending.push((right, depth + 1));
        pending.push((left, depth + 1));
    }
    Some(Outcome::Within(result))
}
